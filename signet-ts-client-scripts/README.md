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
cd chain-signatures-solana/clients/response-server

# Install dependencies
yarn install

# Configure environment variables
cat > .env << EOF
# Substrate Configuration
SUBSTRATE_WS_URL=ws://localhost:8000
SUBSTRATE_SIGNER_SEED=//Bob

# Signing Keys (must match ROOT_PUBLIC_KEY in tests)
PRIVATE_KEY_TESTNET=0x... # Your private key for signing

# Ethereum Configuration (for vault monitoring)
SEPOLIA_RPC_URL=https://sepolia.infura.io/v3/your_infura_key_here

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
npx @acala-network/chopsticks@latest --config=hydradx \
  --wasm-override ./target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm \
  --db=:memory:
```

### 4. Fund Ethereum Account for Vault Tests

The ERC20 vault test requires a funded account on Sepolia. The test derives an Ethereum address from your Substrate account and expects it to have:

- At least 0.001 ETH for gas
- At least 0.01 USDC (testnet) at address `0xbe72E441BF55620febc26715db68d3494213D8Cb`

The derived address is deterministic based on your Substrate account. Run the test once to see the address, then fund it on Sepolia

## Running Tests

```bash
# Run all tests
yarn test

# Run specific test suite
yarn test signet.test.ts
yarn test erc20vault.test.ts

# Run with watch mode
yarn test:watch
```

## Test Coverage

### Basic Signature Tests (`signet.test.ts`)
- **Simple Signatures**: Request and verify ECDSA signatures for 32-byte payloads
- **Transaction Signatures**: Sign and verify EIP-1559 Ethereum transactions
- **Key Derivation**: Verify derived keys match between client and server
- **Address Recovery**: Ensure signature recovery produces expected addresses

### ERC20 Vault Integration (`erc20vault.test.ts`)
- **Vault Initialization**: Initialize vault with MPC signer address
- **Deposit Flow**: 
  - Request MPC signature for ERC20 transfer transaction
  - Verify signature and broadcast to Sepolia
  - Wait for transaction confirmation
- **Result Monitoring**: MPC server observes transaction result on Ethereum
- **Claim Flow**: 
  - Receive transaction output from MPC
  - Verify MPC signature on result
  - Claim deposited tokens in Substrate vault
- **Multi-token Support**: Vault supports any ERC20 token (decimal-agnostic)

## Expected Output

### Basic Signature Test
```
PASS ./signet.test.ts
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
        ✅ Transaction signature verification PASSED
```

### ERC20 Vault Test
```
PASS  ./erc20vault.test.ts (47.902 s)
  ERC20 Vault Integration
    ✓ should complete full deposit and claim flow (43924 ms)

  🔑 Derived Ethereum Address: 0xde36cd568b21c9e5b19ab6ecf01f9e5024398913
  💰 Balances for 0xde36cd568b21c9e5b19ab6ecf01f9e5024398913:
       ETH: 0.00999946
       USDC: 0.06
  
  Initializing vault with MPC address: 0x00a40c2661293d5134e53da52951a3f7767836ef
  ✅ Vault initialized
  
  📊 Current nonce for 0xde36cd568b21c9e5b19ab6ecf01f9e5024398913: 18
  📋 Request ID: 0x15dc855a7fb93a3e694d5a93b9a40a2a141c7af0bbfc6afdc20d8c80ce4124f7
  🚀 Submitting deposit_erc20 transaction...
  ⏳ Waiting for MPC signature...
  
  ✅ Received signature from: 14E5nqKAp3oAJcmzgZhUD2RcptBeUBScxKHgJKU4HPNcKVf3
  🔍 Signature verification:
       Expected address: 0xde36cd568b21c9e5b19ab6ecf01f9e5024398913
       Recovered address: 0xDE36CD568B21C9E5B19AB6ECF01F9e5024398913
       Match: true
  
  📊 Fresh nonce check: 18
  📡 Broadcasting transaction to Sepolia...
       Tx Hash: 0xead2b6a3de9fdd90d04da5f329c5ada25060b4f0e48ebc65aa1c1c601696f974
  ✅ Transaction confirmed in block 9357487
  
  ⏳ Waiting for MPC to read transaction result...
  ✅ Received read response
  ✅ Claim transaction confirmed
  ✅ Balance increased by: 0.01 USDC
       Total balance: 0.04 USDC

Test Suites: 1 passed, 1 total
Tests:       1 passed, 1 total
Time:        48.056 s
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
- **Vault test failures**:
  - Ensure your derived Ethereum address is funded with ETH and USDC on Sepolia
  - Verify the MPC server has Ethereum monitoring enabled with `SEPOLIA_RPC` configured
  - Check that the vault's MPC address matches the server's signing key
- **InvalidSigner errors**: The output bytes from Substrate events include SCALE encoding that must be stripped before verification