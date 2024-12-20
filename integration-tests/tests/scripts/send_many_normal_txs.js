#!/usr/bin/env node
const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {
  const PORT = 45589;        // Adjust this as needed
  const TX_COUNT = 100;      // Number of transactions to send
  const AMOUNT = 1;          // Amount per transfer
  const provider = new WsProvider(`ws://127.0.0.1:${PORT}`);

  console.log("Connecting to the node...");
  const api = await ApiPromise.create({ provider });
  if (!api.isConnected) {
    console.error("Assertion failed: Not connected to the node.");
    process.exit(1);
  }
  console.log("Connected successfully.");

  // Setup keyring and accounts
  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');

  // Query Alice's initial balance
  const { data: initialBalances } = await api.query.system.account(alice.address);
  const initialFree = initialBalances.free.toBn();
  console.log(`Initial Alice balance: ${initialFree.toString()}`);

  // Basic assertion: Alice’s initial balance must be > 0 to send funds
  if (initialFree.isZero()) {
    console.error("Assertion failed: Alice's initial balance is zero, cannot send transactions.");
    process.exit(1);
  }

  console.log(`Sending ${TX_COUNT} normal transactions of ${AMOUNT} each...`);

  for (let i = 0; i < TX_COUNT; i++) {
    const tx = api.tx.balances.transfer(alice.address, AMOUNT);
    // Sign and send, waiting for finalization
    await new Promise((resolve, reject) => {
      let unsub;
      const sendStart = Date.now();

      tx.signAndSend(alice, { nonce: -1 }, (result) => {
        if (result.isError) {
          unsub && unsub();
          console.error("Assertion failed: Extrinsic result indicates an error.");
          reject(new Error("Extrinsic error"));
          return;
        }

        // Once the tx is in a block, we can check for events
        if (result.status.isInBlock) {
          const blockHash = result.status.asInBlock;
          console.log(`Tx #${i + 1} included in block: ${blockHash.toHex()}`);

          // Check events for success and transfer
          const events = result.events;
          let successEventFound = false;
          let transferEventFound = false;

          events.forEach(({ event }) => {
            if (api.events.system.ExtrinsicSuccess.is(event)) {
              successEventFound = true;
            }
            if (api.events.balances.Transfer.is(event)) {
              transferEventFound = true;
            }
          });

          // Assertions
          if (!successEventFound) {
            unsub && unsub();
            console.error(`Assertion failed: No ExtrinsicSuccess event found for tx #${i + 1}.`);
            reject(new Error("No ExtrinsicSuccess event"));
            return;
          }

          if (!transferEventFound) {
            unsub && unsub();
            console.error(`Assertion failed: No Transfer event found for tx #${i + 1}.`);
            reject(new Error("No Transfer event"));
            return;
          }

          // If we reach here, the tx was successful and included required events
          console.log(`Tx #${i + 1} was successful.`);
        } else if (result.status.isFinalized) {
          console.log(`Tx #${i + 1} finalized at blockHash: ${result.status.asFinalized.toHex()}, took ${Date.now() - sendStart}ms`);
          unsub && unsub();
          resolve();
        }
      }).then(u => unsub = u).catch(err => {
        console.error("Assertion failed: signAndSend threw an error.");
        reject(err);
      });
    });
  }

  console.log("All transactions sent and confirmed successful.");

  // Query Alice's final balance
  const { data: finalBalances } = await api.query.system.account(alice.address);
  const finalFree = finalBalances.free.toBn();
  console.log(`Final Alice balance: ${finalFree.toString()}`);

  // Assert final balance changed as expected
  // Since Alice is transferring to herself, we expect fees to reduce her balance.
  // Each transfer costs a transaction fee. We know that the free balance must be less than initialFree
  // after 100 transfers. Let's assert final is strictly less than initial to confirm fees were deducted.
  if (finalFree.gte(initialFree)) {
    console.error("Assertion failed: Final balance is not less than initial balance, no fees were deducted?");
    process.exit(1);
  }

  console.log("Assertions complete. Final state matches expectations. Exiting now.");
  process.exit(0);
})().catch(err => {
  console.error(`Unexpected error: ${err.message}`);
  process.exit(1);
});
