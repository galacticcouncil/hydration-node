import {ApiPromise, WsProvider} from '@polkadot/api';
import {writeFileSync, appendFileSync} from 'fs';

import {createSdkContext, PoolType} from '@galacticcouncil/sdk';

// Technical Committee threshold (how many approvals needed to start motion)
const TC_THRESHOLD = 1;

//PROD RPC
const RPC = "wss://hydration-rpc.n.dwellir.com"

//LOCAL RPC
//const RPC = "ws://localhost:9999"

// Log file for route processing with timestamp
const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
const LOG_FILE = `route-processing-${timestamp}.log`;

async function collectRoutesAndCreateProposal() {
    // Clear log file at start
    writeFileSync(LOG_FILE, '', 'utf8');

    const provider = new WsProvider(RPC);
    const apiPromise = await ApiPromise.create({provider, noInitWarn: true});

    const {api, client} = createSdkContext(apiPromise, {
        router: {exclude: [PoolType.XYK]},
    });

    const routable = await api.router.getAllAssets();
    const report = routable
        .map((a) => [a.id, a.symbol])
        .sort((a, b) => Number(a[0]) - Number(b[0]));
    const ids = routable.map((a) => a.id);

    // Create a map of asset ID to symbol for easy lookup
    const assetSymbolMap = {};
    routable.forEach((a) => {
        assetSymbolMap[a.id] = a.symbol;
    });

    log(`Found ${ids.length} routable assets (excluding XYK pools)`);

    // build all ordered pairs (skip self-pairs)
    const pairs = ids.flatMap((assetIn) =>
        ids.filter((assetOut) => assetOut !== assetIn).map((assetOut) => ({assetIn, assetOut}))
    );

    log(`Fetching routes for ${pairs.length} asset pairs...`);

    const results = await Promise.allSettled(
        pairs.map(async ({assetIn, assetOut}) => {
            const hops = await api.router.getMostLiquidRoute(assetIn, assetOut);
            return {assetIn, assetOut, hops};
        })
    );

    // collect into nested map: routes[assetIn][assetOut] = Hop[]
    const routes = {};

    results.forEach((res) => {
        if (res.status === 'fulfilled') {
            const {assetIn, assetOut, hops} = res.value;
            if (!routes[assetIn]) routes[assetIn] = {};
            routes[assetIn][assetOut] = hops;
        }
    });

    // Optional: still write to file for debugging
    const json = JSON.stringify(routes, null, 2);
    writeFileSync('routes.json', json, 'utf8');
    log('Routes written to routes.json for debugging');

    // Get unique asset pairs (avoiding duplicates like 5->10 and 10->5)
    const uniquePairs = getUniqueAssetPairs(routes);

    log(`Found ${uniquePairs.length} unique asset pairs with routes\n`);

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

    for (const {assetIn, assetOut, route} of uniquePairs) {
        processedCount++;

        log(`\n[${processedCount}/${uniquePairs.length}] Processing: ${assetIn}(${assetSymbolMap[assetIn]}) -> ${assetOut}(${assetSymbolMap[assetOut]})`);
        log(`  Route has ${route.length} hop(s)`);

        // Skip routes with zero hops as something went wrong collecting them
        if (route.length === 0) {
            skippedCount++;
            const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
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
            const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
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
                const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
                skippedRoutes.push(routeLabel);
                log(`  ✅ SKIPPED: Single Omnipool hop already matches existing route on-chain`);
                continue;
            }

            const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
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
            } else if (hop.pool === 'XYK') {
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
                const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
                skippedRoutes.push(routeLabel);
                log(`  ✅ SKIPPED: Route already exists and matches`);
                continue; // Skip this pair as route already exists
            } else {
                const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
                resetRoutes.push(routeLabel);
                log(`  🔄 RESET NEEDED: Route exists but differs`);
            }
        } else {
            log(`  ⭐ NEW: No existing route on-chain`);
            // No existing route, this is a new route
            const routeLabel = `${assetIn}(${assetSymbolMap[assetIn]})->${assetOut}(${assetSymbolMap[assetOut]})`;
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

    for (const {assetIn, assetOut, route} of uniquePairs) {
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
        validationErrors: validationErrors,
        summary: {
            newRoutesCount: newRoutes.length,
            skippedRoutesCount: skippedRoutes.length,
            resetRoutesCount: resetRoutes.length,
            resetToDefaultRoutesCount: resetToDefaultRoutes.length,
            emptyRoutesCount: emptyRoutes.length,
            totalProcessed: uniquePairs.length,
            validationErrorsCount: validationErrors.length
        }
    };

    writeFileSync(`route-check-${timestamp}.json`, JSON.stringify(summary, null, 2), 'utf8');
    log(`\n=== ROUTE SUMMARY ===`);
    log(`New routes: ${newRoutes.length}`);
    log(`Skipped routes: ${skippedRoutes.length}`);
    log(`Reset routes: ${resetRoutes.length}`);
    log(`Reset to default routes: ${resetToDefaultRoutes.length}`);
    log(`Empty routes (skipped): ${emptyRoutes.length}`);
    log(`Validation errors: ${validationErrors.length}`);
    log(`Total processed: ${uniquePairs.length}`);
    log(`Summary written to route-check-${timestamp}.json (see file for detailed route lists)`);

    // Create batch call
    const batch = apiPromise.tx.utility.batchAll(calls);

    // Wrap in technicalCommittee.propose
    const tcProposal = buildTechnicalCommitteePropose(apiPromise, batch, TC_THRESHOLD);

    log('\n--- utility.batchAll info ---');
    log(`Number of calls: ${calls.length}`);
    log(`Batch call length: ${batch.method.encodedLength ?? batch.method.toU8a().length} bytes`);

    const tcProposalHex = tcProposal.method.toHex();
    writeFileSync('tcProposal.txt', tcProposalHex, 'utf8');
    log('\n--- TC propose HEX written to tcProposal.txt ---');
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

function getUniqueAssetPairs(routesData) {
    const uniquePairs = new Set();
    const pairsWithRoutes = [];

    for (const assetIn in routesData) {
        for (const assetOut in routesData[assetIn]) {
            const route = routesData[assetIn][assetOut];

            // Create a canonical pair key (smaller asset ID first)
            const assetInNum = parseInt(assetIn);
            const assetOutNum = parseInt(assetOut);
            const pairKey = assetInNum < assetOutNum
                ? `${assetInNum}-${assetOutNum}`
                : `${assetOutNum}-${assetInNum}`;

            if (!uniquePairs.has(pairKey)) {
                uniquePairs.add(pairKey);
                pairsWithRoutes.push({
                    assetIn: assetInNum,
                    assetOut: assetOutNum,
                    route: route
                });
            }
        }
    }

    return pairsWithRoutes;
}

collectRoutesAndCreateProposal().catch(console.error);
