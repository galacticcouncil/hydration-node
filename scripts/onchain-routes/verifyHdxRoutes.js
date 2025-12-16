import {ApiPromise, WsProvider} from '@polkadot/api';
import {writeFileSync, appendFileSync} from 'fs';

//PROD RPC
//const RPC = "wss://hydration-rpc.n.dwellir.com"
const RPC = "ws://localhost:9999"
//LOCAL RPC
//const RPC = "ws://localhost:9999"

/* ========= CONFIG ========= */

const HDX_ASSET_ID = '0';

// Sample weight to test fee conversion (using a typical weight)
const TEST_WEIGHT = {
    refTime: 1000000000, // 1 billion ref time
    proofSize: 65536     // 64 KB proof size
};

/* ========= MAIN LOGIC ========= */

// Log file with timestamp
const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
const LOG_FILE = `hdx-payment-validation-${timestamp}.log`;

async function validateAcceptedCurrencies() {
    // Clear log file at start
    writeFileSync(LOG_FILE, '', 'utf8');

    log('=== XCM Payment API Validation for Accepted Currencies ===\n');

    const provider = new WsProvider(RPC);
    const apiPromise = await ApiPromise.create({provider, noInitWarn: true});

    // Query accepted currencies
    log('🔍 Querying multiTransactionPayment.acceptedCurrencies...\n');
    const acceptedCurrenciesRaw = await apiPromise.query.multiTransactionPayment.acceptedCurrencies.entries();

    if (acceptedCurrenciesRaw.length === 0) {
        log('⚠️  No accepted currencies found in storage!');
        await apiPromise.disconnect();
        return;
    }

    // Extract asset IDs from storage keys
    const acceptedCurrencies = acceptedCurrenciesRaw.map(([key, _value]) => {
        const assetId = key.args[0].toString();
        return assetId;
    });

    log(`Found ${acceptedCurrencies.length} accepted currencies\n`);

    // Fetch asset metadata
    log('📋 Fetching asset metadata...\n');
    const assetSymbolMap = {};

    for (const assetId of acceptedCurrencies) {
        try {
            const meta = await apiPromise.query.assetRegistry.assets(assetId);
            if (meta.isSome) {
                assetSymbolMap[assetId] = meta.unwrap().symbol.toHuman();
            } else {
                assetSymbolMap[assetId] = `Unknown(${assetId})`;
            }
        } catch (e) {
            assetSymbolMap[assetId] = `Error(${assetId})`;
            log(`⚠️  Warning: Could not fetch metadata for asset ${assetId}: ${e.message}`);
        }
    }

    log('='.repeat(80));
    log('\n');

    // Test XCM Payment API for each accepted currency
    const results = [];
    let successCount = 0;
    let failureCount = 0;

    for (let i = 0; i < acceptedCurrencies.length; i++) {
        const assetId = acceptedCurrencies[i];
        const symbol = assetSymbolMap[assetId] || assetId;

        log(`[${i + 1}/${acceptedCurrencies.length}] Testing ${symbol} (${assetId})`);
        log('-'.repeat(80));

        // Get XCM location for this asset
        let xcmLocation = null;
        let hasLocation = false;

        try {
            const locationQuery = await apiPromise.query.assetRegistry.assetLocations(assetId);
            if (locationQuery.isSome) {
                xcmLocation = locationQuery.unwrap();
                hasLocation = true;
                log(`  📍 XCM Location found: ${JSON.stringify(xcmLocation.toHuman())}`);
            } else {
                log(`  ⚠️  No XCM location found in assetRegistry.assetLocations`);
            }
        } catch (e) {
            log(`  ❌ Error fetching XCM location: ${e.message}`);
        }

        // Try to call xcmPaymentApi.queryWeightToAssetFee
        let feeResult = null;
        let error = null;
        let canComputeFee = false;

        if (hasLocation) {
            try {
                // The location from storage is already a versioned location
                // In XCM V3+, AssetId IS the location (MultiLocation for V3, Location for V4)
                log(`  🔍 Calling xcmPaymentApi.queryWeightToAssetFee...`);
                log(`  📋 Location JSON: ${JSON.stringify(xcmLocation.toJSON())}`);

                // Create versioned asset ID - the location itself is the asset identifier
                let versionedAssetId;

                // Check the version of the location from storage
                const locationJson = xcmLocation.toJSON();

                if (locationJson.v3) {
                    // For V3: AssetId::Concrete(MultiLocation)
                    versionedAssetId = apiPromise.createType('XcmVersionedAssetId', {
                        V3: {
                            Concrete: locationJson.v3
                        }
                    });
                    log(`  🔧 Created V3 AssetId with Concrete wrapper`);
                } else if (locationJson.v4) {
                    // For V4: AssetId is just the Location (no Concrete wrapper in V4)
                    versionedAssetId = apiPromise.createType('XcmVersionedAssetId', {
                        V4: locationJson.v4
                    });
                    log(`  🔧 Created V4 AssetId`);
                } else {
                    // Fallback: try treating it as V3 with Concrete wrapper
                    log(`  ⚠️  Unknown location version, trying as V3 with Concrete...`);
                    versionedAssetId = apiPromise.createType('XcmVersionedAssetId', {
                        V3: {
                            Concrete: xcmLocation.toJSON()
                        }
                    });
                }

                log(`  🔧 AssetId created: ${JSON.stringify(versionedAssetId.toJSON())}`);

                feeResult = await apiPromise.call.xcmPaymentApi.queryWeightToAssetFee(
                    TEST_WEIGHT,
                    versionedAssetId
                );

                // Check if the result is Ok or Err
                if (feeResult.isOk) {
                    canComputeFee = true;
                    successCount++;
                    log(`  ✅ SUCCESS: Fee computable - ${feeResult.asOk.toString()} units`);
                } else {
                    failureCount++;
                    const err = feeResult.asErr;
                    error = err.type || err.toString();
                    log(`  ❌ ERROR: ${error}`);
                }
            } catch (e) {
                failureCount++;
                error = e.message;
                log(`  ❌ EXCEPTION: ${error}`);
                log(`  📋 Error stack: ${e.stack}`);
            }
        } else {
            failureCount++;
            error = 'No XCM location found';
            log(`  ❌ FAILED: Cannot test without XCM location`);
        }

        log('\n');

        // Store result
        results.push({
            assetId,
            symbol,
            hasLocation,
            xcmLocation: xcmLocation ? xcmLocation.toHuman() : null,
            canComputeFee,
            feeResult: feeResult && feeResult.isOk ? feeResult.asOk.toString() : null,
            error
        });
    }

    // Print summary
    log('='.repeat(80));
    log('\n=== SUMMARY ===\n');
    log(`Total accepted currencies: ${acceptedCurrencies.length}`);
    log(`Fee computable (SUCCESS): ${successCount} / ${acceptedCurrencies.length} (${Math.round(successCount / acceptedCurrencies.length * 100)}%)`);
    log(`Fee NOT computable (FAILED): ${failureCount} / ${acceptedCurrencies.length} (${Math.round(failureCount / acceptedCurrencies.length * 100)}%)`);

    // List assets with issues
    const failedAssets = results.filter(r => !r.canComputeFee);
    if (failedAssets.length > 0) {
        log(`\n=== Assets with Issues (Fee NOT Computable) ===\n`);
        failedAssets.forEach(r => {
            log(`  ❌ ${r.symbol} (${r.assetId})`);
            log(`     Reason: ${r.error || 'Unknown'}`);
            if (!r.hasLocation) {
                log(`     Missing XCM location`);
            }
        });
    }

    // List working assets
    const workingAssets = results.filter(r => r.canComputeFee);
    if (workingAssets.length > 0) {
        log(`\n=== Assets Working Correctly ===\n`);
        workingAssets.forEach(r => {
            log(`  ✅ ${r.symbol} (${r.assetId}) - Fee: ${r.feeResult} units`);
        });
    }

    // Write detailed results to JSON
    const jsonOutput = {
        timestamp: new Date().toISOString(),
        testWeight: TEST_WEIGHT,
        totalCurrencies: acceptedCurrencies.length,
        successCount,
        failureCount,
        results
    };
    writeFileSync(`payment-validation-${timestamp}.json`, JSON.stringify(jsonOutput, null, 2), 'utf8');

    log(`\n=== OUTPUT FILES ===`);
    log(`Log file: ${LOG_FILE}`);
    log(`JSON results: payment-validation-${timestamp}.json\n`);

    await apiPromise.disconnect();
}

// Helper function to log to both console and file
function log(message) {
    console.log(message);
    appendFileSync(LOG_FILE, message + '\n', 'utf8');
}

validateAcceptedCurrencies().catch(console.error);
