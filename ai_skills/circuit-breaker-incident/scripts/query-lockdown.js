#!/usr/bin/env node
// Usage: NODE_PATH=$(npm root -g) node query-lockdown.js <ASSET_ID>
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

  // Find trigger block from Subscan
  const triggerBlock = process.argv[3] ? parseInt(process.argv[3]) : null;
  if (triggerBlock) {
    console.log(`\n=== XCM Origin Search ===`);
    // Check blocks before trigger for HRMP messages
    for (let offset = 1; offset <= 5; offset++) {
      const checkBlock = triggerBlock - offset;
      const hash = await api.rpc.chain.getBlockHash(checkBlock);
      const block = await api.rpc.chain.getBlock(hash);
      const vd = block.block.extrinsics[1].method.args[0].toJSON();
      const hm = vd.horizontalMessages;
      
      for (const [pid, msgs] of Object.entries(hm)) {
        if (msgs.length > 0) {
          const relayBlock = msgs[0].sentAt;
          console.log(`Block ${checkBlock}: Para ${pid} sent ${msgs.length} msg(s), relay block: ${relayBlock}`);
          console.log(`\nSubscan XCM search link:`);
          console.log(`https://hydration.subscan.io/xcm_message?page=1&time_dimension=block&block_start=${relayBlock - 5}&block_end=${relayBlock + 5}`);
        }
      }
    }
  }

  await api.disconnect();
}

main().catch(e => { console.error(e.message); process.exit(1); });
