import {ApiPromise, WsProvider} from '@polkadot/api';
import {writeFileSync, appendFileSync} from 'fs';
import {execSync} from 'child_process';

import {createSdkContext, PoolType} from '@galacticcouncil/sdk';

/*
 * This script retrieves EVM assets from the Aave lending pool contract,
 * converts them to HydraDX asset IDs, and creates a Technical Committee
 * proposal to set optimal routes for all asset pair combinations.
 *
 * Prerequisites:
 * - Foundry toolkit (cast CLI) must be installed
 * - Run: curl -L https://foundry.paradigm.xyz | bash && foundryup
 */

// Technical Committee threshold (how many approvals needed to start motion)
const TC_THRESHOLD = 4;

// RPC endpoints
const RPC = "wss://hydration.dotters.network";
const EVM_RPC = "wss://hydration.dotters.network";

// Aave pool contract address
const AAVE_POOL_ADDRESS = "0x1b02e051683b5cfac5929c25e84adb26ecf87b38";

/* ========= HELPER FUNCTIONS ========= */

// Log file for route processing with timestamp
const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
const LOG_FILE = `evm-route-processing-${timestamp}.log`;

// Helper function to log to both console and file
function log(message) {
    console.log(message);
    appendFileSync(LOG_FILE, message + '\n', 'utf8');
}

// Execute shell command and return result
function executeShellCommand(command) {
    try {
        const output = execSync(command, {
            encoding: 'utf8',
            stdio: ['pipe', 'pipe', 'pipe']
        });
        return {success: true, output: output.trim()};
    } catch (error) {
        return {
            success: false,
            error: error.message,
            stderr: error.stderr?.toString() || ''
        };
    }
}

// Parse EVM addresses from cast output
function parseEvmAddresses(castOutput) {
    try {
        // Cast returns addresses in format: [0xAddr1, 0xAddr2, ...]
        // Remove brackets and split by comma
        const cleaned = castOutput.replace(/[\[\]]/g, '').trim();
        if (!cleaned) {
            return [];
        }

        const addresses = cleaned.split(',').map(addr => addr.trim());

        // Validate addresses (should start with 0x and be 42 chars)
        const validAddresses = addresses.filter(addr => {
            return addr.startsWith('0x') && addr.length === 42;
        });

        return validAddresses;
    } catch (error) {
        log(`❌ Error parsing EVM addresses: ${error.message}`);
        return [];
    }
}

// Convert EVM addresses to asset IDs using runtime API
async function convertAddressesToAssets(apiPromise, addresses) {
    const results = await Promise.allSettled(
        addresses.map(async (address) => {
            try {
                // Call runtime API: erc20MappingApi::addressToAsset
                const result = await apiPromise.call.erc20MappingApi.addressToAsset(address);

                // Result is Option<AssetId>
                if (result.isSome) {
                    const assetId = result.unwrap().toString();
                    return {address, assetId, success: true};
                } else {
                    return {address, error: 'No asset mapping found', success: false};
                }
            } catch (error) {
                return {address, error: error.message, success: false};
            }
        })
    );

    const successful = [];
    const failed = [];

    results.forEach((res) => {
        if (res.status === 'fulfilled') {
            if (res.value.success) {
                successful.push({address: res.value.address, assetId: res.value.assetId});
            } else {
                failed.push({address: res.value.address, error: res.value.error});
            }
        } else {
            failed.push({address: 'unknown', error: res.reason});
        }
    });

    return {successful, failed};
}

// Generate unique asset pairs (A,B) where A < B
function generateAssetPairs(assetIds) {
    const pairs = [];

    // Sort asset IDs numerically to ensure pairs are ordered (smaller ID first)
    // This matches the Router storage which uses ordered_pair() (asset_in <= asset_out)
    const sortedAssetIds = assetIds.slice().sort((a, b) => {
        return parseInt(a) - parseInt(b);
    });

    // Generate pairs with smaller ID first
    for (let i = 0; i < sortedAssetIds.length; i++) {
        for (let j = i + 1; j < sortedAssetIds.length; j++) {
            pairs.push({
                assetIn: sortedAssetIds[i],
                assetOut: sortedAssetIds[j]
            });
        }
    }

    return pairs;
}

