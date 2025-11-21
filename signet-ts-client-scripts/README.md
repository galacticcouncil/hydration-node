# Signet Substrate Client

Test client for the Signet pallet on Substrate/Polkadot. Validates signature generation and verification for both simple payloads, EIP-1559 transactions, and ERC20 vault deposits.

## Prerequisites

- Node.js v16+ and npm/yarn
- Running Substrate node with Signet pallet deployed (port 8000)
- Access to the Signet signature server
- For ERC20 vault tests: Funded Ethereum Sepolia account with ETH and USDC

## Setup

### 1. Start the Signature Server

Clone and run the signature server that responds to Substrate signature requests. Add .env to the root of the repository:

```bash
# Clone the server repository
git clone https://github.com/sig-net/solana-signet-program
cd solana-signet-program/clients/response-server

# Install dependencies
yarn install

# Configure environment variables
cat > .env << EOF
# Substrate Configuration
SUBSTRATE_WS_URL=ws://localhost:8000
SUBSTRATE_SIGNER_SEED=//Bob
PRIVATE_KEY_TESTNET=0x0aafff3d8934d620e90cd9eeeea1d63f76c5d35a912471974439560321e9323a
SEPOLIA_RPC_URL=https://ethereum-sepolia-rpc.publicnode.com
# Dummy solana key
SOLANA_PRIVATE_KEY='[16,151,155,240,122,151,187,95,145,26,179,205,196,113,3,62,17,105,18,240,197,176,45,90,176,108,30,106,182,43,7,104,80,202,59,51,239,219,236,17,39,204,155,35,175,195,17,172,201,196,134,125,25,214,148,76,102,47,123,37,203,86,159,147]'
EOF

# Start the server
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
npx @acala-network/chopsticks@latest --config=./launch-configs/chopsticks/hydradx.yml
```

## Running Tests

```bash
yarn test erc20vault.test.ts
```
