// File: scripts/send_normal_txs.js
//
// Sends multiple normal (non-operational) extrinsics to saturate block weight.
// Adjust the extrinsic and count as needed. For instance, sending small
// balance transfers to self multiple times.

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {
  const args = process.argv.slice(2);
  const countIndex = args.indexOf("--count");
  const count = countIndex !== -1 ? parseInt(args[countIndex+1]) : 100;

  const provider = new WsProvider('ws://127.0.0.1:45589'); // Adjust to your node RPC
  const api = await ApiPromise.create({ provider });

  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');

  for (let i = 0; i < count; i++) {
    const tx = api.tx.balances.transfer(alice.address, 1); // minimal non-operational tx
    await tx.signAndSend(alice, { nonce: -1 });
  }

  console.log(`Sent ${count} normal transactions.`);
  process.exit(0);
})();
