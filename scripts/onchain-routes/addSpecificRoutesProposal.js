import {ApiPromise, WsProvider} from '@polkadot/api';
import {writeFileSync, appendFileSync} from 'fs';

import {createSdkContext, PoolType} from '@galacticcouncil/sdk';

// Technical Committee threshold (how many approvals needed to start motion)
const TC_THRESHOLD = 4;

//PROD RPC
const RPC = "wss://hydration-rpc.n.dwellir.com"

//LOCAL RPC
//const RPC = "ws://localhost:9999"

/* ========= CONFIG ========= */

// Asset pairs to create routes for
// Format: "assetIn-assetOut" (e.g., "10-222")
const ASSET_PAIRS = [
    "5-30",
    "30-1000766",
    "30-1000767",
    "0-1112",     // HUSDS
    "0-1000809",  // wstETH
    "0-1000625",  // sUSDe
    "0-1110",     // HUSDC
    "0-1000766",  // USDC
    "0-1113",     // HUSDe
    "0-252525",   // EWT
    "0-1000767",  // USDT
    "0-1006",     // atBTC
    "0-1000745",  // sUSDS
    "0-1039",     // aPAGX
    "0-1001",     // aDOT
    "0-1002",     // aUSDT
    "0-1004",     // aWBTC
    "0-1005",     // avDOT
    //"0-69",       // GDOT
    "0-34"        // ETH
];

/* ========= MAIN LOGIC ========= */

// Log file for route processing with timestamp
const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
const LOG_FILE = `route-processing-specific-${timestamp}.log`;

