# Dispenser E2E Tests

End-to-end tests for the `pallet_signet` + `pallet_dispenser` flow: Substrate `requestFund` -> MPC signature -> EVM faucet `fund()` call.

## Prerequisites

- **Substrate node** — one of:
  - Chopsticks (local fork of HydraDX mainnet)
  - Lark testnet
  - HydraDX mainnet
- **EVM node** — one of:
  - Anvil (local)
  - Sepolia
  - Ethereum mainnet
- **MPC response server** running and connected to the substrate node (chopsticks uses `mock-signature-host: true`)
- **GasFaucet contract** deployed on the EVM network
- Node.js + yarn

## Quick Start (Chopsticks + Anvil)

```bash
cd scripts/dispenser-tests
yarn install

# 1. Start chopsticks (separate terminal)
npx @acala-network/chopsticks@latest \
  --config=../../launch-configs/chopsticks/hydradx.yml \
  --wasm-override ../../target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm \
  --db=:memory: --build-block-mode Instant

# 2. Start Anvil (separate terminal)
anvil

# 3. Deploy faucet contract + fund derived address (see pallets/dispenser/contracts/)

# 4. Set on-chain configs for Signet + Dispenser
npx ts-node tc-set-config.ts

# 5. Run the test
yarn test dispenser.test.ts
```

## Configuration

### `.env` — Network Selection

Only two values are required. Everything else has defaults in `networks.ts`.

```env
SUBSTRATE_NETWORK=chopsticks   # chopsticks | lark | mainnet
EVM_NETWORK=anvil              # anvil | sepolia | mainnet
```

Any preset value can be overridden via env vars (e.g. `EVM_RPC_URL`, `SUBSTRATE_WS_ENDPOINT`, `SUBSTRATE_CHAIN_ID`). See `.env.example` for the full list.

### Network Presets (`networks.ts`)

| Substrate | WS Endpoint | CAIP-2 Chain ID | SS58 |
|-----------|-------------|-----------------|------|
| `chopsticks` | `ws://localhost:8000` | `polkadot:e6b50b06e72a81194e9c96c488175ecd` | 63 |
| `lark` | `wss://1.lark.hydration.cloud` | `polkadot:e6b50b06e72a81194e9c96c488175ecd` | 63 |
| `mainnet` | `wss://rpc.hydradx.cloud` | `polkadot:afdc188f45c71dacbaa0b62e16a91f72` | 63 |

| EVM | RPC URL | Chain ID |
|-----|---------|----------|
| `anvil` | `http://localhost:8545` | 31337 |
| `sepolia` | `https://ethereum-sepolia-rpc.publicnode.com` | 11155111 |
| `mainnet` | `https://eth.llamarpc.com` | 1 |

### Important: `SUBSTRATE_CHAIN_ID`

This is the CAIP-2 chain identifier stored in the signet on-chain config. It determines **which MPC key is derived** for signing. The test's key derivation and the MPC server must use the same value.

- Must match what `tc-set-config.ts` wrote to `Signet.SignetConfig.chainId`
- Format: `polkadot:<genesis_hash_first_16_bytes_hex>` (NOT `polkadot:<parachain_id>`)
- If the derived ETH address doesn't match the MPC signature, this is the first thing to check

## Setting On-Chain Configs (`tc-set-config.ts`)

Both `pallet_signet` and `pallet_dispenser` require on-chain configuration before the test can run. `tc-set-config.ts` sets both in one step.

### Chopsticks

Writes directly to storage via `dev_setStorage` — no governance needed.

```bash
npx ts-node tc-set-config.ts
```

### Lark / Mainnet

Creates a Technical Committee (TC) proposal. Requires `SURI` of a TC member.

```bash
SUBSTRATE_NETWORK=lark SURI=//Alice npx ts-node tc-set-config.ts
```

If the signer is the **only TC member**, the proposal executes immediately (threshold=1). Otherwise, other TC members must vote Aye.

### What it configures

**Signet** (`signet.setConfig`):

