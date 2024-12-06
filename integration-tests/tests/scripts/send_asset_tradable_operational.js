#!/usr/bin/env node
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {
  const PORT = 45589;         // Change this if needed
  const assetId = 1;          // Replace with the actual asset ID
  const desiredState = true;  // Replace with true/false depending on what you want

  const provider = new WsProvider(`ws://127.0.0.1:${PORT}`);
  const api = await ApiPromise.create({ provider });

  // Create a keyring and add the sudo key (Alice)
  const keyring = new Keyring({ type: 'sr25519' });
  const sudoKey = keyring.addFromUri('//Alice');

  console.log(`Sending operational extrinsic to set asset ${assetId} tradability to ${desiredState}...`);

  // Construct the call to set asset tradable state via the sudo pallet.
  // Adjust the method and arguments if needed.
  const call = api.tx.omnipool.setAssetTradableState(assetId, desiredState);

  // Wrap the call with sudo
  const sudoCall = api.tx.sudo.sudo(call);

  // Sign and send the extrinsic
  const unsub = await sudoCall.signAndSend(sudoKey, async (result) => {
    if (result.status.isInBlock) {
      console.log('Extrinsic included in block...');
    } else if (result.status.isFinalized) {
      console.log(`Extrinsic finalized at block hash ${result.status.asFinalized}.`);

      // At this point, the extrinsic is finalized. Let's verify the state on-chain.
      unsub();

      try {
        // Query the updated state from the chain.
        const assetState = await api.query.omnipool.assetTradable(assetId);
        console.log(`Queried asset ${assetId} state from chain: ${assetState.toString()}`);

        // Perform the assertion. Assuming assetTradable returns a boolean-like value.
        // Adjust this condition based on the actual return type of assetTradable.
        if (assetState.isTrue === desiredState) {
          console.log("Assertion passed: The asset's tradability state matches the desired state.");
          process.exit(0);
        } else {
          console.error("Assertion failed: The asset's tradability state does not match the desired state.");
          process.exit(1);
        }
      } catch (err) {
        console.error("Error querying asset state:", err.message);
        process.exit(1);
      }
    }
  });

})().catch((err) => {
  console.error("Error running script:", err.message);
  process.exit(1);
});
