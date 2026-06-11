#!/usr/bin/env node
// Usage: node query-lockdown.cjs <ASSET_ID> [TRIGGER_BLOCK]
// Run from hydration-node/scripts/mint-limit/ so @polkadot/api resolves.
// Queries circuit breaker lockdown state, asset details, and finds the XCM relay block
// for constructing the Subscan XCM message search link.

const { ApiPromise, WsProvider } = require('@polkadot/api');

const ASSET_ID = parseInt(process.argv[2] || '0');
if (!ASSET_ID) { console.error('Usage: node query-lockdown.js <ASSET_ID>'); process.exit(1); }

async function main() {
  const api = await ApiPromise.create({
    provider: new WsProvider('wss://rpc.hydradx.cloud'),
    noInitWarn: true
  });

  const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  const asset = (await api.query.assetRegistry.assets(ASSET_ID)).toHuman();
  const lockdown = (await api.query.circuitBreaker.assetLockdownState(ASSET_ID)).toHuman();

  console.log(`\n=== Asset ${ASSET_ID} (${asset?.symbol || 'unknown'}) ===`);
  console.log(`Decimals: ${asset?.decimals}`);
  console.log(`XCM Rate Limit (raw): ${asset?.xcmRateLimit}`);
  console.log(`Is Sufficient: ${asset?.isSufficient}`);
  console.log(`\nLockdown State: ${JSON.stringify(lockdown)}`);
  console.log(`Current Block: ${currentBlock}`);

  if (lockdown?.Locked) {
    const until = parseInt(lockdown.Locked.replace(/,/g, ''));
    const remaining = until - currentBlock;
    const hoursRemaining = (remaining * 6) / 3600;
    console.log(`Locked until: ${until}`);
    console.log(`Blocks remaining: ${remaining}`);
    console.log(`~Hours remaining: ${hoursRemaining.toFixed(1)}h`);
  }

  // Relay-block window in which the triggering XCM was sent.
  // hrmpWatermark advances to the relay block up to which HRMP messages have been consumed.
  // The "tight" sent-at range is (prevWatermark, triggerWatermark], but Subscan's xcm_message
  // index appears fuzzy on the relay-block filter (or the message's sentAt can sit earlier than
  // the watermark advance), so widen by ±10 for the search URL.
  const triggerBlock = process.argv[3] ? parseInt(process.argv[3]) : null;
  if (triggerBlock) {
    console.log(`\n=== XCM Relay Block Lookup ===`);
    const triggerHash = await api.rpc.chain.getBlockHash(triggerBlock);
    const prevHash = await api.rpc.chain.getBlockHash(triggerBlock - 1);
    const triggerWatermark = (await (await api.at(triggerHash)).query.parachainSystem.hrmpWatermark()).toNumber();
    const prevWatermark = (await (await api.at(prevHash)).query.parachainSystem.hrmpWatermark()).toNumber();
    const buffer = 10;
    const searchStart = prevWatermark - buffer;
    const searchEnd = triggerWatermark + buffer;
    console.log(`hrmpWatermark: ${prevWatermark} (block ${triggerBlock - 1}) → ${triggerWatermark} (block ${triggerBlock})`);
    console.log(`Tight sent-at window: (${prevWatermark}, ${triggerWatermark}]`);
    console.log(`\nBlock events:`);
    console.log(`https://hydration.subscan.io/block/${triggerBlock}?tab=event`);
    console.log(`\nSubscan XCM search link (widened by ±${buffer} for fuzzy relay-block matching):`);
    console.log(`https://hydration.subscan.io/xcm_message?page=1&time_dimension=block&block_start=${searchStart}&block_end=${searchEnd}`);
  }

  await api.disconnect();
}

main().catch(e => { console.error(e.message); process.exit(1); });
