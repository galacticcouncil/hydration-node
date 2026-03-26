# GigaHDX - Bugs Found During Testing

### Bug 1: Lock ID Collision After Partial Unlock

**Test:** `lock_id_collides_after_partial_unlock` (FAILS)

`generate_lock_id` uses `positions.len()` which repeats after `unlock` removes positions.
After unlocking position 0, `positions.len()` decreases. The next unstake generates the
same lock_id as an existing position. `set_lock` overwrites the old lock with the new
(smaller) amount, freeing HDX before the cooldown expires.

**Impact:** Users can bypass the 222-day cooldown and transfer HDX early.

### Bug 2: extend_lock Stale Lock Split

**Test:** `lock_split_recalculates_when_gigahdx_balance_increases` (FAILS)

When a user's GIGAHDX balance changes between votes (e.g., stakes more), `extend_lock`
skips recalculation because the total lock amount hasn't changed. The `LockSplit` and
`GigaHdxVotingLock` stay stale, reflecting the old GIGAHDX balance.

**Impact:** EVM-side lock (`GigaHdxVotingLock`) shows less GIGAHDX locked than it should.
User can transfer GIGAHDX that should be locked by voting.

### Bug 3: Reward Silently Lost When PendingRewards Full

**Test:** `reward_lost_when_pending_rewards_full` (PASSES, Open to discussion what is the desired behaviour)

`on_remove_vote` calls `maybe_allocate_and_record` but ignores the error with `let _ =`.
When `PendingRewards` is full (25 entries), `try_push` fails with `MaxVotesReached`.
The vote is removed from `GigaHdxVotes` (line 97) but no reward entry is created.
The user's reward is permanently lost with no error shown.

**Impact:** Users who don't claim rewards regularly lose future rewards silently.

### Bug 4: Conviction::None Gets Same Reward as Locked1x

**Test:** `none_conviction_gets_same_reward_as_locked1x`

The spec says Conviction::None should have 0.1x reward multiplier (much less than Locked1x = 1x).
But the code sets `Conviction::None => 1` - same multiplier as Locked1x.

**Impact:** No incentive to choose any lock period. Users can vote with None conviction (no lock,
tokens free immediately) and get the same rewards as users who lock for 7 days (Locked1x).

## Bug 5: Partial Unstake Across Free/Voted Boundary Fails (MEDIUM)

**Test:** `unstake_should_split_free_and_voted_portions_with_different_cooldowns` in `integration-tests/src/gigahdx_voting.rs`

When a user has 200 GIGAHDX and voted with 100 using Locked6x, unstaking 150 fails entirely. The AAVE aToken contract
blocks withdrawal because 100 GIGAHDX is still conviction-locked. The system doesn't split the unstake into free (100)
and voted (50) portions with separate cooldowns. Expected: 2 positions — 100 HDX with base cooldown, 50 HDX with
conviction cooldown.

### Bug 6: Failed on_idle Conversion Orphans Funds

**Test:** `failed_on_idle_conversion_orphans_funds_when_trading_disabled` (FAILS)

When `on_idle` tries to convert a non-HDX fee and the Omnipool swap fails (e.g., asset trading
disabled by governance), `PendingConversions` is removed but the funds stay in the pot. No future
`on_idle` will retry because the pending entry is gone. The fees are permanently stuck.

**Impact:** Any temporary trading disruption (governance action, liquidity removal) permanently
orphans accumulated fees. They can never be converted or recovered.

### Bug 7: MinConversionAmount Doesn't Account for Asset Decimals

**Test:** `conversion_fails_for_low_decimal_asset_due_to_min_amount_not_accounting_for_decimals` (FAILS)

`MinConversionAmount = 1_000_000_000_000` assumes 12 decimals (like HDX). For assets with fewer
decimals (e.g., 6), even a meaningful trade fee will be below this threshold. The conversion
always fails with `AmountTooLow` and fees are orphaned.

**Impact:** All non-HDX fees from low-decimal assets are permanently lost. Reproduces the
`ConversionFailed` events seen on lark testnet (blocks 25209-25821).