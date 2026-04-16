# GIGAHDX Specification

**Status:** Draft
**Version:** 0.3
**Date:** February 2025

---

## 1. Overview

GIGAHDX is a Liquid Staking Token (LST) for HDX designed to:

1. Provide automatic value accrual — holders benefit from appreciating exchange rate simply by holding GIGAHDX, with no manual claiming required
2. Enable HDX to be used as collateral in the Money Market while continuing to accrue value
3. Maintain voting capabilities for governance participation
4. Offer additional rewards for active governance participants through referenda voting incentives

---

## 2. Problem Statement

- Current HDX staking has low APR with diminishing incentive for long-term stakers
- Users at 100% claimable rewards have little reason to continue high-conviction participation
- Strong user demand to use HDX as collateral without missing DeFi opportunities
- Need to better align rewards with commitment level

---

## 3. Configurable Parameters

**Key feature:** After giga-unstake, there is a cooldown period (~222 days) before HDX can be claimed. This is a major differentiator from classic HDX staking which has no cooldown.

| Parameter                     | Description                                                           | Example/Suggested Value |
| ----------------------------- | --------------------------------------------------------------------- | ----------------------- |
| **pallet-gigahdx**            |                                                                       |                         |
| Cooldown period               | Lock duration after giga-unstake before HDX can be claimed            | ~222 days               |
| Min stake amount              | Minimum HDX to stake / minimum to hold after partial unstake          | TBD                     |
| stHDX asset ID                | Asset ID for stHDX token                                              | TBD                     |
| HDX asset ID                  | Asset ID for HDX token                                                | 0                       |
| GIGAHDX asset ID              | Asset ID for GIGAHDX token (MM aToken)                                | TBD                     |
| **pallet-gigahdx-voting**     |                                                                       |                         |
| GigaRewardPotId               | PalletId for referenda reward pot                                     | `*b"gigarwrd"`          |
| MaxVotes                      | Maximum active votes per account                                      | 25                      |
| VoteLockingPeriod             | Base voting lock period (multiplied by conviction)                    | 7 days                  |
| Per-track reward %            | Reward percentage per governance track (root, treasurer, etc.)        | 5%–15% per track        |
| **pallet-fee-processor**      |                                                                       |                         |
| HDX Staking Pot %             | Percentage of fees to classic HDX stakers                             | ~20%                    |
| stHDX Pot %                   | Percentage of fees to GIGAHDX value accrual                           | ~50%                    |
| GigaReward Pot %              | Percentage of fees to voting rewards                                  | ~30%                    |
| **Money Market**              |                                                                       |                         |
| LTV (Loan-to-Value)           | Maximum borrow ratio                                                  | ~40%                    |
| LT (Liquidation Threshold)    | Threshold for liquidation                                             | ~50%                    |
| Debt ceiling                  | Maximum total debt for GIGAHDX collateral                             | TBD                     |
| Oracle EMA period             | EMA period for price oracle                                           | TBD                     |

---

## 4. Architecture

Three new pallets and one Solidity contract will be introduced:

| Component | Responsibility |
|-----------|----------------|
| `pallet-gigahdx` | Core staking mechanics: giga_stake, giga_unstake, unlock, exchange rate |
| `pallet-gigahdx-voting` | Voting adapter for conviction-voting, vote tracking, conviction-weighted referenda rewards |
| `pallet-fee-processor` | Fee accumulation, HDX buyback via omnipool, dynamic pot distribution |
| `LockableAToken` (Solidity) | Custom AAVE v3 aToken that enforces voting locks on transfers/withdrawals |

**Dependency graph:**
```
pallet-gigahdx-voting
    ├── pallet-gigahdx (direct: stake_rewards, gigapot_account_id)
    ├── pallet-conviction-voting (provides VotingHooks, Currency adapter)
    └── pallet-referenda (referendum state queries)

pallet-gigahdx
    └── pallet-gigahdx-voting (via GigaHdxHooks trait, loose coupling)

LockableAToken (EVM)
    └── LockManager precompile (0x0806) → reads GigaHdxVotingLock storage
```

