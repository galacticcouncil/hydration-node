import {signFakeWithApi} from '@acala-network/chopsticks-utils';
import {ApiPromise, WsProvider} from "@polkadot/api";

// Bifrost sovereign account: sibling:2030
// Hex: 7369626cee070000000000000000000000000000000000000000000000000000
const BIFROST_SOVEREIGN = "7LCt6dFs6sraSg31uKfbRH7soQ66GRb3LAkGZJ1ie3369crq";

// "bifrosto" as hex
const BIFROST_SOURCE = "0x626966726f73746f";

// Raw storage keys (precomputed from twox128/twox64Concat hashes)
// ExternalSources: twox128("EmaOracle") ++ twox128("ExternalSources") ++ twox64Concat("bifrosto")
const EXTERNAL_SOURCES_KEY = "0x5258a12472693b34a3ed25509781e55f62dd2a081ec5b2cc6c15dfa73f370fe7c7e860d98b04912c626966726f73746f";
// AuthorizedAccounts: twox128("EmaOracle") ++ twox128("AuthorizedAccounts") ++ twox64Concat("bifrosto") ++ twox64Concat(bifrost_sovereign_hex)
const AUTHORIZED_ACCOUNTS_KEY = "0x5258a12472693b34a3ed25509781e55fe1be1943a3ea7bb89bd8ed79a8155ea9c7e860d98b04912c626966726f73746fc31effe0f5c540f27369626cee070000000000000000000000000000000000000000000000000000";

const main = async () => {
    const uri = "ws://127.0.0.1:8000";
    const provider = new WsProvider(uri);

    // Register custom signed extensions so extrinsics encode correctly
    const api = await ApiPromise.create({
        provider,
        signedExtensions: {
            ValidateClaim: {extrinsic: {}, payload: {}},
            CheckMetadataHash: {extrinsic: {mode: "u8"}, payload: {}},
            StorageWeightReclaim: {extrinsic: {}, payload: {}},
        },
    });

    // Step 1: Inject ExternalSources and AuthorizedAccounts via raw storage keys.
    // These storage items are not in the on-chain metadata yet, so we use raw keys.
    console.log("Injecting ExternalSources and AuthorizedAccounts storage...");
    await provider.send("dev_setStorage", [[
        [EXTERNAL_SOURCES_KEY, "0x00"],
        [AUTHORIZED_ACCOUNTS_KEY, "0x00"],
    ]]);
    await provider.send("dev_newBlock", [{}]);

    // Step 2: Set Instant block build mode so the extrinsic gets included
    // in a new block automatically upon submission.
    await provider.send("dev_setBlockBuildMode", ["Instant"]);

    // Step 3: Query asset registry for asset 15 XCM location
    const asset15Location = await api.query.assetRegistry.assetLocations(15);
    if (asset15Location.isNone) {
        console.error("Asset 15 has no XCM location registered");
        process.exit(1);
    }
    const asset15Loc = asset15Location.unwrap();
    console.log("Asset 15 location:", JSON.stringify(asset15Loc.toJSON(), null, 2));

    // DOT location: Location::parent() = { parents: 1, interior: "Here" }
    const dotLocation = {V4: {parents: 1, interior: "Here"}};
    const asset15VersionedLocation = {V4: asset15Loc.toJSON()};

    // Price: (numerator, denominator) — e.g. 1 DOT = 50 units of asset 15
    const price = [50_000_000_000_000, 1_000_000_000_000];

    // Step 4: Build and fake-sign the extrinsic as Bifrost sovereign
    const tx = api.tx.emaOracle.updateBifrostOracle(
        dotLocation,
        asset15VersionedLocation,
        price,
    );

    console.log("Fake-signing as Bifrost sovereign:", BIFROST_SOVEREIGN);
    await signFakeWithApi(api, tx, BIFROST_SOVEREIGN);

    // Step 5: Submit and wait for inclusion
    console.log("Submitting oracle update extrinsic...");
    await new Promise((resolve, reject) => {
        tx.send((result) => {
            console.log("Status:", result.status.type);
            if (result.status.isInBlock) {
                console.log("Included in block:", result.status.asInBlock.toHex());
                resolve(result);
            }
        }).catch(reject);
    });

    console.log("\nDONE");
    await api.disconnect();
};

main().catch((e) => {
    console.error(e);
    process.exit(1);
});
