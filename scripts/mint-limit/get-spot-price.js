#!/usr/bin/env node
// Usage: node get-spot-price.js <ASSET_ID>
// Must be run from hydration-node/scripts/mint-limit/ (needs node_modules with @galacticcouncil/sdk)
// Returns USD spot price using Hydration SDK router (getBestSpotPrice vs USDT asset 10)

import { ApiPromise, WsProvider } from '@polkadot/api';
import { createSdkContext } from '@galacticcouncil/sdk';

const ASSET_ID = process.argv[2];
if (!ASSET_ID) { console.error('Usage: node get-spot-price.js <ASSET_ID>'); process.exit(1); }

const USDT_ID = '10';

try {
  const provider = new WsProvider('wss://rpc.hydradx.cloud');
  const api = await ApiPromise.create({ provider, noInitWarn: true });
  const sdk = await createSdkContext(api);

  const meta = await api.query.assetRegistry.assets(parseInt(ASSET_ID));
  const assetData = meta.unwrap();
  const decimals = Number(assetData.decimals);
  const symbol = assetData.symbol.toHuman ? assetData.symbol.toHuman() : String(assetData.symbol);

  const price = await sdk.api.router.getBestSpotPrice(ASSET_ID, USDT_ID);

  let usdPrice;
  if (price && price.amount !== undefined) {
    usdPrice = parseFloat(price.amount) / 10 ** price.decimals;
  } else {
    usdPrice = ASSET_ID === USDT_ID ? 1.0 : null;
  }

  if (usdPrice) {
    console.log(JSON.stringify({ assetId: ASSET_ID, symbol, decimals, usdPrice }));
  } else {
    console.error(`No spot price found for asset ${ASSET_ID}`);
    process.exit(1);
  }

  sdk.destroy();
  await api.disconnect();
} catch (e) {
  console.error('ERROR:', e.message);
  process.exit(1);
}
