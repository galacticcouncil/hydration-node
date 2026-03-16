# PEPL Worker

**Protocol Executed Partial Liquidation** worker for the Hydration money market (Aave v3 fork).

Monitors borrower health factors on every block and identifies (or executes) liquidation opportunities. Runs in two modes:

| Mode | Purpose | Feature flag |
|------|---------|-------------|
| **Node** | Embedded in the Hydration collator. Submits unsigned liquidation extrinsics via the transaction pool. | `node` |
| **Standalone** | Connects to any Hydration RPC endpoint. Reports liquidation opportunities (default) or submits them (`--submit`). | `standalone` |

## Architecture

```
pepl-worker/
  src/
    lib.rs              # Public API (run_worker, traits, config)
    worker.rs           # Generic liquidation loop — trait-based, no Substrate deps
    traits.rs           # BlockSource, TxSubmitter, OracleSource, DryRunner
    oracle.rs           # DIA oracle transaction parsing
    config.rs           # WorkerConfig, constants

    node/               # Node-mode adapters (feature = "node")
      mod.rs            # ApiProvider, NodeDryRunner, event helpers, LiquidationTaskData
      runner.rs         # LiquidationTask::run() — async orchestration
      block_source.rs   # NodeBlockSource (import_notification_stream)
      tx_submitter.rs   # NodeTxSubmitter (transaction_pool.submit_one)
      mempool.rs        # NodeMempoolMonitor (oracle tx interception)
      rpc.rs            # Liquidation RPC API (getBorrowers, isRunning, etc.)

    standalone/         # Standalone-mode adapters (feature = "standalone")
      mod.rs
      main.rs           # hydra-liquidator binary entry point
      rpc_provider.rs   # RpcState — RuntimeApiProvider via eth_call / state_call
      rpc_block_source.rs  # RpcBlockSource (chain_subscribeNewHeads + eth_getLogs)
      report_submitter.rs  # ReportSubmitter (dry-run logger)
      oracle_injector.rs   # OracleInjector (scenario testing from JSON)
      types.rs          # Stub runtime types for standalone compilation
```

The core worker loop (`worker.rs`) is generic over environment traits — it knows nothing about Substrate clients, RPC, or transaction pools. Each mode provides concrete implementations of these traits.

## Standalone mode

### Build

```bash
cargo build --release -p pepl-worker --features standalone
```

The binary is `target/release/hydra-liquidator`.

### Basic usage — report mode (default)

Connect to mainnet and report liquidation opportunities on every block:

```bash
hydra-liquidator --rpc-url wss://rpc.hydradx.cloud
```

Output:
```
[pepl-worker] Fetched 510 borrowers
[pepl-worker] Money market initialized: 19 reserves
[pepl-worker] Listening for new blocks...
[pepl-worker] block 7234567: starting LiquidateAll (46 borrowers to scan, 510 total)
[pepl-worker] block 7234567: [DRY-RUN] would liquidate user 0xabc...
    collateral: 5, debt: 10, amount: 1234560000000000000
[pepl-worker] block 7234567: LiquidateAll completed in 2841ms
```

### Run against a local fork (Chopsticks)

```bash
# Terminal 1: start Chopsticks
npx @acala-network/chopsticks@latest --config=hydradx

# Terminal 2: run the liquidator
hydra-liquidator --rpc-url ws://localhost:8000
```

### Submit real liquidation transactions

```bash
hydra-liquidator --rpc-url wss://rpc.hydradx.cloud --submit
```

When `--submit` is passed, the worker sends unsigned `pallet_liquidation::liquidate` extrinsics. Without it, all liquidations are logged but never submitted.

### CLI options

| Flag | Default | Description |
|------|---------|-------------|
| `--rpc-url` | `wss://rpc.hydradx.cloud` | WebSocket RPC URL of the Hydration node |
| `--omniwatch-url` | Omniwatch production URL | API endpoint for initial borrower list |
| `--submit` | `false` | Actually submit liquidation transactions |
| `--max-liquidations` | `10` | Maximum liquidations per block |
| `--target-hf` | `1.001` | Target health factor after liquidation |
| `--hf-threshold` | `1.1` | Skip borrowers with cached HF above this (saves RPC calls). Set to `0` to scan all. |
| `--oracle-scenario` | none | JSON file with simulated oracle price updates |
| `--oracle-persist` | `false` | Re-apply oracle scenario prices after every MM re-init |
| `--no-interrupt` | `false` | Complete full scan even if new block arrives mid-scan |
| `--pap-contract` | production address | PoolAddressesProvider contract address (hex) |

### Oracle scenario testing

Simulate a price crash and see what would get liquidated:

