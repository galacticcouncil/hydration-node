---
name: circuit-breaker-incident
description: Investigate Hydration circuit breaker triggers. Use when snakewatch reports an asset lockdown, circuit breaker alert, or when asked to analyze why an asset was locked on Hydration chain. Covers XCM deposit fuse (issuance increase), trade volume limits, and liquidity limits.
---

# Circuit Breaker Incident Response

## Scripts

All scripts are in `scripts/` and use ESM (`import`). Run with `node <script>`.

| Script | Purpose | Usage |
|---|---|---|
| `query-lockdown.js` | Check lockdown state + trace XCM origin | `NODE_PATH=$(npm root -g) node query-lockdown.js <ASSET_ID> [TRIGGER_BLOCK]` |
| `get-spot-price.js` | USD spot price via Hydration SDK | `node get-spot-price.js <ASSET_ID>` (run from `hydration-node/scripts/mint-limit/`) |
| `scan-deposits.js` | Scan all deposits in lookback period | `node scan-deposits.js <ASSET_ID> <TRIGGER_BLOCK> [PERIOD=14400] [BATCH_SIZE=50]` |
| `generate-tc-unlock.js` | Generate TC proposal hex to lift lockdown + raise limit | `node generate-tc-unlock.js <ASSET_ID> <NEW_LIMIT_HUMAN> [TC_THRESHOLD=4]` |

**Note**: `query-lockdown.js` uses CommonJS (`require`). The others use ESM (`import`). Both `scan-deposits.js` and `generate-tc-unlock.js` auto-fetch asset decimals/symbol from chain.

## Quick Response Workflow

When a circuit breaker alert comes in (e.g. from snakewatch):

1. **Extract from alert**: asset name, asset ID, locked-until block
2. **Find the trigger block and event** via Subscan API
3. **Get asset details** from chain (decimals, xcm_rate_limit)
4. **Calculate amounts** in human-readable units and USD
5. **Trace the XCM origin** if deposit-triggered
6. **Report findings** with Subscan links

## Known Gotchas

- **ESM vs CommonJS**: `scripts/mint-limit/` has `"type": "module"` in package.json. Scripts using `require()` (like `query-lockdown.js`) must be saved as `.cjs`. Scripts using `import` (like `get-spot-price.js`) work as `.js`.
- **Subscan API key required**: All API endpoints return 403 without `X-API-Key` header. Use chain-direct queries as fallback.
- **Spot price script can fail silently**: `get-spot-price.js` may fail for assets without good liquidity routes. Always check exit code and use CoinGecko fallback.

## Step 1: Find the Lockdown Event

Query Subscan for the most recent `AssetLockdown` event:

```bash
curl -s -X POST 'https://hydration.api.subscan.io/api/v2/scan/events' \
  -H 'Content-Type: application/json' \
  -d '{"module":"circuitbreaker","event_id":"AssetLockdown","page":0,"row":5}'
```

Then get event params (asset_id, until block):

```bash
curl -s -X POST 'https://hydration.api.subscan.io/api/scan/event' \
  -H 'Content-Type: application/json' \
  -d '{"event_index":"<BLOCK>-<EVENT_IDX>"}'
```

## Step 2: Get All Events in Trigger Block

```bash
curl -s -X POST 'https://hydration.api.subscan.io/api/v2/scan/events' \
  -H 'Content-Type: application/json' \
  -d '{"block_num":<BLOCK>,"page":0,"row":100}'
```

Look for the sequence: `messageQueue.Processed` → `tokens.Deposited` → `circuitbreaker.AssetLockdown`. The `messageQueue.Processed` event has the XCM origin (e.g. `Sibling: 2004` = Moonbeam).

## Step 3: Query Asset Details from Chain

Use `@polkadot/api` (installed globally):

```javascript
NODE_PATH=$(npm root -g) node -e "
const { ApiPromise, WsProvider } = require('@polkadot/api');
async function main() {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });
  const asset = await api.query.assetRegistry.assets(ASSET_ID);
  console.log(JSON.stringify(asset.toHuman(), null, 2));
  const lockdown = await api.query.circuitBreaker.assetLockdownState(ASSET_ID);
  console.log('Lockdown:', JSON.stringify(lockdown.toHuman(), null, 2));
  await api.disconnect();
}
main();
"
```

Key fields from asset registry:
- `decimals` — for converting raw amounts
- `xcmRateLimit` — the deposit limit that triggers lockdown (issuance fuse)
- `symbol` — human-readable name

## Step 4: Calculate Amounts

```python
deposit_raw = <from tokens.Deposited event>
limit_raw = <xcmRateLimit from registry>
decimals = <from registry>

deposit = deposit_raw / 10**decimals
limit = limit_raw / 10**decimals
excess = deposit - limit
```

For USD value, use the **Hydration SDK spot price** (preferred — on-chain, accurate):

```bash
cd hydration-node/scripts/mint-limit && node get-spot-price.js <ASSET_ID> 2>/dev/null
```

