#!/usr/bin/env node
// Usage: node get-trigger-events.cjs <ASSET_ID> <TRIGGER_BLOCK> [WINDOW=2]
// Run from hydration-node/scripts/mint-limit/ so @polkadot/api resolves.
// Dumps the events that matter for a circuit breaker incident — `tokens.Deposited`,
// `tokens.Reserved`, `circuitBreaker.AssetLockdown`, and `messageQueue.Processed` —
// across a small window around the trigger block. Use the output to identify the
// triggering deposit, the excess (Reserved amount), and the XCM origin parachain.

const { ApiPromise, WsProvider } = require('@polkadot/api');

const ASSET_ID = process.argv[2];
const TRIGGER_BLOCK = parseInt(process.argv[3]);
const WINDOW = parseInt(process.argv[4] || '2');

if (!ASSET_ID || !TRIGGER_BLOCK) {
  console.error('Usage: node get-trigger-events.cjs <ASSET_ID> <TRIGGER_BLOCK> [WINDOW=2]');
  process.exit(1);
}

async function main() {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });

  for (let off = -WINDOW; off <= WINDOW; off++) {
    const block = TRIGGER_BLOCK + off;
    const hash = await api.rpc.chain.getBlockHash(block);
    const events = await api.query.system.events.at(hash);
    const relevant = [];
    for (const r of events) {
      const { event } = r;
      const section = event.section;
      const method = event.method;
      if (
        (section === 'circuitBreaker' && method === 'AssetLockdown') ||
        (section === 'tokens' && (method === 'Deposited' || method === 'Reserved') && event.data[0].toString() === ASSET_ID) ||
        (section === 'messageQueue' && method === 'Processed')
      ) {
        relevant.push({ section, method, data: event.data.map(d => d.toString()) });
      }
    }
    if (relevant.length) {
      console.log(`\n=== Block ${block} ===`);
      for (const e of relevant) console.log(`  ${e.section}.${e.method}: ${JSON.stringify(e.data)}`);
    }
  }

  await api.disconnect();
}

main().catch(e => { console.error(e.message); process.exit(1); });
