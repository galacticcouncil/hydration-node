# Gas Estimation Test - Issue #1133

Reproduces the `eth_estimateGas` overestimation bug on HydraDX and verifies the
`rpc-binary-search-estimate` fix.

**Issue:** https://github.com/galacticcouncil/hydration-node/issues/1133

## The problem

When a contract makes multiple external calls (like DecentralPool's `deposit()`),
EIP-150's 63/64th gas rule causes each subcall to forward nearly all remaining gas.
Without binary search, `eth_estimateGas` runs the call at the block gas limit and
reports the total gas *forwarded* — not what was actually consumed. This inflates
estimates to ~54M (183x actual) instead of ~297K.

Wallets like Talisman use `eth_estimateGas` to set the transaction gas limit. When
the estimate exceeds the block gas limit (60M), the node rejects the transaction
entirely and users cannot interact with the contract.

## The fix

Enable the `rpc-binary-search-estimate` feature flag on `fc-rpc` in the workspace
root `Cargo.toml`:

```diff
- fc-rpc = { git = "...", default-features = false }
+ fc-rpc = { git = "...", default-features = false, features = ["rpc-binary-search-estimate"] }
```

This makes `eth_estimateGas` perform a binary search to find the minimum gas that
allows the transaction to succeed, returning a tight estimate close to actual usage.

## Prerequisites

- **hydradx binary:** `cargo build --release` (from repo root)
- **polkadot binary:** built at `../polkadot-sdk/target/release/polkadot` (relative to repo root)
- **zombienet:** installed and on PATH (`npm i -g @nicedotfun/zombienet`)
- **Node.js:** v18+

## Step-by-step reproduction

### 1. Build the node

From the repo root:

```bash
cargo build --release
```

### 2. Build the chain spec

```bash
cd scripts/gas-estimation-test
npm install
bash scripts/build-chainspec.sh
```

This generates `chainspec-raw.json` with:
- WETH (asset 20) registered with 18 decimals
- Test accounts pre-funded with WETH
- `Parameters::IsTestnet = true` (enables 1-block governance periods)
- EVM deployer address whitelisted

### 3. Start zombienet

```bash
cd scripts/gas-estimation-test
zombienet spawn zombienet.json
```

Wait for the parachain to start producing blocks (~60-90 seconds). You'll see
log lines like `Prepared block for proposing at N`.

Keep this terminal running.

### 4. Run chain setup

In a new terminal:

```bash
cd scripts/gas-estimation-test
npm run setup
```

This submits a GeneralAdmin referendum (passes in ~1 block thanks to testnet mode)
that:
- Sets the WETH XCM location (so `WethAssetId` resolves to asset 20)
- Adds WETH as an accepted fee currency (so EVM fee withdrawal works)

Expected output ends with:
```
  Setup complete! EVM is ready.
```

### 5. Run the gas estimation test

```bash
npm run test:gas
```

This deploys a `DepositProxy` contract with 6 external calls per `deposit()`,
then compares `eth_estimateGas` results with actual gas usage.

#### Without the fix (default)

```
  Actual gas used:       297,192
  Estimate (no limit):   54,347,670  (182.9x actual)

  STATUS: rpc-binary-search-estimate is NOT enabled
```

#### With the fix

After enabling `rpc-binary-search-estimate` in `Cargo.toml`, rebuild, restart
zombienet, re-run setup + test:

```
  Actual gas used:       297,192
  Estimate (no limit):   ~300,000    (1.0x actual)

  STATUS: rpc-binary-search-estimate IS enabled
  The fix is working as expected!
```

## Files

| File | Purpose |
|------|---------|
| `contracts/DepositProxy.sol` | Proxy contract with 6 external calls (mimics DecentralPool) |
| `zombienet.json` | Zombienet config: 4 relay validators, 1 parachain collator |
| `scripts/build-chainspec.sh` | Builds patched raw chain spec with WETH + testnet mode |
| `scripts/patch-chainspec.js` | Patches plain chain spec (assets, balances, TC, parachain ID) |
| `scripts/setup-chain.js` | Sets WETH location + accepted currency via referendum |
| `scripts/test-gas-estimation.js` | Deploys contracts, estimates gas, compares, diagnoses |
| `hardhat.config.js` | Hardhat config (chainId 2222222, RPC at localhost:9999) |

## npm scripts

| Command | Description |
|---------|-------------|
| `npm run setup` | Run chain setup (step 4) |
| `npm run test:gas` | Run the gas estimation test (step 5) |
| `npm run compile` | Compile Solidity contracts |

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WS_URL` | `ws://127.0.0.1:9999` | Substrate WebSocket RPC (for setup-chain.js) |
| `EVM_RPC_URL` | `http://127.0.0.1:9999` | EVM JSON-RPC endpoint (for Hardhat) |

## Troubleshooting

**"Parameters::IsTestnet is false"** — The chain spec doesn't have the testnet
flag. Re-run `bash scripts/build-chainspec.sh` and restart zombienet.

**"exceeds block gas limit" on contract deploy** — The gas estimation bug itself.
The test uses explicit `gasLimit` on deploys to work around this. If you see this
on the `deposit()` call, the `gasLimit: 500_000` override may need increasing.

**EVM transactions not included in blocks** — Check that setup completed
successfully (WETH must be an accepted fee currency). Without it, the EVM fee
withdrawal fails silently and transactions are dropped from the pool.