Classic HDX staking remains available alongside GIGAHDX.

---

## 5. Token Representation

### 5.1 Tokens Overview

| Token | Description | Visibility | Requirements |
|-------|-------------|------------|--------------|
| HDX | Native token | Visible | — |
| stHDX | Intermediate staking token (received when staking HDX) | Mostly hidden from users | Transferable, non-tradable (no market/exchange listing) |
| GIGAHDX | Money Market token (received when supplying stHDX) | Visible | Transferable (unless locked), non-tradable, usable as collateral in AAVE v3 MM, usable for governance voting (with locking mechanism) |

Users interact with HDX and GIGAHDX on the frontend. stHDX is an intermediate token that exists for technical reasons but is abstracted away from the user experience.

**Note:** stHDX and GIGAHDX are interchangeable 1:1. The issuance of stHDX and GIGAHDX is always the same — supplying 100 stHDX to MM yields exactly 100 GIGAHDX, and withdrawing 100 GIGAHDX returns exactly 100 stHDX.

**Note on Value Accrual:** In isolation mode, users supply stHDX as collateral to borrow HOLLAR — nobody borrows stHDX itself. This means GIGAHDX rebasing from borrow interest is effectively zero. All value accrual comes from the stHDX pot (fee buybacks), not from Money Market interest.

### 5.2 GIGAHDX Token Implementation

GIGAHDX (the Money Market token) requires a custom ERC20 implementation that combines:

1. **AToken functionality** — standard AAVE aToken behavior for Money Market integration
2. **Locking mechanism** — ability to lock tokens for a specified period (for governance voting)

#### 5.2.1 Locking Mechanism

Lock management is entirely on the Substrate side. The `LockableAToken` EVM contract is **read-only** — it queries lock state via a precompile but never writes locks.

```
WRITE PATH (Substrate-only):
  conviction-voting calls set_lock(amount)
    → GigaHdxVotingCurrency adapter
      → GigaHdxVotingLock storage (GIGAHDX portion)
      → NativeCurrency::set_lock (HDX portion)

READ PATH (EVM):
  LockableAToken._transfer() / .burn()
    → precompile 0x0806: getLockedBalance(token, account)
      → reads GigaHdxVotingLock storage
    → enforces: amount <= balanceOf - locked
```

**Key properties:**
- Users can lock GIGAHDX for voting with full conviction support (None through 6x)
- Locked GIGAHDX cannot be transferred or withdrawn from MM (enforced by LockableAToken contract)
- The only way to withdraw locked GIGAHDX is via giga-unstake, which removes votes first
- Liquidation force-removes all votes (including ongoing referenda), which clears locks through the adapter
- No EVM callbacks or bidirectional communication needed

### 5.3 Token Flow

From the user's perspective, the flow is simplified:

**Flow:**
```
1. Gigastake:    HDX → GIGAHDX
2. Borrow:       HOLLAR against GIGAHDX
3. Giga-unstake: GIGAHDX → HDX (after cooldown period)
```

Internally, gigastake handles: HDX → stHDX → supply to MM → GIGAHDX
Internally, giga-unstake handles: remove locks → withdraw from MM → stHDX → HDX

**Pros:**
- Standard AAVE v3 patterns preserved (no MM modification)
- Clean separation: stHDX mechanics vs AAVE mechanics
- User sees simplified view: HDX and GIGAHDX only

**Cons:**
- Edge case: user could theoretically end up holding only stHDX (which is hidden on frontend), however by design this should not be possible since gigastake always wraps to GIGAHDX and giga-unstake handles the full unwrap

---

## 6. GIGAHDX Value Mechanism

**Note:** This is technically the stHDX value mechanism, as stHDX is the underlying vault token that appreciates. Since stHDX and GIGAHDX are 1:1 interchangeable, the value accrual applies equally to GIGAHDX holders.

GIGAHDX operates as a vault token with appreciating exchange rate.

