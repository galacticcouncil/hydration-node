# PEPL Worker — Testing Liquidatable Positions

## Context

The PEPL worker runs in node mode on a local two-collator chain (forked from mainnet state, ~509 real borrowers). We need to create liquidatable positions for end-to-end testing. All borrowers are currently healthy (HF > 1.0) because prices are frozen at fork time.

## Key Facts

**HF calculation in the worker** (`liquidation-worker-support/src/lib.rs:816`):
- `UserData::new()` fetches token **balances** from chain (EVM calls)
- Multiplies by `Reserve.price` — the **in-memory cached price**
- `update_reserve_price()` patches this cache; subsequent HF calcs use patched price

**Oracle signer authorization** — two levels:
1. **Contract level**: DIA oracle Solidity contract checks `msg.sender == oracleUpdaterAddress` (stored in contract slot 1). Override via `system.setStorage` on `pallet_evm::AccountStorages` (requires governance proposal).
2. **Worker level**: `verify_oracle_update_transaction()` in `pepl-worker/src/node/mod.rs:221` checks `allowed_signers` and `allowed_oracle_call_addresses`
   - Defaults hardcoded in `pepl-worker/src/config.rs:18-26`:
     - Signers: `0x33a5e905...`, `0xff0c6240...`
     - Call addresses (DIA contracts): `0xdee629af...`, `0x48ae7803...`
   - Overridable via CLI: `--oracle-update-signer` and `--oracle-update-call-address` (in `pepl-worker/src/node/runner.rs:58-62`)
   - Also configurable in `launch-configs/fork/config.json` as collator args
   - No substrate storage for these — purely worker config

**Worker scan paths**:
- `LiquidateAll` (every block): re-initializes `MoneyMarketData` from chain (reads oracle contract) → scans all borrowers
- `OracleUpdate` (mempool): patches prices in memory → scans all borrowers with updated prices (faster, pre-block)

---

## Strategy 1: Standalone Mode — Oracle Scenario Injection (implemented)

**What it tests**: Worker scanning, HF calculation, borrower selection, dry-run reporting.

The `--oracle-scenario` flag injects prices into memory. The `--oracle-persist` flag ensures
injected prices survive MoneyMarketData re-init on each new block (without it, prices are
consumed once and lost when fresh oracle prices are fetched from chain).

### Usage
```bash
hydra-liquidator --rpc-url ws://localhost:9944 \
  --oracle-scenario test-scenarios/dot-crash.json \
  --oracle-persist --no-interrupt
```

**Limitation**: Prices are only patched in the worker's memory. The chain still has the original
prices, so no real liquidation transactions can be executed.

---

## Strategy 2: Node Mode — Real Oracle Update on Local Chain

**What it tests**: Full end-to-end — detection, dry-run, tx submission, on-chain execution, Liquidated event, waitlist clearing.

### Prerequisites

```bash
cd pepl-worker/test-scenarios
npm install
```

### Step 1: Verify current oracle signers

```bash
npm run check-signer -- --rpc http://127.0.0.1:9999
```

Expected output (mainnet fork, fresh state):
```
Oracle 1 (0xdee629af973ebf5bf261ace12ffd1900ac715f5e)
  oracleUpdaterAddress: 0x33a5e905fb83fcfb62b0dd1595dfbc06792e054e  [Default signer 1]

Oracle 2 (0x48ae7803cd09c48434e3fc5629f15fb76f0b5ce5)
  oracleUpdaterAddress: 0xff0c624016c873d359dde711b42a2f475a5a07d3  [Default signer 2]
```

### Step 2: Override the DIA oracle signer (governance)

The DIA oracle Solidity contract only allows `oracleUpdaterAddress` to call `setMultipleValues()`.
There is no sudo on Hydration — we use a governance proposal with `system.setStorage` to overwrite
the contract's storage slot 1 (where `oracleUpdaterAddress` lives).

**Generate the encoded call:**
```bash
npm run override-signer -- --ws ws://127.0.0.1:9944
```

This outputs the encoded `system.setStorage` call hex. It does NOT submit anything.

**Apply via governance:**
1. Go to **Polkadot JS → Developer → Extrinsics → Decode** — paste the encoded call hex to verify it decodes to `system.setStorage` with the correct key-value pairs
2. Go to **Governance → Referenda → Submit preimage** — paste the encoded call hex
3. Submit referendum on the appropriate track (Root or WhitelistedCaller)
4. Vote to approve and wait for execution

**How it works**: `system.setStorage` writes to the raw Substrate storage key for
`pallet_evm::AccountStorages(oracle_contract, slot_1)`:

```
twox128("EVM") ++ twox128("AccountStorages")
  ++ blake2_128_concat(oracle_contract_h160)
  ++ blake2_128_concat(H256(1))   // slot 1 = oracleUpdaterAddress
```

The value is the new signer's EVM address left-padded to H256 (Solidity stores `address` right-aligned in 32-byte slots):
```
0x000000000000000000000000d43593c715fdd31c61141abd04a99fd6822c8558
```

**Must be re-proposed after every chain restart** (fresh state restores the original signers).

**Verify the override took effect:**
```bash
npm run check-signer -- --rpc http://127.0.0.1:9999
```