```bash
hydra-liquidator --rpc-url ws://localhost:8000 \
  --oracle-scenario scenario.json
```

`scenario.json`:
```json
{
  "block": "latest",
  "oracle_updates": [
    {
      "pair": "DOT/USD",
      "price": 2.00,
      "asset_address": "0x..."
    },
    {
      "pair": "WETH/USD",
      "price": 1500.00,
      "asset_address": "0x..."
    }
  ]
}
```

Prices use DIA oracle format (8 decimal precision). The `asset_address` must be the EVM address of the asset in the money market reserve list.

### Persistent oracle overrides (`--oracle-persist`)

By default, injected oracle prices from `--oracle-scenario` are consumed once and lost when
`MoneyMarketData` re-initializes on the next block (it fetches fresh prices from chain).

With `--oracle-persist`, the injected prices are stored and re-applied after every MM re-init,
so the worker keeps seeing the simulated prices across all blocks:

```bash
hydra-liquidator --rpc-url ws://localhost:8000 \
  --oracle-scenario test-scenarios/dot-crash.json \
  --oracle-persist --no-interrupt
```

This is the simplest way to test liquidation detection — no on-chain changes needed.
The worker will report DRY-RUN liquidations on every block for as long as it runs.

## Node integration

When built with `--features node`, the crate provides `LiquidationTask::run()` which is spawned by the Hydration collator in `node/src/service.rs`.

The node's `liquidation_worker.rs` is a thin re-export wrapper:

```rust
pub use pepl_worker::node::{
    LiquidationTask, LiquidationTaskData, LiquidationWorkerConfig,
    rpc,
};
```

### How it works in the node

1. `LiquidationTask::run()` is spawned as an async task by the collator
2. It fetches the initial borrower list from Omniwatch, initializes MoneyMarketData
3. A worker thread is spawned that runs the generic `run_worker()` loop
4. The async task feeds block events (from `import_notification_stream`) and oracle updates (from the transaction pool mempool) into the worker via channels
5. On each new block:
   - MoneyMarketData is re-initialized with fresh on-chain state
   - All borrowers are scanned for liquidation opportunities
   - Qualifying liquidations are submitted as unsigned extrinsics via the transaction pool
6. Oracle updates from the DIA oracle (intercepted from the mempool) trigger immediate re-scans with updated prices

### Node CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--liquidation-worker` | `true` for validators | Enable/disable the worker |
| `--pap-contract` | production address | PoolAddressesProvider contract |
| `--runtime-api-caller` | production address | Account used for runtime API calls (needs WETH) |
| `--oracle-update-signer` | production signers | Allowed DIA oracle tx signers |
| `--oracle-update-call-address` | production addresses | DIA oracle contract addresses |
| `--target-hf` | `1.001` (1e18 units) | Target health factor |
| `--omniwatch-url` | production URL | Initial borrower data endpoint |
| `--weight-reserve` | `10` | Percent of block weight reserved for other txs |

### RPC API

The worker exposes three RPC methods:

- `liquidation_getBorrowers` — current borrower list with cached health factors
- `liquidation_isRunning` — whether the worker thread is active
- `liquidation_maxTransactionsPerBlock` — computed max liquidations per block based on weight limits

## Tests

```bash
cargo test -p pepl-worker
```

32 unit tests covering oracle parsing, borrower management, waitlist TTL, trait implementations, and standalone oracle injection.

## Testing Liquidatable Positions on a Local Fork

On mainnet (or a fork of it), all ~509 borrowers are healthy (HF > 1.0) because oracle prices
are frozen at fork time. To create liquidatable positions for end-to-end testing, you need to
crash an oracle price. There are two approaches:

### Strategy 1: Standalone mode — in-memory price injection

The simplest approach. No on-chain changes needed, the worker patches prices in memory.

```bash
# Start a local fork (Zombienet or Chopsticks)
# Then run the standalone liquidator with a price crash scenario:
hydra-liquidator --rpc-url ws://localhost:9944 \
  --oracle-scenario test-scenarios/dot-crash.json \
  --oracle-persist --no-interrupt
```

The `--oracle-persist` flag ensures the injected prices survive MM re-init on each new block.
Without it, prices are consumed once and the worker goes back to seeing healthy borrowers.

**What it tests**: Worker scanning, HF calculation, borrower selection, dry-run reporting.

**Limitation**: Prices are only patched in the worker's memory. The chain still has the original
prices, so no real liquidation transactions can be executed.

### Strategy 2: Node mode — real on-chain oracle update

Full end-to-end: override the oracle contract on-chain, worker detects it, submits liquidation
transactions, pallet executes them, `Liquidated` event emitted.

