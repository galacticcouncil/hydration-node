#!/usr/bin/env node
// Usage: node generate-tc-unlock.js <ASSET_ID> <NEW_XCM_RATE_LIMIT_HUMAN> [TC_THRESHOLD]
// Example: node generate-tc-unlock.js 44 800000 4
// Generates a TC proposal hex that batches forceLiftLockdown + updateXcmRateLimit.
// NEW_XCM_RATE_LIMIT_HUMAN is in human units (e.g. 800000 for 800k EURC).

import { ApiPromise, WsProvider } from '@polkadot/api';

const ASSET_ID = parseInt(process.argv[2]);
const NEW_LIMIT_HUMAN = parseFloat(process.argv[3]);
const TC_THRESHOLD = parseInt(process.argv[4] || '4');

if (!ASSET_ID || !NEW_LIMIT_HUMAN) {
  console.error('Usage: node generate-tc-unlock.js <ASSET_ID> <NEW_XCM_RATE_LIMIT_HUMAN> [TC_THRESHOLD=4]');
  process.exit(1);
}

try {
  const provider = new WsProvider('wss://rpc.hydradx.cloud');
  const api = await ApiPromise.create({ provider, noInitWarn: true });

  // Get asset decimals
  const meta = await api.query.assetRegistry.assets(ASSET_ID);
  const assetData = meta.unwrap();
  const decimals = Number(assetData.decimals);
  const symbol = assetData.symbol.toHuman ? assetData.symbol.toHuman() : String(assetData.symbol);
  const currentLimit = assetData.xcmRateLimit?.toString();

  const newLimitRaw = BigInt(Math.round(NEW_LIMIT_HUMAN * 10 ** decimals));

  console.log(`\n=== TC Unlock Proposal for ${symbol} (asset ${ASSET_ID}) ===`);
  console.log(`Decimals: ${decimals}`);
  console.log(`Current XCM rate limit: ${currentLimit} (${Number(BigInt(currentLimit?.replace(/,/g, '') || '0')) / 10 ** decimals})`);
  console.log(`New XCM rate limit: ${newLimitRaw} (${NEW_LIMIT_HUMAN})`);
  console.log(`TC threshold: ${TC_THRESHOLD}`);

  // 1. Force lift lockdown
  const forceLiftCall = api.tx.circuitBreaker.forceLiftLockdown(ASSET_ID);

  // 2. Update xcmRateLimit
  const updateCall = api.tx.assetRegistry.update(
    ASSET_ID,
    null, null, null,
    newLimitRaw.toString(),
    null, null, null, null
  );

  // 3. Batch both calls
  const batch = api.tx.utility.batchAll([forceLiftCall, updateCall]);
  const lengthBound = batch.method.encodedLength ?? batch.method.toU8a().length;

  // 4. Wrap in TC propose
  const tcProposal = api.tx.technicalCommittee.propose(TC_THRESHOLD, batch.method, lengthBound);

  console.log('\n--- forceLiftLockdown ---');
  console.log(JSON.stringify(forceLiftCall.method.toHuman(), null, 2));

  console.log('\n--- assetRegistry.update ---');
  console.log(JSON.stringify(updateCall.method.toHuman(), null, 2));

  console.log('\n--- batchAll (human) ---');
  console.log(JSON.stringify(batch.method.toHuman(), null, 2));

  console.log('\n--- TC propose HEX (submit via polkadot.js) ---');
  console.log(tcProposal.method.toHex());

  console.log(`\n--- Length bound: ${lengthBound} ---`);

  await api.disconnect();
} catch (e) {
  console.error('ERROR:', e.message || e);
  process.exit(1);
}
