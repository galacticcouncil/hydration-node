# Gas Faucet and Gas Voucher Mainnet Deployment (Foundry)

This repo includes two contracts:

- GasVoucher
- GasFaucet

This README covers a clean mainnet deployment flow using Foundry.

## Requirements

- Foundry (forge, cast)
- Ethereum mainnet RPC URL (Alchemy, Infura, or your own node)
- Deployer wallet funded with about 1.1 to 1.2 ETH (includes 1 ETH faucet funding)
- MPC address (the only address allowed to call fund)

Optional:

- Etherscan API key for contract verification

## Quick start

From the contracts folder:

```bash
cd pallets/dispenser/contracts
forge install
cp .env.example .env
```

Edit `.env`:

```bash
PRIVATE_KEY=0x...
MPC_ADDRESS=0x...
RPC_URL=https://...
ETHERSCAN_API_KEY=...   # optional
```

Do not commit `.env`.

## Pre deploy checks

### 1) Deployer address and balance

```bash
export DEPLOYER_ADDRESS=$(cast wallet address $PRIVATE_KEY)
cast balance $DEPLOYER_ADDRESS --rpc-url $RPC_URL
```

Make sure you have enough ETH for deployment plus 1 ETH funding.

### 2) Gas price

```bash
cast gas-price --rpc-url $RPC_URL
```

### 3) Confirm script parameters

In `script/Deployment.sol`, confirm:

- `MIN_ETH_THRESHOLD = 0.1 ether`
- `INITIAL_FUNDING = 1 ether`

### 4) Dry run the script (no broadcast)

```bash
forge script script/Deployment.sol:GasFaucetScript   --rpc-url $RPC_URL   --private-key $PRIVATE_KEY   --sender $DEPLOYER_ADDRESS
```

Check the printed deployer address and MPC address.

## Deploy to mainnet

### Recommended (broadcast + verify)

```bash
forge script script/Deployment.sol:GasFaucetScript   --rpc-url $RPC_URL   --private-key $PRIVATE_KEY   --broadcast   --verify   --slow
```

## Post deploy verification

### GasVoucher checks

Admin role:

```bash
cast call $GAS_VOUCHER_ADDRESS   "hasRole(bytes32,address)(bool)"   0x0000000000000000000000000000000000000000000000000000000000000000   $DEPLOYER_ADDRESS   --rpc-url $RPC_URL
```

Faucet role:

```bash
cast call $GAS_VOUCHER_ADDRESS   "hasRole(bytes32,address)(bool)"   $(cast keccak "FAUCET_ROLE()")   $GAS_FAUCET_ADDRESS   --rpc-url $RPC_URL
```

Both should return `true`.

### GasFaucet checks

```bash
cast call $GAS_FAUCET_ADDRESS "mpc()(address)" --rpc-url $RPC_URL
cast call $GAS_FAUCET_ADDRESS "minEthThreshold()(uint256)" --rpc-url $RPC_URL
cast call $GAS_FAUCET_ADDRESS "voucher()(address)" --rpc-url $RPC_URL
cast call $GAS_FAUCET_ADDRESS "owner()(address)" --rpc-url $RPC_URL
cast balance $GAS_FAUCET_ADDRESS --rpc-url $RPC_URL
```

Expected:

- mpc matches MPC_ADDRESS
- minEthThreshold is 0.1 ETH (100000000000000000)
- balance is 1 ETH
- voucher matches GAS_VOUCHER_ADDRESS
- owner matches deployer

## Security notes

- MPC address is critical. If it is wrong, fund calls will not work.
- Prefer a hardware wallet for mainnet deploys.
- After deployment, move ownership and admin to a multisig.

### Transfer ownership to multisig

```bash
cast send $GAS_FAUCET_ADDRESS   "transferOwnership(address)"   $MULTISIG_ADDRESS   --private-key $PRIVATE_KEY   --rpc-url $RPC_URL
```

### Grant multisig admin role in GasVoucher

```bash
cast send $GAS_VOUCHER_ADDRESS   "grantRole(bytes32,address)"   0x0000000000000000000000000000000000000000000000000000000000000000   $MULTISIG_ADDRESS   --private-key $PRIVATE_KEY   --rpc-url $RPC_URL
```

Optional, remove deployer admin role:

```bash
cast send $GAS_VOUCHER_ADDRESS   "renounceRole(bytes32,address)"   0x0000000000000000000000000000000000000000000000000000000000000000   $DEPLOYER_ADDRESS   --private-key $PRIVATE_KEY   --rpc-url $RPC_URL
```

## Common issues

### Insufficient funds

```bash
cast balance $(cast wallet address $PRIVATE_KEY) --rpc-url $RPC_URL
```

### Nonce too low or too high

```bash
cast nonce $(cast wallet address $PRIVATE_KEY) --rpc-url $RPC_URL
```

### CREATE2 address already used

You must change the CREATE2 salt in the deployment script, or deploy from a different deployer address.

### Force a gas price

Example 20 gwei:

```bash
forge script script/Deployment.sol:GasFaucetScript   --rpc-url $RPC_URL   --private-key $PRIVATE_KEY   --broadcast   --gas-price 20000000000
```