#### Step 1: Override the DIA oracle signer

The DIA oracle contract only allows `oracleUpdaterAddress` to call `setMultipleValues()`.
We override this at the Substrate storage level via `system.setStorage` (requires governance).

**Generate the encoded call** (run after every chain restart — fresh state restores original signer):

```bash
cd pepl-worker/test-scenarios
npm install @polkadot/api @polkadot/util @polkadot/util-crypto

# Generate encoded system.setStorage call for both oracle contracts (default: Alice as new signer)
node override-oracle-signer.js --ws ws://127.0.0.1:9944

# Or with a custom signer/oracle:
node override-oracle-signer.js --ws ws://127.0.0.1:9944 \
  --signer 0xYourEvmAddress \
  --oracle 0xdee629af973ebf5bf261ace12ffd1900ac715f5e
```

The script outputs the encoded call hex. To apply it:
1. Go to **Polkadot JS → Developer → Extrinsics → Decode** — paste the hex to verify
2. Go to **Governance → Referenda → Submit preimage** — paste the hex
3. Submit referendum on the appropriate track and vote to approve

**How it works**: `system.setStorage` writes to the raw Substrate storage key for
`pallet_evm::AccountStorages(oracle_contract, slot_1)`. The storage key layout is:

```
twox128("EVM") ++ twox128("AccountStorages")
  ++ blake2_128_concat(oracle_contract_h160)
  ++ blake2_128_concat(H256(1))   // slot 1 = oracleUpdaterAddress
```

The value is the new signer's EVM address left-padded to H256.

Verify the override after the governance proposal executes:

```bash
curl -s -X POST http://127.0.0.1:9999 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_getStorageAt","params":["0xdee629af973ebf5bf261ace12ffd1900ac715f5e","0x1","latest"],"id":1}'
# Should return: 0x000000000000000000000000<new_signer_address>
```

#### Step 2: Update oracle prices on-chain

```bash
node update-oracle.js --ws ws://127.0.0.1:9944 --price 1.50 --pair DOT/USD
```

This sends a `setMultipleValues()` call from Alice to the DIA oracle contract.
The contract accepts it because Alice is now `oracleUpdaterAddress` (from step 1).

#### Step 3: Start the PEPL worker node

The worker needs to accept Alice's oracle update TXs. Pass her EVM address as an
allowed signer:

```bash
# Node mode (embedded in collator):
--oracle-update-signer 0xd43593c715fdd31c61141abd04a99fd6822c8558

# Standalone mode:
hydra-liquidator --rpc-url ws://127.0.0.1:9944 --submit
```

The worker will:
1. See Alice's oracle update TX in the mempool (or detect the price change on the next block)
2. Patch in-memory prices → recalculate all borrower HFs
3. Find borrowers with HF < 1.0 due to the DOT price crash
4. Submit liquidation transactions → `Liquidated` event on-chain

### Key addresses

| What | Address |
|------|---------|
| Oracle contract 1 | `0xdee629af973ebf5bf261ace12ffd1900ac715f5e` |
| Oracle contract 2 | `0x48ae7803cd09c48434e3fc5629f15fb76f0b5ce5` |
| Default signer 1 | `0x33a5e905fB83FcFB62B0Dd1595DfBc06792E054e` |
| Default signer 2 | `0xff0c624016c873d359dde711b42a2f475a5a07d3` |
| PAP contract | `0xf3ba4d1b50f78301bdd7eaea9b67822a15fca691` |
| Alice EVM | `0xd43593c715fdd31c61141abd04a99fd6822c8558` |

### Test scenarios

| File | Purpose |
|------|---------|
| `test-scenarios/dot-crash.json` | DOT → $1.50 for standalone oracle injection |
| `test-scenarios/override-oracle-signer.js` | Generate encoded `system.setStorage` call to replace DIA oracleUpdaterAddress (propose via governance) |
| `test-scenarios/update-oracle.js` | Full script: override signer + send price update + verify |

### Quick-start checklist (node mode, full e2e)

1. Start local fork (Zombienet two-collator setup)
2. `node test-scenarios/override-oracle-signer.js --ws ws://127.0.0.1:9944` → get encoded call hex
3. Propose + approve the `system.setStorage` call via governance (Polkadot JS)
4. Verify: `eth_getStorageAt(oracle_contract, 0x1)` returns the new signer
5. Start PEPL worker node with `--oracle-update-signer <NEW_SIGNER_EVM>`
6. `node test-scenarios/update-oracle.js --ws ws://127.0.0.1:9944 --price 1.50 --pair DOT/USD`
7. Watch worker logs for `undercollateralized` → `SUBMITTED` → check chain for `Liquidated` events
