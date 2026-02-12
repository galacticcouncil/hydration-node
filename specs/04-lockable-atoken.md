# Spec 04: LockableAToken (Solidity)

**Status:** Draft
**Depends on:** Spec 03 (pallet-gigahdx-voting for lock state)

---

## 1. Overview

`LockableAToken` is a custom AAVE v3 AToken contract that adds lock-awareness. It extends the standard `AToken` with a single restriction: locked tokens cannot be transferred or withdrawn (burned).

Lock state is managed entirely on the Substrate side (by pallet-gigahdx-voting). The contract reads lock state via a precompile — it never writes locks itself.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Read-only lock queries | Lock lifecycle managed by Substrate pallets, not Solidity |
| Single precompile call | Minimal EVM ↔ Substrate bridge surface |
| No liquidation override | Liquidation pallet removes locks on Substrate side before MM liquidation proceeds |
| Generic `LockableAToken` name | Reusable pattern, not GIGAHDX-specific |

### What This Contract Does NOT Do

- Does not set or remove locks (Substrate side handles this)
- Does not handle liquidation lock removal (pallet-liquidation does this before calling MM)
- Does not track lock duration or conviction (pallet-gigahdx-voting does this)

---

## 2. Contract

```solidity
// SPDX-License-Identifier: BUSL-1.1
pragma solidity ^0.8.10;

import {IERC20} from '../../dependencies/openzeppelin/contracts/IERC20.sol';
import {GPv2SafeERC20} from '../../dependencies/gnosis/contracts/GPv2SafeERC20.sol';
import {AToken} from './AToken.sol';
import {IPool} from '../../interfaces/IPool.sol';

interface ILockManager {
    function getLockedBalance(address token, address account) external view returns (uint256);
}

contract LockableAToken is AToken {
    using GPv2SafeERC20 for IERC20;

    address public constant LOCK_MANAGER = 0x0000000000000000000000000000000000000806;

    error ExceedsFreeBalance(uint256 requested, uint256 available);

    constructor(IPool pool) AToken(pool) {}

    function getFreeBalance(address account) public view returns (uint256) {
        uint256 total = balanceOf(account);
        uint256 locked = getLockedBalance(account);
        return locked >= total ? 0 : total - locked;
    }

    function getLockedBalance(address account) public view returns (uint256) {
        return ILockManager(LOCK_MANAGER).getLockedBalance(address(this), account);
    }

    function burn(
        address from,
        address receiverOfUnderlying,
        uint256 amount,
        uint256 index
    ) external virtual override onlyPool {
        uint256 freeBalance = getFreeBalance(from);
        if (amount > freeBalance) revert ExceedsFreeBalance(amount, freeBalance);

        _burnScaled(from, receiverOfUnderlying, amount, index);
        if (receiverOfUnderlying != address(this)) {
            IERC20(_underlyingAsset).safeTransfer(receiverOfUnderlying, amount);
        }
    }

    function _transfer(
        address from,
        address to,
        uint256 amount,
        bool validate
    ) internal virtual override {
        uint256 freeBalance = getFreeBalance(from);
        if (amount > freeBalance) revert ExceedsFreeBalance(amount, freeBalance);
        super._transfer(from, to, amount, validate);
    }
}
```

---

## 3. Precompile: ILockManager

The contract queries lock state via a Substrate precompile at address `0x0000000000000000000000000000000000000806`.

### 3.1 Interface

```solidity
interface ILockManager {
    /// Returns the locked balance for a given token and account.
    /// @param token The ERC20 token address (LockableAToken contract address)
    /// @param account The account to query
    /// @return The locked amount (0 if no locks)
    function getLockedBalance(address token, address account) external view returns (uint256);
}
```

### 3.2 Substrate Precompile Implementation

The precompile reads lock state from pallet-gigahdx-voting storage.

```rust
/// Precompile at address 0x0806
/// Reads GIGAHDX voting lock from pallet-gigahdx-voting storage.
pub struct LockManagerPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> LockManagerPrecompile<Runtime>
where
    Runtime: pallet_gigahdx_voting::Config + pallet_evm::Config,
{
    /// Selector: getLockedBalance(address,address)
    #[precompile::public("getLockedBalance(address,address)")]
    #[precompile::view]
    fn get_locked_balance(
        handle: &mut impl PrecompileHandle,
        token: Address,
        account: Address,
    ) -> EvmResult<U256> {
        let account_id = Runtime::AddressMapping::into_account_id(account.into());

        // Read lock from pallet-gigahdx-voting storage
        let locked = pallet_gigahdx_voting::GigaHdxVotingLock::<Runtime>::get(&account_id)
            .unwrap_or_default();

        Ok(locked.into())
    }
}
```

### 3.3 Precompile Registration

```rust
// In runtime EVM precompile set
(0x0806, LockManagerPrecompile<Runtime>),
```

---

## 4. Overridden Functions

### 4.1 `burn` (withdrawal from Money Market)

**Original behavior:** Burns aTokens and transfers underlying asset to receiver.

**Override:** Checks free balance before burning. Reverts with `ExceedsFreeBalance` if the requested amount exceeds the unlocked portion.

**When it's called:**
- User withdraws stHDX from Money Market (standard MM withdrawal)
- pallet-gigahdx calls MM withdraw during `giga_unstake`

**Flow:**
```
User calls MM withdraw
  → Pool calls LockableAToken.burn(from, receiver, amount, index)
    → getFreeBalance(from) = balanceOf(from) - getLockedBalance(from)
    → if amount > freeBalance → revert ExceedsFreeBalance
    → _burnScaled(from, receiver, amount, index)
    → transfer underlying stHDX to receiver
```

