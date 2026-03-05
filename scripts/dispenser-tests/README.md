# Dispenser (ERC20 Vault) Integration Test

End-to-end test for the `ethDispenser` pallet on HydraDX. The test submits a
`requestFund` extrinsic on Substrate (Lark testnet), waits for the MPC response
server to sign and broadcast an EVM transaction on a local Anvil chain, and
verifies the full bidirectional flow completes.

## Architecture

```
Test (Alice)                    Response Server (Bob)           Anvil (local EVM)
     |                                |                              |
     |-- requestFund (substrate) ---->|                              |
     |   (emits SignBidirectional)    |                              |
     |                                |-- sign EVM tx ------------->|
     |                                |-- signet.respond ---------->|
     |<-- waitForSignature ---------- |                              |
     |-- broadcast signed EVM tx --->|                              |-- fund() -->
     |                                |-- poll tx receipt ---------->|
     |                                |-- signet.respondBidirectional
     |<-- waitForReadResponse --------|                              |
```

- **Substrate**: Lark testnet (`wss://1.lark.hydration.cloud`, SS58 prefix 63)
- **EVM**: Local Anvil (`http://localhost:8545`, chain ID 31337)
- **Test signer**: `//Alice` (substrate)
- **Server signer**: `//Bob` (substrate) — must be different from the test signer to avoid nonce conflicts

## Prerequisites