Should now show:
```
Oracle 1 (0xdee629af973ebf5bf261ace12ffd1900ac715f5e)
  oracleUpdaterAddress: 0xd43593c715fdd31c61141abd04a99fd6822c8558  [Alice (dev)]
```

### Step 3: Configure the worker to accept Alice's oracle updates

The worker-level check (`verify_oracle_update_transaction()`) must also know about Alice.
This is configured via the `--oracle-update-signer` CLI flag on the collator.

Already set in `launch-configs/fork/config.json`:
```json
"args": [
  "--oracle-update-signer=0xd43593c715fdd31c61141abd04a99fd6822c8558"
]
```

If running the node manually, pass the flag directly:
```bash
./target/release/hydradx ... --oracle-update-signer 0xd43593c715fdd31c61141abd04a99fd6822c8558
```

### Step 4: Fund Alice's EVM address with WETH

Alice needs WETH to pay EVM gas fees (Hydration EVM uses `WethCurrency`). On a mainnet fork,
dev accounts have no balances. Transfer WETH to Alice or include a WETH balance override in
the governance proposal.

### Step 5: Update oracle prices on-chain

Send a price update via `evm.call()` signed by Alice's Substrate key (the runtime's
`EnsureAddressTruncated` CallOrigin maps her Substrate account to her EVM address):

```bash
# Single pair
npm run update-oracle -- --ws ws://127.0.0.1:9944 --pair DOT/USD --price 1.50

# Multiple pairs in one call (matches real DIA setMultipleValues format)
npm run update-oracle -- --ws ws://127.0.0.1:9944 \
  --pair AAVE/USD --price 80.00 \
  --pair wstETH/USD --price 1800.00
```

The script:
1. Verifies the sender matches the contract's `oracleUpdaterAddress` (exits with error if not)
2. Reads current oracle prices for reference
3. Packs prices in DIA format: `(price_8dec << 128) | timestamp`
4. Submits `evm.call()` signed by Alice (`//Alice`)
5. Verifies updated prices after inclusion

### Step 6: Watch the worker

The worker should:
1. See the oracle update TX in the mempool (or detect the price change on the next block)
2. Patch in-memory prices → recalculate all borrower HFs
3. Find borrowers with HF < 1.0 due to the price crash
4. Submit liquidation transactions → `Liquidated` event on-chain

---

## Key Addresses

| What | Address |
|------|---------|
| Oracle contract 1 | `0xdee629af973ebf5bf261ace12ffd1900ac715f5e` |
| Oracle contract 2 | `0x48ae7803cd09c48434e3fc5629f15fb76f0b5ce5` |
| Default signer 1 | `0x33a5e905fB83FcFB62B0Dd1595DfBc06792E054e` |
| Default signer 2 | `0xff0c624016c873d359dde711b42a2f475a5a07d3` |
| PAP contract | `0xf3ba4d1b50f78301bdd7eaea9b67822a15fca691` |
| Alice Substrate | `5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY` |
| Alice EVM | `0xd43593c715fdd31c61141abd04a99fd6822c8558` |

## Test Scenario Scripts

All scripts live in `pepl-worker/test-scenarios/`. Run `npm install` once to install dependencies.

| Script | npm command | Purpose |
|--------|-------------|---------|
| `check-oracle-signer.js` | `npm run check-signer` | Read current oracleUpdaterAddress from both DIA contracts |
| `override-oracle-signer.js` | `npm run override-signer` | Generate encoded `system.setStorage` call for governance proposal |
| `update-oracle.js` | `npm run update-oracle` | Send price update via `evm.call()` signed by Alice |

| Data file | Purpose |
|-----------|---------|
| `dot-crash.json` | DOT → $1.50 scenario for standalone `--oracle-scenario` |

## DIA Oracle Price Format

Prices are packed as `uint256`: upper 128 bits = price (8 decimal precision), lower 128 bits = Unix timestamp.

Example from a real DIA update:
```
AAVE/USD:    price=11205659643  → $112.06    timestamp=1773247167
wstETH/USD:  price=253393687069 → $2,533.94  timestamp=1773247167
```

The `update-oracle.js` script handles packing automatically — just pass human-readable prices.

Reference for ABI encoding: `integration-tests/src/liquidation.rs:134-158`

## Quick-Start Checklist (Node Mode, Full E2E)

1. Start local fork: `cd launch-configs/fork && zombienet spawn config.json`
   - Config already includes `--oracle-update-signer` for Alice
2. `cd pepl-worker/test-scenarios && npm install`
3. Verify current signers: `npm run check-signer -- --rpc http://127.0.0.1:9999`
4. Generate override call: `npm run override-signer -- --ws ws://127.0.0.1:9944`
5. Propose + approve the `system.setStorage` call via Polkadot JS governance
6. Verify override: `npm run check-signer -- --rpc http://127.0.0.1:9999` (should show Alice)
7. Fund Alice with WETH for gas
8. Update prices: `npm run update-oracle -- --ws ws://127.0.0.1:9944 --pair DOT/USD --price 1.50`
9. Watch worker logs for `undercollateralized` → `SUBMITTED` → check chain for `Liquidated` events
