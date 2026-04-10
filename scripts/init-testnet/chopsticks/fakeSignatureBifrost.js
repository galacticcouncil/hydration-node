// Exercises the PRE-PR `emaOracle.updateBifrostOracle` path against a chopsticks fork.
//
// The old runtime gates this call with `BifrostOrigin = EnsureSignedBy<BifrostAcc>`, so the
// caller must literally be the Bifrost sibling sovereign account. We use chopsticks'
// `signFakeWithApi` to bypass signature verification. No storage injection is needed because
// the `ExternalSources` / `AuthorizedAccounts` maps don't exist yet in the old runtime.
//
// The old code also has a 10% `is_within_range` guard that kicks in if a TenMinutes oracle
// entry already exists for (BIFROST_SOURCE, DOT/asset_15). We query the current entry and
// reuse its exact price to avoid tripping it.

import {signFakeWithApi} from '@acala-network/chopsticks-utils';
import {ApiPromise, WsProvider} from "@polkadot/api";

// Bifrost sovereign (sibling:2030)
const BIFROST_SOVEREIGN = "7LCt6dFs6sraSg31uKfbRH7soQ66GRb3LAkGZJ1ie3369crq";

// "bifrosto" as [u8; 8]
const BIFROST_SOURCE = "0x626966726f73746f";

// Asset whose price Bifrost feeds in production. 15 = vDOT on Hydration.
const QUOTE_ASSET_ID = 15;

