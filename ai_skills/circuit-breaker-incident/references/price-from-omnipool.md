# Price from Omnipool State

Query omnipool asset state for two assets and compute the cross-price via hub reserve ratios.

```javascript
NODE_PATH=$(npm root -g) node -e "
const { ApiPromise, WsProvider } = require('@polkadot/api');
async function main() {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });
  
  const ASSET_ID = 16;  // target asset
  const USDT_ID = 10;   // quote asset (USDT, 6 decimals)
  
  const targetState = (await api.query.omnipool.assets(ASSET_ID)).toJSON();
  const usdtState = (await api.query.omnipool.assets(USDT_ID)).toJSON();
  
  if (targetState && usdtState && targetState.hubReserve && usdtState.hubReserve) {
    const hubPerTargetUnit = Number(BigInt(targetState.hubReserve)) / Number(BigInt(targetState.reserve));
    const hubPerUsdtUnit = Number(BigInt(usdtState.hubReserve)) / Number(BigInt(usdtState.reserve));
    // Adjust for decimal difference: target has TARGET_DECIMALS, USDT has 6
    const price = (hubPerTargetUnit * 10**TARGET_DECIMALS) / (hubPerUsdtUnit * 10**6);
    console.log('Price in USD:', price);
  } else {
    console.log('Asset not in omnipool or state null — try a different quote asset');
  }
  await api.disconnect();
}
main();
"
```

**Note**: If USDT (asset 10) returns null for either side, try DOT (asset 5) as the intermediate / quote asset and convert via DOT/USDT.
