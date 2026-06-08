#!/usr/bin/env node
// Usage: node scan-deposits.js <ASSET_ID> <TRIGGER_BLOCK> [PERIOD=14400] [BATCH_SIZE=50]
// Scans all token deposits for an asset in the lookback window before the trigger block.
// Shows individual deposits, per-recipient totals, and overall total.

import { ApiPromise, WsProvider } from '@polkadot/api';

const ASSET_ID = process.argv[2];
const TRIGGER_BLOCK = parseInt(process.argv[3]);
const PERIOD = parseInt(process.argv[4] || '14400');
const BATCH_SIZE = parseInt(process.argv[5] || '50');

if (!ASSET_ID || !TRIGGER_BLOCK) {
  console.error('Usage: node scan-deposits.js <ASSET_ID> <TRIGGER_BLOCK> [PERIOD=14400] [BATCH_SIZE=50]');
  process.exit(1);
}

const START_BLOCK = TRIGGER_BLOCK - PERIOD;

try {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });

  // Get asset info
  const meta = await api.query.assetRegistry.assets(parseInt(ASSET_ID));
  const assetData = meta.unwrap();
  const decimals = Number(assetData.decimals);
  const symbol = assetData.symbol.toHuman ? assetData.symbol.toHuman() : String(assetData.symbol);
  const xcmRateLimit = assetData.xcmRateLimit?.toString()?.replace(/,/g, '') || '0';
  const limitHuman = Number(BigInt(xcmRateLimit)) / 10 ** decimals;

  console.log(`Scanning blocks ${START_BLOCK} to ${TRIGGER_BLOCK} for ${symbol} (asset ${ASSET_ID}) deposits...`);
  console.log(`Period: ${PERIOD} blocks, batch size: ${BATCH_SIZE}`);
  console.log(`XCM rate limit: ${limitHuman} ${symbol}\n`);

  const deposits = [];

  for (let batchStart = START_BLOCK; batchStart < TRIGGER_BLOCK; batchStart += BATCH_SIZE) {
    const batchEnd = Math.min(batchStart + BATCH_SIZE, TRIGGER_BLOCK + 1);
    const blockNums = [];
    for (let b = batchStart; b < batchEnd; b++) blockNums.push(b);

    const hashes = await Promise.all(blockNums.map(b => api.rpc.chain.getBlockHash(b)));
    const eventsArr = await Promise.all(hashes.map(h => api.query.system.events.at(h)));

    for (let i = 0; i < blockNums.length; i++) {
      for (const record of eventsArr[i]) {
        const { event } = record;
        if (event.section === 'tokens' && event.method === 'Deposited' && event.data[0].toString() === ASSET_ID) {
          deposits.push({
            block: blockNums[i],
            who: event.data[1].toString(),
            amount: BigInt(event.data[2].toString())
          });
        }
      }
    }

    if ((batchStart - START_BLOCK) % 500 === 0) {
      const pct = ((batchStart - START_BLOCK) / PERIOD * 100).toFixed(1);
      process.stderr.write(`Progress: ${pct}% (block ${batchStart}, found ${deposits.length} deposits so far)\n`);
    }
  }

  // Individual deposits
  console.log(`\n=== Found ${deposits.length} ${symbol} deposits in period ===\n`);
  let totalRaw = 0n;
  for (const d of deposits) {
    const human = (Number(d.amount) / 10 ** decimals).toFixed(decimals);
    totalRaw += d.amount;
    console.log(`Block ${d.block}: ${human} ${symbol} -> ${d.who.slice(0, 8)}...${d.who.slice(-6)}`);
  }

  // Per-recipient summary
  const byRecipient = {};
  for (const d of deposits) {
    byRecipient[d.who] = (byRecipient[d.who] || 0n) + d.amount;
  }
  const sorted = Object.entries(byRecipient).sort((a, b) => (b[1] > a[1] ? 1 : -1));

  console.log(`\n=== Per-recipient totals ===\n`);
  for (const [who, raw] of sorted) {
    const human = (Number(raw) / 10 ** decimals).toFixed(decimals);
    console.log(`${who.slice(0, 8)}...${who.slice(-6)}: ${human} ${symbol}`);
  }

  const totalHuman = (Number(totalRaw) / 10 ** decimals).toFixed(decimals);
  console.log(`\nTotal deposited in period: ${totalHuman} ${symbol}`);
  console.log(`XCM rate limit: ${limitHuman} ${symbol}`);
  const excess = Number(totalRaw) / 10 ** decimals - limitHuman;
  if (excess > 0) {
    console.log(`Excess over limit: ${excess.toFixed(decimals)} ${symbol}`);
  }

  await api.disconnect();
} catch (e) {
  console.error('ERROR:', e.message || e);
  process.exit(1);
}