// Build Technical Committee proposal
function buildTechnicalCommitteePropose(api, call, threshold) {
    const len = call.method.encodedLength ?? call.method.toU8a().length;
    // collective pallet expects (threshold, proposal, lengthBound)
    return api.tx.technicalCommittee.propose(threshold, call.method, len);
}

/* ========= MAIN LOGIC ========= */

async function collectEvmAssetRoutesAndCreateProposal() {
    // Phase A: Initialize
    log('=== EVM ASSET ROUTE PROPOSAL SCRIPT ===\n');

    // Clear log file at start
    writeFileSync(LOG_FILE, '=== EVM ASSET ROUTE PROPOSAL SCRIPT ===\n\n', 'utf8');

    log('Phase A: Initializing...');

    const provider = new WsProvider(RPC);
    const apiPromise = await ApiPromise.create({provider, noInitWarn: true});

    const {api, client} = createSdkContext(apiPromise, {
        router: {exclude: PoolType.HSM} // Exclude HSM pools
    });

    log(`✅ Connected to ${RPC}`);
    log(`✅ SDK initialized with HSM exclusion\n`);

    // Initialize tracking arrays
    const newRoutes = [];
    const skippedRoutes = [];
    const skippedAlreadyExist = [];
    const skippedSingleOmnipool = [];
    const emptyRoutes = [];
    const failedToFetchRoutes = [];
    const validationErrors = [];

    // Phase B: Retrieve EVM Assets
    log('Phase B: Retrieving EVM assets from Aave pool contract...');

    const castCommand = `cast call ${AAVE_POOL_ADDRESS} "getReservesList()(address[])" --rpc-url ${EVM_RPC}`;
    log(`Executing: ${castCommand}`);

    const castResult = executeShellCommand(castCommand);

    if (!castResult.success) {
        log(`\n❌ FATAL ERROR: Failed to execute cast command`);
        log(`Error: ${castResult.error}`);
        if (castResult.stderr) {
            log(`Stderr: ${castResult.stderr}`);
        }
        log(`\nPlease ensure Foundry toolkit (cast) is installed:`);
        log(`curl -L https://foundry.paradigm.xyz | bash && foundryup`);
        await apiPromise.disconnect();
        return;
    }

    log(`✅ Cast command executed successfully`);

    const evmAddresses = parseEvmAddresses(castResult.output);

    if (evmAddresses.length === 0) {
        log(`\n❌ FATAL ERROR: No EVM addresses retrieved`);
        log(`Cast output: ${castResult.output}`);
        await apiPromise.disconnect();
        return;
    }

    log(`✅ Retrieved ${evmAddresses.length} EVM addresses`);
    evmAddresses.forEach((addr, idx) => {
        log(`  ${idx + 1}. ${addr}`);
    });
    log('');

    // Phase C: Convert to Asset IDs
    log('Phase C: Converting EVM addresses to asset IDs...');

    const {successful: successfulConversions, failed: failedConversions} =
        await convertAddressesToAssets(apiPromise, evmAddresses);

    log(`✅ Converted ${successfulConversions.length} addresses successfully`);

    if (failedConversions.length > 0) {
        log(`⚠️  Failed to convert ${failedConversions.length} addresses:`);
        failedConversions.forEach(({address, error}) => {
            log(`  ${address}: ${error}`);
        });
    }

    if (successfulConversions.length === 0) {
        log(`\n❌ FATAL ERROR: No successful address conversions`);
        await apiPromise.disconnect();
        return;
    }

    log('');
    log('Successful conversions:');
    successfulConversions.forEach(({address, assetId}) => {
        log(`  ${address} → Asset ${assetId}`);
    });
    log('');

    // Create mapping for logging
    const assetIdToAddress = {};
    successfulConversions.forEach(({address, assetId}) => {
        assetIdToAddress[assetId] = address;
    });

    // Phase D: Generate Asset Pairs
    log('Phase D: Generating asset pairs...');

    const assetIds = successfulConversions.map(item => item.assetId);
    const pairs = generateAssetPairs(assetIds);

    log(`✅ Generated ${pairs.length} unique asset pairs`);
    log(`   (N*(N-1)/2 where N=${assetIds.length})\n`);

    // Phase E: Fetch Asset Metadata
    log('Phase E: Fetching asset metadata...');

    const assetSymbolMap = {};
    for (const assetId of assetIds) {
        try {
            const meta = await apiPromise.query.assetRegistry.assets(assetId);
            if (meta.isSome) {
                assetSymbolMap[assetId] = meta.unwrap().symbol.toHuman();
            } else {
                assetSymbolMap[assetId] = `Unknown(${assetId})`;
            }
        } catch (e) {
            assetSymbolMap[assetId] = `Error(${assetId})`;
            log(`  ⚠️  Warning: Could not fetch metadata for asset ${assetId}`);
        }
    }

    log(`✅ Fetched metadata for ${Object.keys(assetSymbolMap).length} assets\n`);

    // Phase F: Fetch Routes from SDK
    log(`Phase F: Fetching routes from SDK for ${pairs.length} pairs...`);
    log('(This may take 30-60 seconds)\n');

    const routeResults = await Promise.allSettled(
        pairs.map(async ({assetIn, assetOut}) => {
            const hops = await api.router.getMostLiquidRoute(assetIn, assetOut);
            return {assetIn, assetOut, hops};
        })
    );

    // Collect successful routes and track failed ones
    const routesData = [];
    routeResults.forEach((res, idx) => {
        if (res.status === 'fulfilled') {
            const {assetIn, assetOut, hops} = res.value;
            routesData.push({assetIn, assetOut, route: hops});
        } else {
            const {assetIn, assetOut} = pairs[idx];
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            const errorMsg = String(res.reason);
            failedToFetchRoutes.push({label: routeLabel, error: errorMsg});
            log(`❌ Failed to fetch route for ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]}): ${res.reason}`);
        }
    });

    log(`✅ Successfully fetched ${routesData.length} routes`);
    log(`❌ Failed to fetch ${failedToFetchRoutes.length} routes\n`);

    // Write routes to file for debugging
    const routesJson = JSON.stringify(routesData, null, 2);
    writeFileSync(`evm-routes-${timestamp}.json`, routesJson, 'utf8');
    log(`Routes written to evm-routes-${timestamp}.json for debugging\n`);

    // Phase G: Process Routes & Check Existing Storage
    log('Phase G: Processing routes and checking existing storage...\n');

    const calls = [];
    let processedCount = 0;

    for (const {assetIn, assetOut, route} of routesData) {
        processedCount++;

        log(`[${processedCount}/${routesData.length}] Processing: ${assetIn}(${assetSymbolMap[assetIn]}) -> ${assetOut}(${assetSymbolMap[assetOut]})`);
        log(`  Route has ${route.length} hop(s)`);

        // Step 1: Skip routes with zero hops
        if (route.length === 0) {
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            emptyRoutes.push(routeLabel);
            log(`  ❌ SKIPPED: Route has zero hops (collection error)\n`);
            continue;
        }

        // Log route hops
        route.forEach((hop, idx) => {
            log(`  Hop ${idx + 1}: ${hop.assetIn} -> ${hop.assetOut} via ${hop.pool}${hop.poolId ? `(${hop.poolId})` : ''}`);
        });

        // Step 2: Check if route already exists
        log(`  🔍 Checking on-chain storage...`);
        const existingRoute = await apiPromise.query.router.routes({assetIn, assetOut});

        if (existingRoute.isSome) {
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            skippedAlreadyExist.push(routeLabel);
            log(`  ✅ SKIPPED: Route already exists on-chain\n`);
            continue;
        }

        log(`  ⭐ Route does not exist on-chain`);

        // Step 3: Filter single Omnipool hops (default routes)
        const isSingleOmnipoolHop = route.length === 1 && route[0].pool === 'Omnipool';

        if (isSingleOmnipoolHop) {
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            skippedSingleOmnipool.push(routeLabel);
            log(`  ❌ SKIPPED: Single Omnipool hop (default route)\n`);
            continue;
        }

        // Step 4: Transform route to on-chain format
        log(`  🔄 Transforming route to on-chain format...`);

        const transformedRoute = route.map(hop => {
            let pool;

            // Transform pool type to match the exact on-chain storage format
            if (hop.pool === 'Omnipool') {
                pool = {omnipool: null};
            } else if (hop.pool === 'Stableswap') {
                pool = {stableswap: parseInt(hop.poolId)};
            } else if (hop.pool === 'Aave') {
                pool = {aave: null};
            } else if (hop.pool === 'Xyk') {
                pool = {xyk: null};
            } else if (hop.pool === 'LBP') {
                pool = {lbp: null};
            } else if (hop.pool === 'Hsm') {
                pool = {hsm: null};
            } else {
                const errorMsg = `Unknown pool type: ${hop.pool} for route ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
                log(`\n❌ ERROR: ${errorMsg}`);
                throw new Error(errorMsg);
            }

            return {
                pool: pool,
                assetIn: parseInt(hop.assetIn),
                assetOut: parseInt(hop.assetOut)
            };
        });

        log(`  Transformed: ${JSON.stringify(transformedRoute)}`);

        // Step 5: Validate route integrity
        const firstHop = route[0];
        const lastHop = route[route.length - 1];

        if (String(firstHop.assetIn) !== String(assetIn)) {
            const error = `Route ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]}) starts with ${firstHop.assetIn} instead of ${assetIn}`;
            validationErrors.push(error);
            log(`  ❌ VALIDATION ERROR: ${error}\n`);
            continue;
        }

        if (String(lastHop.assetOut) !== String(assetOut)) {
            const error = `Route ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]}) ends with ${lastHop.assetOut} instead of ${assetOut}`;
            validationErrors.push(error);
            log(`  ❌ VALIDATION ERROR: ${error}\n`);
            continue;
        }

        // Add to proposal
        const assetPair = {assetIn, assetOut};
        const call = apiPromise.tx.router.forceInsertRoute(assetPair, transformedRoute);
        calls.push(call);

        const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) -> ${assetSymbolMap[assetOut]}(${assetOut})`;
        newRoutes.push(routeLabel);

        log(`  ✅ Added forceInsertRoute call to batch\n`);
    }

    log(`\n=== PROCESSING COMPLETE ===`);
    log(`Processed: ${processedCount} pairs`);
    log(`New routes to set: ${newRoutes.length}`);
    log(`Skipped (already exist): ${skippedAlreadyExist.length}`);
    log(`Skipped (single Omnipool): ${skippedSingleOmnipool.length}`);
    log(`Empty routes: ${emptyRoutes.length}`);
    log(`Failed to fetch: ${failedToFetchRoutes.length}`);
    log(`Validation errors: ${validationErrors.length}`);
    log(`Calls created: ${calls.length}\n`);

    // Phase H: Create Proposal
    if (calls.length === 0) {
        log('⚠️  No calls to batch! All routes are either already set correctly or were skipped.');
        log('No proposal will be created.\n');

        // Still write summary
        writeSummaryFiles();

        await apiPromise.disconnect();
        return;
    }

    log('Phase H: Creating Technical Committee proposal...');

    // Create batch call
    const batch = apiPromise.tx.utility.batchAll(calls);

    // Wrap in technicalCommittee.propose
    const tcProposal = buildTechnicalCommitteePropose(apiPromise, batch, TC_THRESHOLD);

    log(`✅ Batch created with ${calls.length} calls`);
    log(`✅ Wrapped in Technical Committee proposal (threshold: ${TC_THRESHOLD})`);

    const tcProposalHex = tcProposal.method.toHex();
    writeFileSync(`evm-tcProposal-${timestamp}.txt`, tcProposalHex, 'utf8');
    log(`✅ TC proposal hex written to evm-tcProposal-${timestamp}.txt\n`);

    // Phase I: Generate Outputs
    log('Phase I: Generating summary outputs...');

    writeSummaryFiles();

    log(`✅ Summary written to evm-route-check-${timestamp}.json`);
    log(`✅ Log written to ${LOG_FILE}\n`);

    // Final summary
    log('='.repeat(60));
    log('EVM ASSET ROUTE PROCESSING SUMMARY');
    log('='.repeat(60));
    log('');
    log('EVM Assets:');
    log(`  Retrieved: ${evmAddresses.length}`);
    log(`  Successfully Converted: ${successfulConversions.length} (${Math.round(successfulConversions.length / evmAddresses.length * 100)}%)`);
    log(`  Failed Conversions: ${failedConversions.length} (${Math.round(failedConversions.length / evmAddresses.length * 100)}%)`);
    log('');
    log('Asset Pairs:');
    log(`  Unique Pairs Generated: ${pairs.length}`);
    log('');
    log('Route Processing:');
    log(`  New Routes to Set: ${newRoutes.length} (${Math.round(newRoutes.length / routesData.length * 100)}%)`);
    log(`  Already Exist (Skipped): ${skippedAlreadyExist.length} (${Math.round(skippedAlreadyExist.length / routesData.length * 100)}%)`);
    log(`  Single Omnipool (Skipped): ${skippedSingleOmnipool.length} (${Math.round(skippedSingleOmnipool.length / routesData.length * 100)}%)`);
    log(`  Empty Routes (Skipped): ${emptyRoutes.length}`);
    log(`  Failed to Fetch: ${failedToFetchRoutes.length}`);
    log(`  Validation Errors: ${validationErrors.length}`);
    log('');
    log('Proposal Details:');
    log(`  Total forceInsertRoute calls: ${calls.length}`);
    log(`  Technical Committee Threshold: ${TC_THRESHOLD}`);
    log('');
    log('Output Files:');
    log(`  - Proposal: evm-tcProposal-${timestamp}.txt`);
    log(`  - Routes: evm-routes-${timestamp}.json`);
    log(`  - Summary: evm-route-check-${timestamp}.json`);
    log(`  - Log: ${LOG_FILE}`);
    log('');

    if (newRoutes.length > 0) {
        log('New Routes to be Set:');
        log('-'.repeat(60));
        newRoutes.forEach(route => log(`  ${route}`));
        log('-'.repeat(60));
        log('');
    }

    log('Done! Ready to submit the Technical Committee proposal.');

    await apiPromise.disconnect();

    // Helper function to write summary files
    function writeSummaryFiles() {
        const summary = {
            evmAssets: {
                totalRetrieved: evmAddresses.length,
                convertedSuccessfully: successfulConversions.length,
                conversionFailed: failedConversions.length,
                failedAddresses: failedConversions
            },
            pairGeneration: {
                uniquePairsGenerated: pairs.length
            },
            routeProcessing: {
                newRoutes: newRoutes.length,
                skippedAlreadyExist: skippedAlreadyExist.length,
                skippedSingleOmnipool: skippedSingleOmnipool.length,
                emptyRoutes: emptyRoutes.length,
                failedToFetch: failedToFetchRoutes.length,
                validationErrors: validationErrors.length,
                totalProcessed: routesData.length
            },
            newRoutes: newRoutes,
            skippedAlreadyExist: skippedAlreadyExist,
            skippedSingleOmnipool: skippedSingleOmnipool,
            emptyRoutes: emptyRoutes,
            failedToFetchRoutes: failedToFetchRoutes,
            validationErrors: validationErrors
        };

        writeFileSync(`evm-route-check-${timestamp}.json`, JSON.stringify(summary, null, 2), 'utf8');
    }
}

collectEvmAssetRoutesAndCreateProposal().catch(console.error);
