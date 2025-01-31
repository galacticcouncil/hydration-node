// File: query_asset_state.js
//
// This script queries the tradability state of an asset in the `omnipool` pallet.
// It includes a basic assertion by checking if the asset is tradable (for example).
// Adjust the condition as needed.
//
// If the asset is tradable (depending on what `assetState` returns), it exits with 0.
// Otherwise, it prints an error and exits with 1.

const { ApiPromise, WsProvider } = require('@polkadot/api');

(async () => {
  try {
    const provider = new WsProvider('ws://127.0.0.1:PORT'); // Adjust to the correct RPC port
    const api = await ApiPromise.create({ provider });

    // Replace with your actual asset ID or arguments needed for the storage query
    const assetId = 42; // Example asset ID, change as needed
    const assetState = await api.query.omnipool.assetTradable(assetId);

    console.log(`Fetched asset tradable state for asset ${assetId}: ${assetState.toString()}`);

    // Here we do a simple assertion: Suppose `true` means tradable and `false` means not tradable.
    // Adjust based on what the storage returns.
    if (assetState.isTrue) {
      console.log("Assertion passed: Asset is tradable.");
      process.exit(0);
    } else {
      console.error("Assertion failed: Asset is not tradable as expected.");
      process.exit(1);
    }
  } catch (err) {
    console.error("Error:", err.message);
    process.exit(1);
  }
})();