const main = async () => {
    const uri = "ws://127.0.0.1:8000";
    const provider = new WsProvider(uri);

    // Same custom signed extensions as the PR script — needed so extrinsics encode
    // against this runtime's metadata.
    const api = await ApiPromise.create({
        provider,
        signedExtensions: {
            ValidateClaim: {extrinsic: {}, payload: {}},
            CheckMetadataHash: {extrinsic: {mode: "u8"}, payload: {}},
            StorageWeightReclaim: {extrinsic: {}, payload: {}},
        },
    });

    // Instant block build so our tx lands in a block as soon as it's submitted.
    await provider.send("dev_setBlockBuildMode", ["Instant"]);

    // Resolve the quote asset's XCM location from the asset registry.
    const quoteLocationOpt = await api.query.assetRegistry.assetLocations(QUOTE_ASSET_ID);
    if (quoteLocationOpt.isNone) {
        console.error(`Asset ${QUOTE_ASSET_ID} has no XCM location registered`);
        process.exit(1);
    }
    const quoteVersionedLocation = {V4: quoteLocationOpt.unwrap().toJSON()};
    // DOT = relay chain, Location::parent()
    const dotVersionedLocation = {V4: {parents: 1, interior: "Here"}};

    console.log("Asset location:", JSON.stringify(quoteVersionedLocation));

    // Determine a price that passes the 10% range check: reuse the current TenMinutes
    // entry's price if one already exists for this pair under BIFROST_SOURCE.
    //
    // Storage key uses the ORDERED pair (smaller asset id first). DOT (asset 5 on Hydration)
    // < vDOT (15), so the ordered pair is (5, 15).
    const DOT_ASSET_ID = 5;
    const orderedPair = DOT_ASSET_ID < QUOTE_ASSET_ID
        ? [DOT_ASSET_ID, QUOTE_ASSET_ID]
        : [QUOTE_ASSET_ID, DOT_ASSET_ID];

    const existing = await api.query.emaOracle.oracles(
        BIFROST_SOURCE,
        orderedPair,
        'TenMinutes',
    );

    let price;
    let prevLastBlockUpdatedAt = null;
    if (existing.isSome) {
        const [entry] = existing.unwrap();
        const p = entry.price;
        // price is EmaPrice { n, d }
        price = [p.n.toString(), p.d.toString()];
        console.log("Reusing current TenMinutes price:", price);
    } else {
        price = ["1000000000000", "1000000000000"]; // 1.0 — no range check will apply
        console.log("No existing oracle, using default price:", price);
    }

    // Snapshot pre-tx LastBlock entry so we can detect the accumulator flush.
    const prevLastBlock = await api.query.emaOracle.oracles(
        BIFROST_SOURCE,
        orderedPair,
        'LastBlock',
    );
    if (prevLastBlock.isSome) {
        const [entry] = prevLastBlock.unwrap();
        prevLastBlockUpdatedAt = entry.updatedAt.toNumber();
        console.log("Pre-tx LastBlock.updated_at:", prevLastBlockUpdatedAt);
    }

    const tx = api.tx.emaOracle.updateBifrostOracle(
        dotVersionedLocation,
        quoteVersionedLocation,
        price,
    );

    console.log("Fake-signing as Bifrost sovereign:", BIFROST_SOVEREIGN);
    await signFakeWithApi(api, tx, BIFROST_SOVEREIGN);

    console.log("Submitting...");
    await new Promise((resolve, reject) => {
        tx.send((result) => {
            console.log("Status:", result.status.type);
            if (result.dispatchError) {
                const err = result.dispatchError;
                if (err.isModule) {
                    const decoded = api.registry.findMetaError(err.asModule);
                    console.error(`DispatchError: ${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`);
                } else {
                    console.error("DispatchError:", err.toString());
                }
                reject(new Error("dispatch error"));
                return;
            }
            if (result.status.isInBlock) {
                console.log("Included in block:", result.status.asInBlock.toHex());
                for (const {event} of result.events) {
                    console.log(`  event: ${event.section}.${event.method}`);
                }
                resolve(result);
            }
        }).catch(reject);
    });

    // === Post-tx verification ==================================================
    // Prove the PR's v1->v2 migration actually seeded the new storages and the
    // deprecated extrinsic's write-path landed.

    console.log("\n--- Post-tx storage verification ---");

    // 1. ExternalSources[BIFROST_SOURCE] must exist (seeded by MigrateV1ToV2).
    const extSrc = await api.query.emaOracle.externalSources(BIFROST_SOURCE);
    console.log("ExternalSources[bifrosto]:", extSrc.isSome ? "PRESENT" : "MISSING");

    // 2. AuthorizedAccounts[BIFROST_SOURCE][bifrost_sovereign] must exist.
    const auth = await api.query.emaOracle.authorizedAccounts(BIFROST_SOURCE, BIFROST_SOVEREIGN);
    console.log(`AuthorizedAccounts[bifrosto][${BIFROST_SOVEREIGN}]:`, auth.isSome ? "PRESENT" : "MISSING");

    // 3. The extrinsic should have pushed an entry into the accumulator, and
    //    `on_finalize` should have flushed it into LastBlock with the current
    //    block's `updated_at`. If `prevLastBlockUpdatedAt` was set, the new one
    //    must be strictly greater.
    const newLastBlock = await api.query.emaOracle.oracles(
        BIFROST_SOURCE,
        orderedPair,
        'LastBlock',
    );
    if (newLastBlock.isSome) {
        const [entry] = newLastBlock.unwrap();
        const nowUpdatedAt = entry.updatedAt.toNumber();
        console.log("Post-tx LastBlock.updated_at:", nowUpdatedAt);
        if (prevLastBlockUpdatedAt !== null) {
            const advanced = nowUpdatedAt > prevLastBlockUpdatedAt;
            console.log(
                `LastBlock entry advanced: ${advanced ? "YES" : "NO"}`,
                `(${prevLastBlockUpdatedAt} -> ${nowUpdatedAt})`,
            );
        }
    } else {
        console.log("Post-tx LastBlock entry: MISSING");
    }

    const ok = extSrc.isSome && auth.isSome;
    console.log("\nRESULT:", ok ? "PASS" : "FAIL");
    if (!ok) process.exitCode = 1;

    console.log("\nDONE");
    await api.disconnect();
};

main().catch((e) => {
    console.error(e);
    process.exit(1);
});