**Important:** During `giga_unstake`, the Substrate pallet removes locks BEFORE calling MM withdraw, so the burn always succeeds for the unstaked amount.

### 4.2 `_transfer` (ERC20 transfer)

**Original behavior:** Transfers aTokens between accounts.

**Override:** Checks free balance before transferring. Reverts with `ExceedsFreeBalance` if the requested amount exceeds the unlocked portion.

**When it's called:**
- Direct ERC20 transfer of GIGAHDX between accounts
- Internal transfers within Money Market operations

### 4.3 Functions NOT Overridden

| Function | Why |
|----------|-----|
| `mint` | No lock check needed — minting adds tokens, doesn't move locked ones |
| `transferOnLiquidation` | Locks are removed by Substrate pallet BEFORE liquidation reaches the contract |
| `balanceOf` | Returns total balance (locked + unlocked) — needed for MM accounting |
| `totalSupply` | Unchanged — total supply is independent of locks |

---

## 5. Balance Model

```
┌─────────────────────────────────────────┐
│            balanceOf(account)            │
│         (total GIGAHDX balance)          │
│                                         │
│  ┌──────────────┐  ┌─────────────────┐  │
│  │   locked      │  │     free        │  │
│  │  (voting)     │  │  (transferable) │  │
│  └──────────────┘  └─────────────────┘  │
└─────────────────────────────────────────┘

locked      = ILockManager.getLockedBalance(token, account)
free        = balanceOf(account) - locked
transferable = free (can transfer, withdraw, etc.)
```

- `balanceOf()` — total balance, unchanged from AToken (includes locked)
- `getLockedBalance()` — reads from Substrate precompile
- `getFreeBalance()` — total minus locked (what can be transferred/burned)

**Invariant:** `getFreeBalance(account) + getLockedBalance(account) <= balanceOf(account)`

The `<=` case handles the edge where `locked >= total`, which returns `free = 0` (no underflow).

---

## 6. Interaction Scenarios

### 6.1 Normal Transfer

```
User has 1000 GIGAHDX, 400 locked for voting

transfer(to, 600) → getFreeBalance = 1000 - 400 = 600 → OK
transfer(to, 601) → getFreeBalance = 600, 601 > 600 → revert ExceedsFreeBalance
```

### 6.2 MM Withdrawal (No Locks)

```
User has 1000 GIGAHDX, 0 locked

MM withdraw(500) → Pool calls burn(from, receiver, 500, index)
  → getFreeBalance = 1000 - 0 = 1000
  → 500 <= 1000 → OK → burn proceeds
```

### 6.3 MM Withdrawal (With Locks — Blocked)

```
User has 1000 GIGAHDX, 800 locked for voting

MM withdraw(500) → Pool calls burn(from, receiver, 500, index)
  → getFreeBalance = 1000 - 800 = 200
  → 500 > 200 → revert ExceedsFreeBalance
```

User must use `giga_unstake` instead, which removes locks first.

### 6.4 Giga-unstake Flow

```
User has 1000 GIGAHDX, 800 locked, wants to unstake 500

1. pallet-gigahdx calls T::Hooks::on_unstake()
   → pallet-gigahdx-voting removes votes from finished referenda
   → voting locks reduced/removed in Substrate storage

2. pallet-gigahdx calls MM withdraw(500)
   → Pool calls burn(from, receiver, 500, index)
   → getFreeBalance now reflects updated lock state from step 1
   → burn proceeds
```

### 6.5 Liquidation Flow

```
User has 1000 GIGAHDX, 800 locked, position undercollateralized

1. pallet-liquidation removes all voting locks on Substrate side
   → GigaHdxVotingLock storage cleared for account

2. pallet-liquidation calls MM liquidation
   → Pool calls transferOnLiquidation(from, treasury, amount)
   → Standard AToken._transfer (no lock check needed, locks already removed)
```

---

## 7. Error Handling

| Error | Triggered When |
|-------|----------------|
| `ExceedsFreeBalance(requested, available)` | Transfer or burn amount exceeds free (unlocked) balance |

The error includes both the requested amount and available free balance for debugging.

---

## 8. Deployment

### 8.1 Deployment Parameters

The `LockableAToken` is deployed as the aToken implementation for the stHDX reserve in the AAVE v3 Money Market.

```
constructor(IPool pool)
```

- `pool`: The AAVE v3 Pool contract address

### 8.2 Initialization

After deployment, the token is initialized as part of AAVE v3 reserve setup:

```
initialize(
    pool,           // IPool
    treasury,       // Treasury address
    underlyingAsset, // stHDX ERC20 address
    incentivesController,
    aTokenDecimals,  // 12 (matching HDX)
    aTokenName,      // "GIGAHDX"
    aTokenSymbol,    // "GIGAHDX"
    params
)
```

### 8.3 LOCK_MANAGER Address

The precompile address `0x0000000000000000000000000000000000000806` must be registered in the runtime's precompile set before the contract is deployed.

---

## 9. File Structure

```
aave-protocol/contracts/protocol/tokenization/
├── AToken.sol                  # Base AAVE v3 aToken (unmodified)
└── LockableAToken.sol          # This spec: lock-aware aToken

precompiles/lock-manager/       # Substrate precompile
├── Cargo.toml
├── src/
│   └── lib.rs                  # LockManagerPrecompile implementation
```
