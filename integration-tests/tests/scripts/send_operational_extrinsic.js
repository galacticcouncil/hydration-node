#!/usr/bin/env node
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {
  const RPC_ENDPOINT = 'ws://127.0.0.1:45589'; // Adjust as needed
  const PALLET_NAME = "omnipool";
  const FUNCTION_NAME = "set_asset_tradable_state";
  const TIMEOUT_MS = 30000; // 30s timeout for extrinsic finalization

  try {
    console.log("Connecting to the node...");
    const provider = new WsProvider(RPC_ENDPOINT);
    const api = await ApiPromise.create({ provider });

    // Assert that API is connected
    if (!api.isConnected) {
      console.error("Assertion failed: API is not connected.");
      process.exit(1);
    }
    console.log("Connected successfully to:", RPC_ENDPOINT);

    const keyring = new Keyring({ type: 'sr25519' });
    const alice = keyring.addFromUri('//Alice');

    // Construct operational extrinsic (example)
    const tx = api.tx.transactionPause.pauseTransaction(PALLET_NAME, FUNCTION_NAME);

    console.log("Sending operational extrinsic...");
    let resolved = false;
    const timer = setTimeout(() => {
      if (!resolved) {
        console.error("Assertion failed: Extrinsic did not finalize within the timeout.");
        process.exit(1);
      }
    }, TIMEOUT_MS);

    await new Promise((resolve, reject) => {
      tx.signAndSend(alice, { nonce: -1 }, (result) => {
        const { status, events } = result;

        // If we have a dispatch error, it may appear here, but we also check events.
        if (result.dispatchError) {
          let errMsg = "Unknown error";
          if (result.dispatchError.isModule) {
            const decoded = api.registry.findMetaError(result.dispatchError.asModule);
            errMsg = `${decoded.section}.${decoded.name}: ${decoded.documentation.join(' ')}`;
          } else {
            errMsg = result.dispatchError.toString();
          }
          console.error(`Assertion failed: DispatchError: ${errMsg}`);
          reject(new Error(errMsg));
          return;
        }

        // Check for block inclusion
        if (status.isInBlock) {
          console.log(`Extrinsic included in block: ${status.asInBlock.toHex()}`);
        }

        // Check for finalization and events
        if (status.isFinalized) {
          console.log(`Extrinsic finalized at blockHash: ${status.asFinalized.toHex()}`);

          // Check events for success or failure
          let success = false;
          let failure = false;

          events.forEach(({ event: { data, method, section } }) => {
            if (section === 'system' && method === 'ExtrinsicSuccess') {
              success = true;
            } else if (section === 'system' && method === 'ExtrinsicFailed') {
              failure = true;
              const [error] = data;
              let errMsg = "Unknown error";
              if (error.isModule) {
                const decoded = api.registry.findMetaError(error.asModule);
                errMsg = `${decoded.section}.${decoded.name}: ${decoded.documentation.join(' ')}`;
              } else {
                errMsg = error.toString();
              }
              console.error(`Assertion failed: ExtrinsicFailed: ${errMsg}`);
              reject(new Error(`Extrinsic failed: ${errMsg}`));
              return;
            }
          });

          if (!failure && success) {
            console.log("Operational extrinsic was successfully included and finalized!");
            resolve();
          } else if (!failure && !success) {
            console.error("Assertion failed: No ExtrinsicSuccess event found.");
            reject(new Error("No ExtrinsicSuccess event, extrinsic result unclear."));
          }
        }
      }).catch(err => {
        console.error("Assertion failed: signAndSend threw an error:", err.message);
        reject(err);
      });
    });

    // If we get here, extrinsic was successful
    resolved = true;
    clearTimeout(timer);
    process.exit(0);

  } catch (err) {
    console.error("Error:", err.message);
    process.exit(1);
  }
})();