This calls `sdk.api.router.getBestSpotPrice(assetId, '10')` where `'10'` is USDT.
Returns JSON: `{"assetId":"16","symbol":"GLMR","decimals":18,"usdPrice":0.0147}`

**Note**: The script lives in both `skills/circuit-breaker-incident/scripts/` and `hydration-node/scripts/mint-limit/`. Must run from `mint-limit/` dir (needs its `node_modules` with `@galacticcouncil/sdk`). Use `2>/dev/null` to suppress noisy polkadot disconnect logs.

**Fallback**: CoinGecko API (see CoinGecko IDs table below):
```
https://api.coingecko.com/api/v3/simple/price?ids=<COINGECKO_ID>&vs_currencies=usd
```

**Note**: `get-spot-price.js` can fail silently for some assets (e.g. EURC). Always check exit code and fall back to CoinGecko if needed.

## Step 5: Find the XCM Message Link

The `messageQueue.Processed` event gives the origin parachain and message hash, but the critical deliverable is the **Subscan XCM message link** showing the full cross-chain trace.

### 5a: Get the relay chain block number

The XCM was received as an HRMP message in the block **before** the trigger block. Query it:

```javascript
NODE_PATH=$(npm root -g) node -e "
const { ApiPromise, WsProvider } = require('@polkadot/api');
async function main() {
  const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });
  const hash = await api.rpc.chain.getBlockHash(TRIGGER_BLOCK - 1);
  const block = await api.rpc.chain.getBlock(hash);
  const vd = block.block.extrinsics[1].method.args[0].toJSON();
  const hm = vd.horizontalMessages;
  for (const [pid, msgs] of Object.entries(hm)) {
    if (msgs.length > 0) {
      console.log('Para', pid, ':', msgs.length, 'msgs, sentAt:', msgs[0].sentAt);
    }
  }
  await api.disconnect();
}
main();
"
```

Look for the origin parachain (from Step 2). The `sentAt` value is the **relay chain block number**.

**Note**: If no messages found at `TRIGGER_BLOCK - 1`, check `TRIGGER_BLOCK - 2` through `TRIGGER_BLOCK - 5`. The message queue may process with a delay.

### 5b: Construct the Subscan XCM message search URL

