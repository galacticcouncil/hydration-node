# Signet Substrate Client

Test client for the Signet pallet on Substrate/Polkadot. Validates signature generation and verification for both simple payloads and EIP-1559 transactions.

## Prerequisites

- Node.js v16+ and npm/yarn
- Running Substrate node with Signet pallet deployed (port 8000)
- Access to the Signet signature server

## Setup

### 1. Start the Signature Server

Clone and run the signature server that responds to Substrate signature requests:

```bash
# Clone the server repository
git clone https://github.com/sig-net/chain-signatures-solana.git
cd chain-signatures-solana/clients/response-server

# Install dependencies
yarn install

# Configure environment variables
cat > .env << EOF
# Substrate Configuration
SUBSTRATE_WS_URL=ws://localhost:8000
SUBSTRATE_SIGNER_SEED=//Alice

# Signing Keys (must match ROOT_PUBLIC_KEY in tests)
PRIVATE_KEY_TESTNET=0x... # Your private key for signing

# Optional: Ethereum RPC for transaction monitoring
INFURA_API_KEY=your_infura_key_here
EOF

# Start the server
yarn start
```

The server will connect to your Substrate node and automatically respond to signature requests.

### 2. Install Test Client Dependencies

```bash
yarn install
```

### 3. Ensure Substrate Node is Running

The tests expect a Substrate node with the Signet pallet at `ws://localhost:8000`. If using Chopsticks:

```bash
npx @acala-network/chopsticks@latest --config=hydradx \
  --wasm-override ./path/to/hydradx_runtime.wasm \
  --db=:memory:
```

## Running Tests

```bash
# Run all tests
yarn test

# Run with watch mode
yarn test:watch
```

## Test Coverage

The test suite validates:
- **Simple Signatures**: Request and verify ECDSA signatures for 32-byte payloads
- **Transaction Signatures**: Sign and verify EIP-1559 Ethereum transactions
- **Key Derivation**: Verify derived keys match between client and server
- **Address Recovery**: Ensure signature recovery produces expected addresses

## Expected Output

```
PASS  ./signet.test.ts
  Signet Pallet Integration
    Sign
      ✓ should request and verify a signature
        ✅ Signature received from: 14E5nqKAp3oAJcmzgZhUD2RcptBeUBScxKHgJKU4HPNcKVf3
           Recovered: 0xF4a62e4f48e8e71170BA758b5bAf90646db61301
           Expected:  0xf4a62e4f48e8e71170ba758b5baf90646db61301
        ✅ Signature verification PASSED
    SignRespond
      ✓ should request and verify a transaction signature
        ✅ Transaction signature received from: 14E5nqKAp3oAJcmzgZhUD2RcptBeUBScxKHgJKU4HPNcKVf3
           Recovered: 0xF4a62e4f48e8e71170BA758b5bAf90646db61301
           Expected:  0xf4a62e4f48e8e71170ba758b5baf90646db61301
        ✅ Transaction signature verification PASSED
```

## Configuration

The root public key used for derivation is hardcoded in the tests. Ensure the server's `PRIVATE_KEY_TESTNET` corresponds to:

```typescript
const ROOT_PUBLIC_KEY = "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
```

## Troubleshooting

- **Timeout errors**: Ensure the signature server is running and connected to the same Substrate node
- **Address mismatch**: Verify the server's private key matches the client's expected public key
- **Transaction errors**: Check that the Signet pallet is initialized (tests handle this automatically)