| Field | Value | Description |
|-------|-------|-------------|
| `signatureDeposit` | 0.1 HDX | Deposit locked per signing request |
| `maxChainIdLength` | 128 | Max chain ID byte length |
| `maxEvmDataLength` | 100,000 | Max EVM tx data byte length |
| `chainId` | From network preset | CAIP-2 chain ID for MPC key derivation |

**Dispenser** (`ethDispenser.setConfig`):

| Field | Value | Description |
|-------|-------|-------------|
| `faucetAddress` | `0x189d33...` | GasFaucet contract on EVM |
| `minFaucetThreshold` | 0.05 ETH | Min remaining ETH after a request |
| `minRequest` | 0 | Min request amount |
| `maxDispense` | 1 ETH | Max request amount |
| `dispenserFee` | 1 HDX | Fee charged per request (must be >= HDX existential deposit) |
| `faucetBalanceWei` | 10 ETH | Tracked faucet balance |

## MPC Response Server

Clone and run the MPC response server that listens for `SignBidirectionalRequested` events and responds with signatures.

```bash
# From scripts/dispenser-tests
git clone https://github.com/sig-net/solana-signet-program
cd solana-signet-program/clients/response-server

# Configure .env
cat > .env << 'EOF'
SUBSTRATE_WS_URL=ws://localhost:8000
SUBSTRATE_SIGNER_SEED=//Bob
PRIVATE_KEY_TESTNET=<ETHEREUM_PRIVATE_KEY>
SEPOLIA_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com
SOLANA_PRIVATE_KEY='[16,151,155,240,...,147]'
EOF

yarn install && yarn start
```

On chopsticks with `mock-signature-host: true`, the mock MPC is built in — no separate server needed.

## Test Flow

1. **Setup** — fund pallet accounts (dispenser + signet), ensure Alice has WETH, set configs
2. **requestFund** — Alice submits `ethDispenser.requestFund` on substrate
   - Charges `dispenserFee` (HDX) and locks WETH collateral to Treasury
   - Emits `SignBidirectionalRequested` event for the MPC
3. **MPC signature** — MPC server signs the EVM transaction, emits `SignatureResponded`
4. **Signature verification** — test recovers the signer address and verifies it matches the derived MPC address
5. **EVM broadcast** — signed transaction is broadcast to the EVM network, calling `fund(to, amount)` on the faucet contract
6. **Read response** — MPC reads the EVM tx receipt and emits `RespondBidirectionalEvent`

## Common Issues

| Error | Cause | Fix |
|-------|-------|-----|
| `{"token":"BelowMinimum"}` | Transfer amount below existential deposit | Ensure `REQUEST_FUND_AMOUNT_WEI` > WETH ED (~5.4e12), `dispenserFee` >= HDX ED (1e12), and signet pallet account is funded with HDX |
| Signature verification failed | `SUBSTRATE_CHAIN_ID` mismatch | Ensure `.env` chain ID matches the on-chain signet config. Re-run `tc-set-config.ts` if needed |
| `NotConfigured` | Signet or dispenser config not set | Run `npx ts-node tc-set-config.ts` |
| `DuplicateRequest` | Same request ID used twice | Restart chopsticks or wait for nonce to advance |
| Timeout waiting for MPC signature | MPC not running or not connected | Check MPC server logs. On chopsticks, ensure `mock-signature-host: true` in the yml config |

## File Overview

| File | Description |
|------|-------------|
| `networks.ts` | Network presets (endpoints, chain IDs, defaults) |
| `env.ts` | Loads `.env`, merges with presets, exports `ENV` |
| `tc-set-config.ts` | Sets signet + dispenser on-chain configs (chopsticks or TC proposal) |
| `dispenser.test.ts` | Main e2e test |
| `signet-client.ts` | Signet pallet helpers (request ID, wait for signature, block scanning) |
| `utils.ts` | Shared helpers (submitWithRetry, executeAsRoot, fund accounts, tip escalation) |
| `key-derivation.ts` | MPC child key derivation (epsilon derivation from root public key) |
