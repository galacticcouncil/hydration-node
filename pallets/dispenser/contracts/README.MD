# Gas Faucet & Gas Voucher Contracts

This repository contains a small on-chain gas funding system built with **Solidity** and **Foundry**:

- **GasFaucet** – holds ETH and either:
  - sends ETH directly to users when the faucet has enough balance above a configured threshold, or
  - mints **GasVoucher** IOU tokens when it doesn’t.
- **GasVoucher** – an ERC20 “gas IOU” token that can be minted and burned by authorized faucet contracts.

The project uses:

- [Foundry](https://book.getfoundry.sh/) (forge, cast, anvil)
- [OpenZeppelin Contracts](https://github.com/OpenZeppelin/openzeppelin-contracts) (`Ownable`, `ERC20`, `AccessControl`)
- [forge-std](https://github.com/foundry-rs/forge-std) for tests and scripts

---

## 1. Project Structure

Relevant files:

- `src/GasFaucet.sol` – main faucet contract
- `src/GasVoucher.sol` – ERC20 IOU token
- `src/interfaces/IGasFaucet.sol` – faucet interface
- `src/interfaces/IGasVoucher.sol` – voucher interface
- `src/utils/Errors.sol` – shared custom errors
- `test/GasFaucet.t.sol` – tests for `GasFaucet`
- `test/GasVoucher.t.sol` – tests for `GasVoucher`
- `script/GasFaucet.s.sol` – deployment script (Used to deploy to Sepolia or other networks)

Note: `lib/` is **gitignored**, so dependencies must be installed locally.

---

## 2. Prerequisites (Install Once)

### 2.1 Install Foundry

If you don’t have Foundry installed:

```bash
curl -L https://foundry.paradigm.xyz | bash
foundryup
```

Verify:

```bash
forge --version
cast --version
```

---

## 4. Install Dependencies (lib/ is gitignored)

From the project root:

```bash
# OpenZeppelin Contracts (Ownable, ERC20, AccessControl, etc.)
forge install openzeppelin/openzeppelin-contracts --no-commit

# forge-std (Test, Script, console.log, etc.)
forge install foundry-rs/forge-std --no-commit
```

This will populate the `lib/` directory with the required dependencies.

---

## 5. Build / Compile the Contracts

From the project root:

```bash
forge build
```

If successful, you’ll see something like:

```text
Compiling 5 files with 0.8.13
Compiler run successful
```

Artifacts are generated in `out/`.

---

## 6. Run Tests

### 6.1 Run All Tests

```bash
forge test
```

### 6.2 Run Only Faucet Tests

```bash
forge test --match-contract GasFaucetTest
```

### 6.3 Run Only Voucher Tests

```bash
forge test --match-contract GasVoucherTest
```

---

## 7. Check Test Coverage (Optional)

To see coverage summary:

```bash
forge coverage
```

---

## 8. Contract Behavior Overview

### 8.1 GasFaucet

Key state:

- `owner()` – faucet owner (set to deployment EOA by OpenZeppelin `Ownable`)
- `mpc()` – address allowed to call `fund` (off-chain MPC / backend)
- `minEthThreshold()` – minimum ETH balance (in wei) that must remain in the contract after funding
- `voucher()` – address of the `GasVoucher` used for IOUs

Key functions (external):

- `setMPC(address _mpc)` – owner-only, sets the MPC address
- `setMinEthThreshold(uint256 _thresholdWei)` – owner-only
- `setVoucher(address _voucher)` – owner-only
- `fund(address to, uint256 amountWei)` – MPC-only
  - If faucet has enough ETH and will stay above `minEthThreshold` → sends ETH
  - Else → mints `GasVoucher` to `to`
- `redeem(uint256 amountWei)` – user burns vouchers to receive ETH (if faucet has enough ETH)
- `withdraw(address payable to, uint256 amountWei)` – owner-only, withdraws ETH from faucet

### 8.2 GasVoucher

Key features:

- `ERC20("Gas Voucher", "GVCH")`
- `AccessControl` roles:
  - `DEFAULT_ADMIN_ROLE` – can manage roles
  - `FAUCET_ROLE` – can `faucetMint` and `faucetBurnFrom`

Key functions:

- `setFaucet(address _faucet)` – admin-only, grants `FAUCET_ROLE`
- `faucetMint(address to, uint256 amount)` – faucet-only, mints vouchers
- `faucetBurnFrom(address from, uint256 amount)` – faucet-only, burns vouchers

---

## 9. Deployment to Sepolia

Deployment uses a Foundry script (e.g. `script/GasFaucet.s.sol`) that:

1. Deploys `GasVoucher`
2. Deploys `GasFaucet`
3. Grants `FAUCET_ROLE` to `GasFaucet`
4. Optionally funds `GasFaucet` with some ETH

---

## 10. Step-by-Step: Deploy to Sepolia

### Step 1: Get a Sepolia RPC URL

Register with a provider (Alchemy, Infura, QuickNode, etc.) and obtain a Sepolia endpoint, e.g.:

```text
https://sepolia.infura.io/v3/<project-id>
```

We will refer to this as `SEPOLIA_RPC_URL`.

### Step 2: Fund Your Deployer Address on Sepolia

- Choose an EOA that will be the “owner” of the contracts.
- Get its private key (for local deployment only; never commit).
- Send some **Sepolia ETH** to that address using a Sepolia faucet.

### Step 3: Set Environment Variables

In your shell (do **not** commit these):

```bash
export PRIVATE_KEY=0xYourDeployerPrivateKey
export MPC_ADDRESS=0xYourMpcAddress       # This address will be allowed to call `fund`
export SEPOLIA_RPC_URL=https://...        # Your Sepolia RPC URL
```

### Step 4: Dry-Run the Deployment (No Broadcast)

This simulates the script against Sepolia state:

```bash
forge script script/GasFaucet.s.sol:GasFaucetScript   --rpc-url $SEPOLIA_RPC_URL
```

You should see the simulated deployment output in the console.

### Step 5: Deploy (Broadcast Transactions)

To actually deploy to Sepolia:

```bash
forge script script/GasFaucet.s.sol:GasFaucetScript   --rpc-url $SEPOLIA_RPC_URL   --broadcast
```

You’ll see:

- Deployer address
- Deployed `GasVoucher` address
- Deployed `GasFaucet` address
- Confirmation that `GasFaucet` is set as faucet in `GasVoucher`
- Faucet’s initial ETH balance

### Step 6 (Optional): Verify on Etherscan

If you have an Etherscan API key (for Sepolia):

```bash
export ETHERSCAN_API_KEY=YourEtherscanApiKey

forge script script/GasFaucet.s.sol:GasFaucetScript   --rpc-url $SEPOLIA_RPC_URL   --broadcast   --verify
```

Your contracts will then be verified and readable on Sepolia Etherscan.

---
