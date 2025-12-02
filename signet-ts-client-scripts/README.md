# Signet Substrate Client

Test client for the Signet pallet on Substrate/Polkadot. Validates signature generation and verification for both simple payloads, EIP-1559 transactions, and ERC20 vault deposits.

## Prerequisites

- Node.js v16+ and npm/yarn
- Running Substrate node with Signet pallet deployed (port 8000)
- Access to the Signet signature server
- For ERC20 vault tests: Funded Ethereum Sepolia account with ETH and USDC

## Setup

### 1. Start the Signature Server

Clone and run the signature server that responds to Substrate signature requests. Copy-paste the .env.example to .env at the root of the dir - signet-ts-client-scripts:

```bash
cd signet-ts-client-scripts/clients/response-server
yarn start
```

The server will:

- Connect to your Substrate node
- Automatically respond to signature requests
- Monitor Ethereum transactions and report results back to Substrate

### 2. Install Test Client Dependencies

```bash
yarn install
```

### 3. Ensure Substrate Node is Running

The tests expect a Substrate node with the Signet pallet at `ws://localhost:8000`. If using Chopsticks:

```bash
cargo build --release
```

```bash
npx @acala-network/chopsticks@latest --config=hydradx \
  --wasm-override ./target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm \
  --db=:memory:
```

## Running Tests

```bash
yarn test erc20vault.test.ts
```