Use the relay block with ±5 buffer (Subscan's filter needs some range):

```
https://hydration.subscan.io/xcm_message?page=1&time_dimension=block&block_start=<RELAY_BLOCK-5>&block_end=<RELAY_BLOCK+5>
```

Example: relay block 29959078 →
`https://hydration.subscan.io/xcm_message?page=1&time_dimension=block&block_start=29959073&block_end=29959083`

The matching XCM message in the results links to the full trace (e.g. `/xcm_message/polkadot-xxx`).

### Important notes
- Subscan XCM message API (`api/scan/xcm/messages`) is **paywalled (402)**. Use UI links.
- Subscan UI has **Cloudflare protection** — `web_fetch`/`curl` won't work. Provide links to user.
- The `messageQueue.Processed` event's `id` hash is **NOT searchable** on Subscan.

## Step 6: Report Template

```
Circuit breaker triggered for <SYMBOL> (asset <ID>).
<AMOUNT> <SYMBOL> (~$<USD>) deposited via XCM from <ORIGIN_CHAIN>.
Mint limit: <LIMIT> <SYMBOL> (~$<USD>). Excess: <EXCESS> (~$<USD>).
Asset locked until block <BLOCK> (~<HOURS>h).

XCM message search: <SUBSCAN_XCM_SEARCH_LINK>
Block events: https://hydration.subscan.io/block/<TRIGGER_BLOCK>?tab=event
```

## Circuit Breaker Types

Three fuse types can trigger lockdown:

| Fuse | What triggers it | Key storage |
|---|---|---|
| **Issuance (deposit) fuse** | XCM deposit exceeds `xcmRateLimit` per period | `AssetLockdownState` |
| **Trade volume limit** | Net trade volume exceeds % of pool reserve per block | `TradeVolumeLimitPerAsset` |
| **Liquidity limit** | Add/remove liquidity exceeds % limit per block | `LiquidityAddLimitPerAsset` / `LiquidityRemoveLimitPerAsset` |

Most common trigger: **issuance fuse** from large XCM bridge transfers.

**Two trigger patterns:**
1. **Single large deposit** — one XCM deposit exceeds the limit (e.g. GLMR 6.9M > 4.3M limit)
2. **Cumulative period breach** — multiple small deposits over the period cumulatively exceed the limit. The triggering deposit may be tiny (e.g. jitoSOL: 38 jitoSOL triggered it but period total exceeded 2,777 limit). Check `tokens.Reserved` amount vs `tokens.Deposited` to distinguish.

If `tokens.Deposited` amount < `xcmRateLimit`, it's a cumulative trigger.

## Quick Trigger Block Calculation

`trigger_block ≈ locked_until_block - 14400` (default lockdown period is 14400 blocks)

Verify by matching against Subscan's `AssetLockdown` events list.

## Key Parachain IDs

| Chain | Para ID |
|---|---|
| Hydration | 2034 |
| Asset Hub | 1000 |
| Moonbeam | 2004 |
| Astar | 2006 |
| Acala | 2000 |
| Interlay | 2032 |
| Bifrost | 2030 |
| Centrifuge | 2031 |

## Subscan API Notes

**⚠️ All Subscan API endpoints now require an API key (HTTP 403 without one).** Store in `SUBSCAN_API_KEY` env var and pass as `-H "X-API-Key: $SUBSCAN_API_KEY"`. If unavailable, use chain-direct queries as fallback (see "Chain-Direct Fallback" section below).

- Events API: `https://hydration.api.subscan.io/api/v2/scan/events` — free
- Event detail: `https://hydration.api.subscan.io/api/scan/event` — free
- Extrinsic detail: `https://hydration.api.subscan.io/api/scan/extrinsic` — free
- XCM messages: `https://hydration.api.subscan.io/api/scan/xcm/messages` — **paywalled (402)**
- Subscan UI has Cloudflare protection — `web_fetch` won't work, provide links to user instead

## Chain-Direct Fallback: Scanning for Deposits

When Subscan API is unavailable, or to analyze **cumulative triggers** (where you need to find all deposits in the 14,400-block period), scan chain events directly:

```javascript
// Batch-scan pattern: query 50 blocks in parallel for tokens.Deposited events
import { ApiPromise, WsProvider } from '@polkadot/api';

const TRIGGER_BLOCK = <trigger_block>;
const PERIOD = 14400;
const START_BLOCK = TRIGGER_BLOCK - PERIOD;
const ASSET_ID = '<asset_id>';
const BATCH_SIZE = 50;

const api = await ApiPromise.create({ provider: new WsProvider('wss://rpc.hydradx.cloud'), noInitWarn: true });

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
}
```

This takes ~3-5 minutes for the full 14,400-block window. Group results by recipient to identify the main depositors.

## CoinGecko IDs for Common Hydration Assets

| Asset (ID) | CoinGecko ID |
|---|---|
| HDX (0) | `hydradx` |
| DOT (5) | `polkadot` |
| USDT (10) | `tether` |
| GLMR (16) | `moonbeam` |
| ASTR (9) | `astar` |
| CFG (41) | `centrifuge` |
| BNC (14) | `bifrost-native-coin` |
| jitoSOL (40) | `jito-staked-sol` |
| IBTC (11) | `interbtc` |
| INTR (12) | `interlay` |
| EURC (44) | `euro-coin` |

## Past Incidents (Reference)

### GLMR (16) — Block 11375067 (Feb 2026)
- Single large deposit: 6,900,001 GLMR (~$101K) from Moonbeam (para 2004)
- Limit: 4,295,059 GLMR. Excess: 2,604,942 GLMR
- Relay block: 29959078

### jitoSOL (40) — Block 10954824
- Cumulative trigger: 38.31 jitoSOL deposit was the straw, period total exceeded 2,777 limit
- Origin: Moonbeam (para 2004). Relay block: 29525807

### CFG (41) — Block 10536839
- Single large deposit: 1,168,803 CFG (~$104K) from Asset Hub (para 1000)
- Limit: 725,000 CFG. Excess: 443,803 CFG
- Relay block: 29100043

### EURC (44) — Block 11786380 (Mar 2026)
- Cumulative trigger: 23 deposits totaling 221,234 EURC over period, limit was 200,000 EURC
- Triggering deposit: 34,751 EURC (~$39,964). All deposits from Moonbeam (para 2004)
- Three main recipients accounted for 98% of volume (~$250K total)
- Relay block: 30429186. Limit raised to 800,000 EURC via TC proposal

## Lifting Lockdown

If the deposit is legitimate and lockdown needs lifting early, a Technical Committee proposal is required.

In most cases, you'll want to **batch two calls** in a single TC proposal:
1. `circuitBreaker.forceLiftLockdown(assetId)` — immediately lifts the lockdown
2. `assetRegistry.update(assetId, ..., xcmRateLimit)` — raises the mint limit to prevent re-trigger

Pattern:
```javascript
// 1. Force lift lockdown
const forceLiftCall = api.tx.circuitBreaker.forceLiftLockdown(ASSET_ID);

// 2. Update xcmRateLimit (e.g. 800k EURC = 800_000 * 10^decimals)
const updateCall = api.tx.assetRegistry.update(
    ASSET_ID,
    null, null, null,           // name, asset_type, existential_deposit
    NEW_LIMIT.toString(),       // xcm_rate_limit
    null, null, null, null      // is_sufficient, symbol, decimals, location
);

// 3. Batch and wrap in TC propose
const batch = api.tx.utility.batchAll([forceLiftCall, updateCall]);
const lengthBound = batch.method.encodedLength ?? batch.method.toU8a().length;
const tcProposal = api.tx.technicalCommittee.propose(TC_THRESHOLD, batch.method, lengthBound);

console.log('HEX:', tcProposal.method.toHex());
```

See `scripts/mint-limit/eurc-lockdown-proposal.js` as a complete template, or `scripts/mint-limit/liftLockdown.js` for lift-only proposals.