- Node.js v18+
- [Foundry](https://book.getfoundry.sh/getting-started/installation) (`forge`, `cast`, `anvil`)
- yarn or npm
- Access to Lark testnet (public WebSocket endpoint)

## Setup

### Step 1: Start Anvil

Start a local Anvil instance. It must stay running for the entire test.

```bash
anvil
```

This starts a local EVM chain at `http://localhost:8545` with chain ID 31337.
Anvil account 0 (`0xf39F...`) is pre-funded with 10000 ETH.

### Step 2: Deploy the GasFaucet contracts

From the contracts directory, deploy GasFaucet + GasVoucher via the Foundry
script. The contracts use CREATE2, so the addresses are deterministic.

```bash
cd pallets/dispenser/contracts

# Install Solidity dependencies (first time only)
forge install

# Set deployment env vars
export PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
export MPC_ADDRESS=0xB62E97b543ff7c9Ec78997CCa807C9Fb982cBa89

# Deploy to local Anvil
forge script script/Deployment.sol:GasFaucetScript \
  --rpc-url http://localhost:8545 \
  --private-key $PRIVATE_KEY \
  --broadcast
```

Expected output:
- GasVoucher: `0x73779f1abc7414af11c1108ce924649a81607777`
- GasFaucet: `0x189d33ea9A9701fdb67C21df7420868193dcf578`

> **Note**: The `MPC_ADDRESS` is the derived Ethereum address from the MPC root
> key + Lark chain ID. If you change the MPC root key or the chain ID, you need
> to recompute it. See [Deriving the MPC address](#deriving-the-mpc-address).

### Step 3: Fund the derived ETH address

The MPC-derived address needs ETH for gas on Anvil:

```bash
cast send 0xB62E97b543ff7c9Ec78997CCa807C9Fb982cBa89 \
  --value 10ether \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --rpc-url http://localhost:8545
```

### Step 4: Clone and start the Response Server

The response server listens for Substrate `SignBidirectionalRequested` events,
signs EVM transactions, broadcasts them, and reports results back to Substrate.

```bash
# Clone inside the dispenser-tests directory (already gitignored)
cd scripts/dispenser-tests
git clone git@github.com:galacticcouncil/solana-signet-program.git

cd solana-signet-program/clients/response-server
yarn install
```

Create the server `.env` at `solana-signet-program/.env` (note: **root** of the
cloned repo, not inside `clients/response-server`):

```bash
cat > ../../.env << 'EOF'
MPC_ROOT_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
EVM_RPC_URL=http://127.0.0.1:8545
BITCOIN_NETWORK=testnet
SUBSTRATE_WS_URL=wss://1.lark.hydration.cloud
SUBSTRATE_SIGNER_SEED=//Bob
EOF
```

Start the server:

```bash
yarn start
```

You should see:
```
Connected to Substrate node
  Chain: ...
  Signer address: 5FHneW46... (Bob)
```

Keep this running in a separate terminal.

### Step 5: Configure and run the test

```bash
cd scripts/dispenser-tests

# Install dependencies (first time only)
yarn install

# Copy the example env (already configured for Lark + local Anvil)
cp .env.example .env
# Edit .env if you need to change anything (defaults work out of the box)

# Run the test
yarn test dispenser.test.ts
```

The test will:
1. Fund Bob (server signer) with HDX if needed
2. Fund the pallet account with HDX if needed
3. Ensure Alice has enough faucet asset (WETH) via Root governance referendum
4. Set the faucet contract's MPC address if needed
5. Submit `requestFund` to Substrate
6. Wait for the MPC signature (~6-30s)
7. Broadcast the signed EVM transaction to Anvil
8. Wait for the server to read the result and respond (~30-120s)
9. Verify the full flow completed

The test timeout is 20 minutes to accommodate governance referendum waits on
first run.

## Environment Variables

See `.env.example` for all variables with documentation. Key values:

| Variable | Default | Description |
|---|---|---|
| `NETWORK` | `sepolia` | Network selector (affects chain ID defaults) |
| `EVM_RPC_URL` | `http://localhost:8545` | Anvil RPC endpoint |
| `EVM_CHAIN_ID` | `31337` | Must match Anvil |
| `ROOT_PUBLIC_KEY` | `0x0483...` | Uncompressed pubkey of MPC root key |
| `FAUCET_ADDRESS` | `0x189d...` | GasFaucet contract on Anvil |
| `SUBSTRATE_WS_ENDPOINT` | `wss://1.lark.hydration.cloud` | Lark WS |
| `SUBSTRATE_CHAIN_ID` | `polkadot:e6b50b...` | Lark genesis hash prefix |
| `SS58_PREFIX` | `63` | Lark SS58 prefix |
| `TARGET_ADDRESS` | `0x7f67...` | ETH address to receive funds |
| `REQUEST_FUND_AMOUNT_WEI` | `1000000000000` | 0.000001 ETH |

## Deriving the MPC address

The MPC-derived Ethereum address is deterministic based on:
- `MPC_ROOT_KEY` (Anvil account 0 private key)
- `SUBSTRATE_CHAIN_ID` (`polkadot:e6b50b06e72a81194e9c96c488175ecd`)

To compute it:

```bash
node pubKey.js
```

This prints the uncompressed public key and Ethereum address for Anvil account 0.
The derived address used for signing is further derived using the chain ID and
the substrate account path. With the default keys, the derived address is:

```
0xB62E97b543ff7c9Ec78997CCa807C9Fb982cBa89
```

This address must be:
1. Set as `MPC_ADDRESS` when deploying the GasFaucet contract
2. Funded with ETH on Anvil for gas
3. The test's `ensureFaucetMpcAddress()` helper auto-sets it on each run

## Troubleshooting

### Test stuck at "Waiting for MPC signature"

- Check the response server is running and connected to Lark
- Check server logs for errors after `SignBidirectionalRequested`
- The server signer (`//Bob`) needs HDX on Lark for tx fees. The test funds Bob
  automatically, but if it's the first run and Alice doesn't have enough HDX,
  you may need to fund Bob manually via Polkadot.js Apps

### EVM transaction reverts

- Check the faucet contract's MPC address: `cast call <FAUCET_ADDRESS> "mpc()(address)" --rpc-url http://localhost:8545`
- It must match the derived address (`0xB62E...`). The test sets this automatically
- Check the faucet has ETH: `cast balance <FAUCET_ADDRESS> --rpc-url http://localhost:8545`

### Server tx goes Ready -> Broadcast -> never InBlock

This is a known issue with Polkadot.js WebSocket subscriptions on remote nodes.
The server has a nonce-polling fallback that detects inclusion within ~15-20s.
Check server logs for "nonce advanced (poll fallback)".

### Anvil restarted mid-test

Restarting Anvil resets all state. You must re-deploy the contracts (Step 2) and
re-fund the derived address (Step 3). The test's `ensureFaucetMpcAddress()`
will re-set the MPC address automatically.

### "Transaction Invalid" in server logs

Usually a stale nonce. Restart the server to reset its internal state.

### Governance referendum takes too long

On first run, the test may need to submit Root governance referendums on Lark
(e.g., to mint WETH for Alice or unpause the dispenser). This can take several
minutes per referendum. Subsequent runs skip these if state is already correct.