async function collectRoutesAndCreateProposal() {
    // Clear log file at start
    writeFileSync(LOG_FILE, '', 'utf8');

    const provider = new WsProvider(RPC);
    const apiPromise = await ApiPromise.create({provider, noInitWarn: true});

    const {api, client} = createSdkContext(apiPromise, {
        router: {exclude: PoolType.HSM} // Exclude HSM pools if needed,
    });

    // Parse asset pairs from input
    const pairs = ASSET_PAIRS.map(pairStr => {
        const [assetIn, assetOut] = pairStr.split('-').map(id => id.trim());
        if (!assetIn || !assetOut) {
            throw new Error(`Invalid asset pair format: ${pairStr}. Expected format: "assetIn-assetOut"`);
        }
        return {assetIn, assetOut};
    });

    log(`Processing ${pairs.length} asset pairs`);

    // Get asset metadata for logging
    const assetSymbolMap = {};
    for (const {assetIn, assetOut} of pairs) {
        for (const assetId of [assetIn, assetOut]) {
            if (!assetSymbolMap[assetId]) {
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
        }
    }

    log(`\nFetching routes for ${pairs.length} asset pairs...\n`);

    // Fetch routes for each pair
    const results = await Promise.allSettled(
        pairs.map(async ({assetIn, assetOut}) => {
            const hops = await api.router.getMostLiquidRoute(assetIn, assetOut);
            return {assetIn, assetOut, hops};
        })
    );

    // Collect successful routes and track failed ones
    const routesData = [];
    const failedToFetchRoutes = [];
    results.forEach((res, idx) => {
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

    // Optional: write to file for debugging
    const json = JSON.stringify(routesData, null, 2);
    writeFileSync(`routes-specific-${timestamp}.json`, json, 'utf8');
    log(`Routes written to routes-specific-${timestamp}.json for debugging\n`);

    // Build forceInsertRoute calls
    const calls = [];
    let processedCount = 0;
    let skippedCount = 0;

    // Track different types of routes for summary
    const newRoutes = [];
    const skippedRoutes = [];
    const resetRoutes = [];
    const resetToDefaultRoutes = [];
    const emptyRoutes = [];

    for (const {assetIn, assetOut, route} of routesData) {
        processedCount++;

        log(`\n[${processedCount}/${routesData.length}] Processing: ${assetIn}(${assetSymbolMap[assetIn]}) -> ${assetOut}(${assetSymbolMap[assetOut]})`);
        log(`  Route has ${route.length} hop(s)`);

        // Skip routes with zero hops as something went wrong collecting them
        if (route.length === 0) {
            skippedCount++;
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            emptyRoutes.push(routeLabel);
            log(`  ❌ SKIPPED: Route has zero hops (collection error)`);
            continue;
        }

        route.forEach((hop, idx) => {
            log(`  Hop ${idx + 1}: ${hop.assetIn} -> ${hop.assetOut} via ${hop.pool}${hop.poolId ? `(${hop.poolId})` : ''}`);
        });

        const isSingleOmnipoolHop = route.length === 1 && route[0].pool === 'Omnipool';

        // Check if route already exists in storage first (moved up to handle reset-to-default case)
        log(`  🔍 Checking on-chain storage...`);
        const existingRoute = await apiPromise.query.router.routes({assetIn, assetOut});

        // Skip routes with only one Omnipool hop ONLY if there's no existing route on-chain
        // (Omnipool is the default route, but we need to clear existing non-default routes)
        if (isSingleOmnipoolHop && existingRoute.isNone) {
            skippedCount++;
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            skippedRoutes.push(routeLabel);
            log(`  ❌ SKIPPED: Single Omnipool hop (default route) and no existing route on-chain`);
            continue;
        }

        // If it's a single Omnipool hop but there IS an existing route, we need to reset it to the Omnipool route
        // This can happen of example when an asset moved directly to omnipool, so no need multiple hops anymore
        if (isSingleOmnipoolHop && existingRoute.isSome) {
            // Transform the single Omnipool hop to on-chain format
            const omnipoolRoute = [{
                pool: {omnipool: null},
                assetIn: parseInt(assetIn),
                assetOut: parseInt(assetOut)
            }];

            //We skip if we already have the omnipool route set in storage
            const stored = existingRoute.unwrap().toJSON();
            const storedJson = JSON.stringify(stored);
            const omnipoolJson = JSON.stringify(omnipoolRoute);
            if (storedJson === omnipoolJson) {
                skippedCount++;
                const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
                skippedRoutes.push(routeLabel);
                log(`  ✅ SKIPPED: Single Omnipool hop already matches existing route on-chain`);
                continue;
            }

            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            resetToDefaultRoutes.push(routeLabel);
            log(`  🔄 RESET TO DEFAULT: Single Omnipool hop (default) but existing route needs to be reset`);
            log(`    Stored:   ${storedJson}`);
            log(`    Omnipool: ${omnipoolJson}`);

            const assetPair = {assetIn, assetOut};
            const call = apiPromise.tx.router.forceInsertRoute(assetPair, omnipoolRoute);
            calls.push(call);
            log(`  ✅ Added forceInsertRoute with Omnipool hop to batch to reset to default`);
            continue;
        }

        // Transform the SDK route format to match the on-chain storage format
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
        log(`  Transformed:`, JSON.stringify(transformedRoute));

        // existingRoute was already checked above, reuse it here
        if (existingRoute.isSome) {
            const stored = existingRoute.unwrap().toJSON();
            log(`  📦 Found existing route on-chain:`, JSON.stringify(stored));

            // Compare routes (deep equality check using transformed route)
            const storedJson = JSON.stringify(stored);
            const transformedJson = JSON.stringify(transformedRoute);
            const areEqual = storedJson === transformedJson;

            log(`  ⚖️  Comparing routes...`);
            log(`    Stored:      ${storedJson}`);
            log(`    Transformed: ${transformedJson}`);
            log(`    Equal? ${areEqual}`);

            if (areEqual) {
                skippedCount++;
                const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
                skippedRoutes.push(routeLabel);
                log(`  ✅ SKIPPED: Route already exists and matches`);
                continue; // Skip this pair as route already exists
            } else {
                const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
                resetRoutes.push(routeLabel);
                log(`  🔄 RESET NEEDED: Route exists but differs`);
            }
        } else {
            log(`  ⭐ NEW: No existing route on-chain`);
            // No existing route, this is a new route
            const routeLabel = `${assetSymbolMap[assetIn]}(${assetIn}) <> ${assetSymbolMap[assetOut]}(${assetOut})`;
            newRoutes.push(routeLabel);
        }

        // api.tx.router.forceInsertRoute({assetIn, assetOut}, route)
        const assetPair = {assetIn, assetOut};
        const call = apiPromise.tx.router.forceInsertRoute(assetPair, transformedRoute);
        calls.push(call);
        log(`  ✅ Added forceInsertRoute call to batch`);
    }

    log(`\n=== PROCESSING COMPLETE ===`);
    log(`Processed: ${processedCount} pairs`);
    log(`Skipped: ${skippedCount} pairs`);
    log(`Calls created: ${calls.length}`);

    // Double-check route validity: verify that A->B routes start with A and end with B
    log(`\n=== VALIDATING ROUTES ===`);
    const validationErrors = [];

    for (const {assetIn, assetOut, route} of routesData) {
        if (route.length > 0) {
            const firstHop = route[0];
            const lastHop = route[route.length - 1];

            // Convert to strings for comparison to handle type mismatches
            if (String(firstHop.assetIn) !== String(assetIn)) {
                const error = `ERROR: Route ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]}) starts with ${firstHop.assetIn} (${typeof firstHop.assetIn}) instead of ${assetIn} (${typeof assetIn})`;
                validationErrors.push(error);
                log(`  ❌ ${error}`);
            }

            if (String(lastHop.assetOut) !== String(assetOut)) {
                const error = `ERROR: Route ${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]}) ends with ${lastHop.assetOut} (${typeof lastHop.assetOut}) instead of ${assetOut} (${typeof assetOut})`;
                validationErrors.push(error);
                log(`  ❌ ${error}`);
            }
        }
    }

    if (validationErrors.length === 0) {
        log(`  ✅ All routes validated successfully!`);
    } else {
        log(`  ❌ Found ${validationErrors.length} validation errors!`);
    }

    // Write summary to check file
    const summary = {
        newRoutes: newRoutes,
        skippedRoutes: skippedRoutes,
        resetRoutes: resetRoutes,
        resetToDefaultRoutes: resetToDefaultRoutes,
        emptyRoutes: emptyRoutes,
        failedToFetchRoutes: failedToFetchRoutes,
        validationErrors: validationErrors,
        summary: {
            newRoutesCount: newRoutes.length,
            skippedRoutesCount: skippedRoutes.length,
            resetRoutesCount: resetRoutes.length,
            resetToDefaultRoutesCount: resetToDefaultRoutes.length,
            emptyRoutesCount: emptyRoutes.length,
            failedToFetchRoutesCount: failedToFetchRoutes.length,
            totalProcessed: routesData.length,
            validationErrorsCount: validationErrors.length
        }
    };

    writeFileSync(`route-check-specific-${timestamp}.json`, JSON.stringify(summary, null, 2), 'utf8');
    log(`\n=== ROUTE SUMMARY ===`);
    log(`New routes: ${newRoutes.length}`);
    log(`Skipped routes: ${skippedRoutes.length}`);
    log(`Reset routes: ${resetRoutes.length}`);
    log(`Reset to default routes: ${resetToDefaultRoutes.length}`);
    log(`Empty routes (skipped): ${emptyRoutes.length}`);
    log(`Failed to fetch routes: ${failedToFetchRoutes.length}`);
    log(`Validation errors: ${validationErrors.length}`);
    log(`Total processed: ${routesData.length}`);
    log(`Summary written to route-check-specific-${timestamp}.json (see file for detailed route lists)`);

    // Display comprehensive summary of all pairs
    log(`\n${'='.repeat(60)}`);
    log(`COMPREHENSIVE PAIRS SUMMARY`);
    log(`${'='.repeat(60)}\n`);

    // Display pairs to be changed for easy sharing with colleagues
    const pairsToBeChanged = [...newRoutes, ...resetRoutes, ...resetToDefaultRoutes];
    if (pairsToBeChanged.length > 0) {
        log(`📝 PAIRS TO BE CHANGED (${pairsToBeChanged.length}) - Copy this for colleagues:`);
        log(`${'─'.repeat(60)}`);
        pairsToBeChanged.forEach(pair => {
            log(`  ${pair}`);
        });
        log(`${'─'.repeat(60)}\n`);
    } else {
        log(`📝 PAIRS TO BE CHANGED: None\n`);
    }

    // Show breakdown by category
    if (newRoutes.length > 0) {
        log(`✨ NEW ROUTES (${newRoutes.length}):`);
        newRoutes.forEach(pair => log(`  ${pair}`));
        log(``);
    }

    if (resetRoutes.length > 0) {
        log(`🔄 ROUTES TO RESET (${resetRoutes.length}):`);
        resetRoutes.forEach(pair => log(`  ${pair}`));
        log(``);
    }

    if (resetToDefaultRoutes.length > 0) {
        log(`🔙 ROUTES TO RESET TO DEFAULT (${resetToDefaultRoutes.length}):`);
        resetToDefaultRoutes.forEach(pair => log(`  ${pair}`));
        log(``);
    }

    if (skippedRoutes.length > 0) {
        log(`✅ SKIPPED - ALREADY CORRECT (${skippedRoutes.length}):`);
        skippedRoutes.forEach(pair => log(`  ${pair}`));
        log(``);
    }

    if (emptyRoutes.length > 0) {
        log(`⚠️  EMPTY ROUTES - SKIPPED (${emptyRoutes.length}):`);
        emptyRoutes.forEach(pair => log(`  ${pair}`));
        log(``);
    }

    if (failedToFetchRoutes.length > 0) {
        log(`❌ FAILED TO FETCH FROM SDK (${failedToFetchRoutes.length}):`);
        failedToFetchRoutes.forEach(item => {
            log(`  ${item.label}`);
            log(`     Error: ${item.error}`);
        });
        log(``);
    }

    log(`${'='.repeat(60)}\n`);

    if (calls.length === 0) {
        log('\n⚠️  No calls to batch! All routes are either already set correctly or were skipped.');
        log('No proposal will be created.');
        apiPromise.disconnect();
        await apiPromise.disconnect();
        return;
    }

    // Create batch call
    const batch = apiPromise.tx.utility.batchAll(calls);

    // Wrap in technicalCommittee.propose
    const tcProposal = buildTechnicalCommitteePropose(apiPromise, batch, TC_THRESHOLD);

    log('\n--- utility.batchAll info ---');
    log(`Number of calls: ${calls.length}`);
    log(`Batch call length: ${batch.method.encodedLength ?? batch.method.toU8a().length} bytes`);

    const tcProposalHex = tcProposal.method.toHex();
    writeFileSync(`tcProposal-specific-${timestamp}.txt`, tcProposalHex, 'utf8');
    log(`\n--- TC propose HEX written to tcProposal-specific-${timestamp}.txt ---`);
    // log(`Hex length: ${tcProposalHex.length} characters`);

    // Optional: create preimage
    // const preimage = apiPromise.tx.preimage.notePreimage(batch.method.toHex());
    // log('\n--- preimage hex ---\n', preimage.method.toHex());

    apiPromise.disconnect();
    await apiPromise.disconnect();
    log('\nDone! Ready to submit the Technical Committee proposal.');
    log(`\nRoute processing log written to ${LOG_FILE}`);
}

// Helper function to log to both console and file
function log(message) {
    console.log(message);
    appendFileSync(LOG_FILE, message + '\n', 'utf8');
}

function buildTechnicalCommitteePropose(api, call, threshold) {
    const len = call.method.encodedLength ?? call.method.toU8a().length;
    // collective pallet expects (threshold, proposal, lengthBound)
    return api.tx.technicalCommittee.propose(threshold, call.method, len);
}

collectRoutesAndCreateProposal().catch(console.error);