### 6.1 Exchange Rate

At launch, the ratio is 1:1. As rewards accrue to the pot, the HDX-per-GIGAHDX ratio increases. Later entrants receive fewer GIGAHDX per HDX deposited.

**Formulas:**

```
Exchange rate (HDX per GIGAHDX) = Total HDX in gigapot / Total stHDX supply

GIGAHDX received when staking = HDX deposited / Exchange rate

HDX received when unstaking = GIGAHDX burned × Exchange rate
```

**Note:** The exchange rate calculation uses stHDX supply internally (since stHDX and GIGAHDX are 1:1 interchangeable).

### 6.2 Example

1. User 1 deposits 1000 HDX → receives 1000 GIGAHDX
2. User 2 deposits 500 HDX → receives 500 GIGAHDX
3. 100 HDX accrues from fees into pot
4. Total pot: 1600 HDX, Total supply: 1500 GIGAHDX
5. Exchange rate: 1.067 HDX per GIGAHDX
6. User 3 deposits 500 HDX → receives 469 GIGAHDX
7. If User 3 withdraws immediately: receives 500 HDX (no profit from others' rewards)

---

## 7. pallet-gigahdx

### 7.1 Core Functions

#### 7.1.1 Gigastake

Converts HDX to GIGAHDX:
1. Transfers HDX from user to special account (gigapot)
2. Mints stHDX based on current stHDX:HDX exchange rate
3. Immediately supplies stHDX to Money Market as collateral
4. User receives GIGAHDX

User earns staking rewards as long as they hold GIGAHDX. The intermediate stHDX token is not visible to the user.

#### 7.1.2 Giga-unstake

Converts GIGAHDX to HDX:
1. Checks `can_unstake(who)` — **blocks** if user has votes in ongoing referenda
2. Captures `additional_unstake_lock(who)` — max remaining voting lock duration
3. Calls `on_unstake(who)` — force-removes votes from finished referenda, records rewards
4. Withdraws from Money Market — user's GIGAHDX is returned as stHDX
5. Burns stHDX from user
6. Calculates HDX amount based on current exchange rate
7. Creates pending unlock position with dynamic cooldown: `max(base_222_days, remaining_voting_lock)`

**Note:** Giga-unstake is the only way to withdraw locked GIGAHDX. Standard MM withdrawal will fail if GIGAHDX is locked. Steps 1-3 handle vote removal through `pallet-gigahdx-voting`, which clears locks through the adapter before MM withdrawal proceeds.

Notes:
- Allows partial unstake, but must respect minimum staked amount
- User can have multiple concurrent unlock positions
- If user has votes in ongoing referenda, unstake fails — user must wait or manually remove votes
- Dynamic cooldown ensures voting lock timing is respected (e.g., Locked6x extends cooldown to 224 days)

#### 7.1.3 Claim Unlock

Claims HDX after cooldown period expires.

#### 7.1.4 Migrate

One-time migration from existing staked HDX to GIGAHDX directly:
1. Converts all staked HDX plus all unclaimed rewards (regardless of claim status)
2. Mints stHDX at current exchange rate
3. Immediately supplies stHDX to Money Market
4. User receives GIGAHDX

#### 7.1.5 Claim Rewards

Claims conviction-weighted rewards from governance participation. This extrinsic is on `pallet-gigahdx-voting` (see spec 03, section 9).

**Flow:**
1. Retrieves all pending reward entries for the user (recorded when votes were removed)
2. For each entry: transfers HDX from referenda-specific reward pot to gigapot
3. Calls `pallet_gigahdx::stake_rewards()` to convert HDX → GIGAHDX
4. User receives GIGAHDX as reward
5. Claimed entries cleared from storage

**Note:** Rewards become claimable only after the user removes their vote from a finished referenda (Approved or Rejected). Rewards are conviction-weighted — see Section 10.4.

### 7.2 Configuration

| Parameter | Description |
|-----------|-------------|
| Cooldown period | Lock duration in blocks after giga-unstake |
| Min stake amount | Minimum HDX to stake / minimum GIGAHDX to hold after partial unstake |
| stHDX asset ID | Asset ID for stHDX token |
| HDX asset ID | Asset ID for HDX token |
| GIGAHDX asset ID | Asset ID for GIGAHDX token (Money Market aToken) |

---

## 8. pallet-fee-processor

### 8.1 Purpose

Consolidates fee handling currently spread across referral pallet:
1. Accumulate trading fees (various assets)
2. Buyback HDX via omnipool
3. Distribute to registered pots based on configured percentages

### 8.2 Note on Refactoring

This functionality is currently handled by `pallet-referrals`:
- Fee accumulation
- HDX buybacks (in `on_idle` hook)
- Distribution to staking pot (fixed, single pot)

This logic needs to be extracted from `pallet-referrals` into the new `pallet-fee-processor`. The referrals pallet should be refactored to delegate fee processing to this new pallet, retaining only referral-specific logic.

### 8.3 Pot System

Pots can be registered and updated dynamically via governance. Initial pots:

| Pot | Description |
|-----|-------------|
| HDX Staking Pot | Classic HDX stakers (existing) |
| stHDX Pot | Auto-compounds into stHDX value |
| stHDX Reward Pot | Active participation rewards (e.g., voting incentives) |

### 8.4 Buyback Mechanism

- Runs in `on_idle` hook
- Converts accumulated fee assets → HDX via omnipool
- Distributes HDX to pots based on configured percentages
- Available for any component that needs asset exchange (not just GIGAHDX)

---

## 9. Voting Integration

### 9.1 Overview

Voting uses the standard `pallet-conviction-voting` — no modifications to the conviction voting pallet are required. A separate pallet (`pallet-gigahdx-voting`) provides the custom Currency adapter and VotingHooks implementation.

**Key points:**
- Users can vote with combined GIGAHDX + HDX balance using **full conviction support** (None through Locked6x)
- Higher conviction = longer lock period and higher reward multiplier
- Governance locks and staking cooldown **do not stack** — they are independent mechanisms
- GIGAHDX is locked first (prioritized over HDX) to incentivize governance participation from GIGAHDX holders

### 9.2 Current Implementation

The conviction voting pallet has a `Currency` config option:

```rust
type Currency: ReservableCurrency<Self::AccountId>
    + LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self, I>>
    + fungible::Inspect<Self::AccountId>;
```

Currently, this is configured to use an implementation from the balances pallet, which supports voting with the native HDX token only.

### 9.3 Custom Adapter: GigaHdxVotingCurrency

`pallet-gigahdx-voting` provides `GigaHdxVotingCurrency<T>`, a combined GIGAHDX + HDX currency adapter that implements `LockableCurrency`, `ReservableCurrency`, and `fungible::Inspect`. It is configured as the `Currency` type for `pallet-conviction-voting`.

The adapter:
1. **Returns combined balance** — `balance(who)` returns GIGAHDX + HDX
2. **Locks GIGAHDX first** — When a lock is required, GIGAHDX is consumed first, HDX for the remainder
3. **Tracks the split** — `LockSplit` storage records how much of a lock is GIGAHDX vs HDX
4. **Writes lock to Substrate storage** — `GigaHdxVotingLock` is written directly (same pallet), read by EVM via precompile

See spec 03, section 6 for full implementation details.

### 9.4 Lock Operations

#### set_lock

When `set_lock` is called with amount X:
1. Determine how much GIGAHDX user has
2. Calculate split: `gigahdx_lock = min(X, gigahdx_balance)`, `hdx_lock = X - gigahdx_lock`
3. Write `GigaHdxVotingLock` storage (precompile reads this — no EVM callback needed)
4. Call `NativeCurrency::set_lock` for HDX portion

#### remove_lock

1. Read `LockSplit` to determine GIGAHDX vs HDX portions
2. Remove `GigaHdxVotingLock` storage (precompile sees lock cleared)
3. Call `NativeCurrency::remove_lock` for HDX portion

### 9.5 Example

User has 1000 GIGAHDX + 2000 HDX, votes with 2500:
- Adapter returns total balance: 3000
- Lock requested: 2500
- GIGAHDX locked: 1000 (full balance)
- HDX locked: 1500 (remainder)

### 9.6 Resolved: Liquidation vs Voting Lock Conflict

When a GIGAHDX position with active voting locks needs to be liquidated:

1. `pallet-liquidation` calls `prepare_for_liquidation(who)` on `pallet-gigahdx-voting`
2. This **force-removes ALL votes** from conviction-voting (including ongoing referenda)
3. Each removal triggers the `on_remove_vote` hook — rewards are recorded for finished referenda
4. Conviction-voting recalculates locks → calls adapter's `remove_lock`/`set_lock`
5. `GigaHdxVotingLock` is cleared naturally through the adapter
6. EVM precompile sees lock cleared → `transferOnLiquidation` succeeds

Votes in ongoing referenda are invalidated — this is acceptable because liquidation is a forced event that takes priority over governance participation. Users can still claim any recorded rewards from finished referenda later.

See spec 03, section 10.1 for implementation details.

### 9.7 Resolved: Voting Lock vs Giga-unstake

When a user with active votes wants to giga-unstake:

- **Ongoing referenda:** Unstake is **blocked** with `ActiveVotesInOngoingReferenda` error. Users must wait for the referenda to finish or manually remove their votes first.
- **Finished referenda only:** Votes are **force-removed**, rewards recorded, and a dynamic cooldown is calculated:

```
remaining_voting_lock = max(lock_expires_at - current_block, 0) for all removed votes
actual_cooldown = max(base_cooldown_222_days, remaining_voting_lock)
```

This ensures voting lock timing is always respected. If a user voted with Locked6x (224 days), their cooldown extends to 224 days instead of the base 222 days.

See spec 03, sections 10.2 and 7.2 for implementation details.

---

## 10. Referenda Participation Rewards

This section describes how active Referenda participants will be rewarded. Implemented in `pallet-gigahdx-voting` (see spec 03 for full details).

**Important:**
- Rewards are distributed in **GIGAHDX**, not HDX (HDX is converted via internal giga_stake)
- Only votes made with **GIGAHDX** are eligible for rewards — votes made with HDX are not rewarded
- Rewards are **conviction-weighted** — higher conviction = proportionally higher rewards

### 10.1 Overview

Rewards for referenda participation are set up **after** a referenda finishes using a lazy approach. No temporary pots are created during the referenda.

**Important:** Only **Approved** or **Rejected** referenda receive rewards. **Cancelled** referenda do not trigger any reward allocation.

### 10.2 Vote Tracking

`pallet-gigahdx-voting` implements `VotingHooks` for `pallet-conviction-voting` to track GIGAHDX votes:
- `on_before_vote`: Records GIGAHDX portion of vote with conviction level and lock expiry
- `on_remove_vote`: Processes rewards for finished referenda, updates totals
- Only GIGAHDX portion of votes is tracked — HDX votes are not recorded

**Tracked per vote:** amount, conviction level, voted_at block, lock_expires_at block.

### 10.3 Lazy Reward Allocation

Rewards are allocated lazily — triggered by the first vote removal after a referenda finishes (Approved or Rejected only).

**When the first vote is removed from a finished referenda:**
1. Get the referenda's **track ID** and look up the **per-track reward percentage**
2. Take that percentage from the GigaReward Pot
3. Transfer to a referenda-specific reward pot account
4. Snapshot total conviction-weighted votes for reward calculation

**Per-track reward percentages** (configurable via `TrackRewardConfig`):

| Track | Reward % | Rationale |
|-------|----------|-----------|
| Root | 15% | Highest importance |
| Treasurer | 12% | Financial governance |
| Economic Parameters | 12% | Protocol economics |
| General Admin | 10% | General governance |
| Omnipool Admin | 10% | Pool governance |
| Spender | 8% | Spending proposals |
| Tipper | 5% | Lowest importance |
| Default | 10% | Other tracks |

**Cancelled referenda:** No rewards allocated. Users simply remove their votes.

### 10.4 Conviction-Weighted Reward Calculation

When a user removes their vote from a finished referenda, their reward is calculated using conviction-weighted amounts:

```
user_weighted_vote = gigahdx_amount × conviction_multiplier
user_reward = (user_weighted_vote / total_weighted_votes) × referenda_reward_pool
```

**Conviction multipliers** (matching standard Substrate conviction voting):

| Conviction | Multiplier | Lock Period |
|------------|------------|-------------|
| None | 0.1x | 0 |
| Locked1x | 1x | 1× base |
| Locked2x | 2x | 2× base |
| Locked3x | 3x | 4× base |
| Locked4x | 4x | 8× base |
| Locked5x | 5x | 16× base |
| Locked6x | 6x | 32× base |

**Example:**
- Referenda reward pool: 400 HDX
- User A: 100 GIGAHDX × Locked2x (2x) = 200 weighted
- User B: 200 GIGAHDX × Locked1x (1x) = 200 weighted
- Total weighted: 400

Rewards:
- User A: (200 / 400) × 400 = 200 HDX
- User B: (200 / 400) × 400 = 200 HDX

Equal rewards despite User B having more GIGAHDX, because User A committed with higher conviction.

### 10.5 Claiming Rewards

Users call `claim_rewards` on `pallet-gigahdx-voting`:

1. Retrieve all pending reward entries for the user
2. For each entry: transfer HDX from referenda pot to gigapot
3. Call `pallet_gigahdx::stake_rewards()` to convert HDX → GIGAHDX
4. User receives GIGAHDX as reward
5. Claimed entries cleared from storage

### 10.6 Referenda Rewards Storage

Stored in `pallet-gigahdx-voting`:

**Per vote (GigaHdxVotes):** account, referenda_id → amount, conviction, voted_at, lock_expires_at

**Per referenda (ReferendaTotalWeightedVotes):** referenda_id → total conviction-weighted votes

**Per finished referenda (ReferendaRewardPool):** track_id, total_reward, total_weighted_votes, remaining_reward, pot_account

**Per user (PendingRewards):** list of (referenda_id, reward_amount)

---

## 11. Money Market Integration

### 11.1 Price Oracle

GIGAHDX price in the Money Market is determined as:

```
GIGAHDX/USD = EMA oracle of HDX/USD × GIGAHDX/HDX ratio
```

### 11.2 Isolation Mode

GIGAHDX will be configured as isolated collateral in AAVE v3:

| Parameter | Value |
|-----------|-------|
| Collateral | stHDX |
| Borrowable assets | HOLLAR only |
| LTV (Loan-to-Value) | ~40% (conservative) |
| LT (Liquidation Threshold) | ~50% |
| Debt ceiling | Start low, increase over time |

### 11.3 Liquidation

GIGAHDX liquidation supports two complementary paths: **external liquidation** for unlocked positions and **treasury liquidation** (via the liquidation pallet) for locked positions. See `specs/05-gigahdx-liquidation.md` for the full detailed specification.

#### External Liquidation (Unlocked GIGAHDX)

Anyone can call the Aave Pool contract's `liquidationCall` directly to liquidate a GIGAHDX-collateralized position, provided:

1. The position is under-collateralized (HF < 1.0)
2. The GIGAHDX collateral is **NOT locked** for governance voting
3. The external liquidator has enough HOLLAR to repay the debt portion

If GIGAHDX is locked, the `LockableAToken._transfer()` will revert with `ExceedsFreeBalance`, and the liquidation fails. No pallet changes are needed for this path — it is handled entirely by the existing LockableAToken contract (Spec 04).

#### Treasury Liquidation (Locked GIGAHDX)

When GIGAHDX is locked for governance voting, the treasury liquidation path handles it. This is implemented as a new branch in the existing `liquidate` extrinsic in `pallet-liquidation`, triggered when `collateral_asset == GIGAHDX`.

**On-chain Liquidation Flow:**
1. Calls `prepare_for_liquidation(who)` — force-removes ALL votes from conviction-voting, which clears `GigaHdxVotingLock` naturally through the adapter (see section 9.6)
2. Treasury borrows HOLLAR against its own Money Market collateral (`Pool.borrow()`)
3. Treasury calls `Pool.liquidationCall(GIGAHDX, HOLLAR, user, amount, receive_atoken=true)` — receives GIGAHDX as aToken
4. Seized GIGAHDX is transferred to a derived treasury sub-account (derived from `BorrowingTreasuryAccount`)
5. External governance action decides what to do with the seized GIGAHDX afterward

**Key design decisions:**
- **No flash minting:** Treasury borrows HOLLAR directly — simpler than flash mint + repay round-trip
- **No profit check:** GIGAHDX is not swapped to debt asset; value is in the seized collateral itself
- **`receive_atoken = true`:** Seized collateral stays as GIGAHDX (not unwrapped to stHDX)
- **PEPL worker can trigger:** Unsigned transactions, same as other liquidations
- **Derived sub-account:** Keeps seized GIGAHDX separate from main treasury, trackable by governance

**Debt Ceiling:** Should be set based on what treasury can safely cover in case of liquidations. The treasury accumulates HOLLAR debt that must be managed through governance.

**Locked GIGAHDX:**
If the GIGAHDX being liquidated has governance voting locks, step 1 force-removes all votes from conviction-voting. This triggers the adapter's lock recalculation, which clears `GigaHdxVotingLock` storage. The EVM precompile sees the lock cleared, and `transferOnLiquidation` succeeds. See section 9.6 for the full resolved design.

### 11.4 Future Features (Out of Scope for Initial Launch)

| Feature | Description |
|---------|-------------|
| Immortal loans | Pay charge after liquidation to recover position |
| Reduced HOLLAR borrow APY | Preferential rate for GIGAHDX holders |
| Self-repaying loans | HOLLAR revenue distributed as loan repayment |

---

## 12. Migration

Existing staked HDX holders can migrate to GIGAHDX directly:

1. All staked HDX plus all unclaimed rewards converted at current exchange rate
2. stHDX is minted and immediately supplied to MM
3. User receives GIGAHDX
4. One-time opt-in per account

---

## 13. Parallel Workstreams

The following workstreams can be developed:

| # | Workstream | Detailed Spec | Can Parallel? |
|---|------------|---------------|---------------|
| 1 | `pallet-fee-processor` | spec 01 | Yes |
| 2 | `pallet-gigahdx` (core staking) | spec 02 | Yes |
| 3 | `pallet-gigahdx-voting` (adapter + rewards) | spec 03 | Partially (needs pallet-gigahdx interface) |
| 4 | `LockableAToken` + LockManager precompile | spec 04 | Yes |
| 5 | GIGAHDX liquidation (external + treasury) | spec 05 | After 2, 3, 4 are working |
| 6 | Migration from HDX staking | TBD (future) | After 2 is working |

Workstreams 1, 2, and 4 can be developed fully in parallel. Workstream 3 can be started in parallel but depends on pallet-gigahdx interfaces. Workstreams 5 and 6 are deferred until the foundation is implemented and working.

---

## 14. Detailed Specifications

| Spec | File | Covers |
|------|------|--------|
| 01 | `specs/01-pallet-fee-processor.md` | Fee accumulation, HDX buyback, pot distribution |
| 02 | `specs/02-pallet-gigahdx.md` | Core staking: giga_stake, giga_unstake, unlock, exchange rate |
| 03 | `specs/03-pallet-gigahdx-voting.md` | Voting adapter, VotingHooks, conviction-weighted rewards, liquidation prep |
| 04 | `specs/04-lockable-atoken.md` | Solidity aToken contract, LockManager precompile |
| 05 | `specs/05-gigahdx-liquidation.md` | External + treasury liquidation for GIGAHDX collateral |
