# onchain-routes

Generate a Technical Committee proposal that registers or updates on-chain router routes for a specific list of asset pairs.

> **Only `addSpecificRoutesProposal.js` is current.** `addRoutesProposal.js` and `addEvmAssetRoutesProposal.js` are deprecated — do not use.

## `addSpecificRoutesProposal.js`

For each `assetIn-assetOut` pair listed in `ASSET_PAIRS`, the script:

1. Asks the Hydration SDK router for the most-liquid route.
2. Builds a `router.setRoute(asset_pair, route)` call.
3. Batches them and wraps the batch in `technicalCommittee.propose`.

Use it whenever you need to register a route for a newly-listed asset, or re-baseline an existing route after liquidity has shifted.

### Setup

```bash
cd scripts/onchain-routes
npm install
```

### Run

1. Edit `ASSET_PAIRS` at the top of the script. Format: `"<assetIn>-<assetOut>"`, e.g. `"0-34"` for HDX → ETH.
2. Run:

   ```bash
   node addSpecificRoutesProposal.js
   ```

3. Copy the printed TC propose HEX and submit it via polkadot.js. A log file `route-processing-specific-<timestamp>.log` is written alongside for review.

`TC_THRESHOLD` and `RPC` are configured at the top of the script. The local RPC line is commented in for fork testing.
