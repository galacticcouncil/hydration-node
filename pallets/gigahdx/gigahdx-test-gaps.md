# GigaHDX - Test Gap Analysis

Cross-referenced against the original spec (GitHub issue #1362, gigahdx.spec,
and detailed specs 01-04). Each gap includes the spec reference that justifies it.

**Existing test coverage:**

- pallet-gigahdx: 21 unit tests (stake, unstake, unlock, exchange_rate)
- pallet-gigahdx-voting: 34 unit tests (adapter, hooks, rewards, claim, gigahdx_hooks)
- pallet-fee-processor: 17 unit tests (process_fee, convert)
- integration-tests: 26 tests (gigahdx, gigahdx_voting, fee_processor, lock_manager)

---

## HIGH RISK - Partial Unstake with Voted GIGAHDX

The current `giga_unstake` applies a single cooldown to the entire unstake amount.
It does not distinguish between free (unvoted) and locked (voted) GIGAHDX.

**Note:** The written spec describes `actual_cooldown = max(base_cooldown, voting_lock)` as a
single value per unstake call. The CTO's verbal requirement is that unstaking should split into
multiple positions (free portion at base cooldown, voted portion at conviction cooldown).
These gaps test the CTO's desired behavior, which requires a code change.

### Gap 1: Partial unstake when only some GIGAHDX is voted

**Scenario:**

- Stake 200 GIGAHDX
- Vote 100 GIGAHDX with Locked6x conviction (224 days lock)
- Unstake 150 GIGAHDX

**Expected:** Two unlock positions:

- 100 GIGAHDX (free portion) -> 222-day cooldown (base)
- 50 GIGAHDX (voted portion) -> max(222, 224) = 224-day cooldown

**Current behavior:** All 150 locked for 224 days (the max), which penalizes the free 100.

**No test exists for this scenario.**

**Spec reference:** Issue #1362 states "Must support partial lock removal on partial unstakes".
gigahdx.spec section 7.1.2 says "Allows partial unstake" and "User can have
multiple concurrent unlock positions". The CTO explicitly flagged this as incorrect behavior:
the current code assumes all GIGAHDX is either locked or unlocked.

### Gap 2: Unstake exactly the free amount

**Scenario:**

- 200 GIGAHDX, 100 voted
- Unstake exactly 100 (the free portion)

**Expected:** Single unlock position with base 222-day cooldown only.

**Not tested.**

**Spec reference:** Same as Gap 1.

### Gap 3: Unstake more than free but less than total

**Scenario:**

- 200 GIGAHDX, 100 voted
- Unstake 180

**Expected:** Split: 100 free (222-day cooldown) + 80 from voted (conviction cooldown).

**Not tested.**

**Spec reference:** Same as Gap 1.

### Gap 4: Unstake all when partially voted

**Scenario:**

- 200 GIGAHDX, 100 voted
- Unstake all 200

**Expected:** Split: 100 free (222 days) + 100 voted (conviction cooldown).

**Not tested.**

**Spec reference:** Same as Gap 1.

---

## HIGH RISK - Reward Calculation Edge Cases

### Gap 5: Rewards only for GIGAHDX portion when voting with both GIGAHDX + HDX (end-to-end)

**Scenario:**

- User has 100 HDX + 100 GIGAHDX
- Votes 200 (uses all GIGAHDX + all HDX)
- Referendum finishes, removes vote, claims rewards

**Expected:** Reward calculated only on the 100 GIGAHDX portion, NOT the 100 HDX.

**Status:** Integration test `combined_voting_power` checks the split is recorded correctly,
but **never follows through to claim_rewards** to verify the actual reward amount excludes HDX.

**Spec reference:** Issue #1362 explicitly states "Only votes made with GIGAHDX are eligible
for rewards". Spec 03 section 10.2 confirms "Only GIGAHDX portion of votes is tracked".

### Gap 6: Multiple referenda with different convictions, claim all at once

**Scenario:**

- Vote on referendum A with Locked1x (1x multiplier)
- Vote on referendum B with Locked6x (6x multiplier)
- Both finish, remove both votes
- Call `claim_rewards` once

**Expected:** Rewards calculated independently per referendum with correct conviction weights.

**Status:** `conviction_weighted_rewards` test only covers ONE referendum.
No end-to-end test with multiple referenda.

**Spec reference:** Spec 03 section 9.1 describes claiming all pending rewards at once.
Section 10.4 shows conviction-weighted calculation per referendum. The system must handle
multiple pending entries from different referenda with different conviction levels.

### Gap 7: Reward pot depletion across multiple referenda

**Scenario:**

- Referendum A finishes -> first voter removes vote -> takes 10% of GigaReward pot (10,000 HDX -> 1,000)
- Referendum B finishes -> first voter removes vote -> takes 10% of REMAINING pot (9,000 HDX -> 900)

**Expected:** Each referendum gets progressively less reward (by design).
Need to verify the math is correct and no overflow/underflow.

**Not tested.**

**Spec reference:** Spec 03 section 8.1 describes lazy allocation: "Get reward percentage,
Take that percentage from the GigaReward Pot". Each referendum takes a percentage of the
CURRENT pot balance, meaning sequential referenda get progressively less.

---

## MEDIUM RISK - Unstake Flow Edge Cases

### Gap 8: Full unstake flow with force-removed votes and reward recording

**Scenario:**

- Stake -> vote with conviction -> referendum ends -> DON'T manually remove vote -> giga_unstake

**Expected:**

1. `on_unstake` force-removes the finished vote
2. Reward is recorded in PendingRewards
3. Cooldown is max(222, remaining_conviction_lock)
4. User can later call `claim_rewards`

**Status:** Unit test `on_unstake_force_removes_finished_votes` tests the hook in isolation.
**No integration test for the full flow** (stake -> vote -> referendum ends -> unstake -> verify rewards + cooldown).

**Spec reference:** Spec 03 section 10.2.2 describes the full flow: "Force remove all votes
from finished referenda, Process rewards, Calculate dynamic cooldown, Proceed with unstake".
This is a core spec requirement.

### Gap 9: Dynamic cooldown with multiple votes at different convictions

**Scenario:**

- Vote on referendum A with Locked1x (7 days lock)
- Vote on referendum B with Locked6x (224 days lock)
- Both finish, unstake

**Expected:** Cooldown = max(222, 224) = 224 days (uses the highest remaining lock).

**Status:** Unit test `additional_unstake_lock_returns_max_remaining` tests the calculation.
**No integration test for the complete flow with actual conviction-voting.**

**Spec reference:** Spec 03 section 10.2.3: "remaining_voting_lock = max(lock_expires_at -
current_block, 0) for all removed votes. actual_cooldown = max(base_cooldown_222_days,
remaining_voting_lock)". The max is across ALL votes.

### Gap 10: Interleaved stake/unstake/vote operations

**Scenario:**

- Stake 100 -> unstake 50 -> stake 100 more -> vote -> unstake again

**Expected:** System handles multiple overlapping positions and votes correctly.

**Not tested.**

**Spec reference:** Not explicitly in spec, but the spec allows partial unstake (section 7.1.2)
and multiple concurrent unlock positions. The combination of these with voting is an important
integration test.

---

## MEDIUM RISK - Exchange Rate Edge Cases

### Gap 11: Exchange rate manipulation via direct HDX transfer to gigapot (first depositor attack)

**Scenario:**

- Stake a tiny amount of HDX (e.g., minimum)
- Someone sends a large amount of HDX directly to gigapot address (bypassing fee processor)
- Exchange rate inflates drastically
- Subsequent small stakes could round down to 0 stHDX and fail with `ZeroAmount`

This is the classic "first depositor attack" from DeFi vaults. For normal-sized stakes the
value is preserved (fewer GIGAHDX but each worth more), but small stakes can be bricked
by rounding.

**Not tested.**

**Spec reference:** gigahdx.spec section 6.1: "Exchange rate = Total HDX in gigapot / Total
stHDX supply". The rate is derived from gigapot balance. Anyone who can transfer HDX to
gigapot can influence the rate.

### Gap 12: Multiple sequential reward claims affecting exchange rate

**Scenario:**

- User A claims rewards -> HDX goes to gigapot -> exchange rate changes
- User B claims rewards immediately after -> gets different amount of GIGAHDX for same HDX reward

**Not tested.**

**Spec reference:** Spec 02 section 10 describes `stake_rewards` which uses pre-reward rate:
"Exclude the reward HDX from total to get the pre-reward rate". This is meant to prevent
rate manipulation from reward claims, but is untested with sequential claims.

### Gap 13: Unstake at extreme exchange rates

**Scenario:**

- Exchange rate is very high (e.g., 1000:1)
- Unstake 1 GIGAHDX -> should return 1000 HDX
- Does the gigapot have enough? Rounding issues?

**Not tested.** (Highest tested rate in existing tests is 2:1)

**Spec reference:** gigahdx.spec section 6.1 defines the exchange rate formula. The math
should work at any rate, but extreme values could trigger overflow or rounding issues in
`multiply_by_rational_with_rounding`.

### Gap 14: Exchange rate with zero stHDX supply edge case

**Scenario:**

- All stakers unstake (total_stHDX = 0, but gigapot may still have dust HDX from rounding)
- New staker comes in - exchange rate should reset to 1:1
- But dust HDX in gigapot is "donated" to the next staker

**Not tested.**

**Spec reference:** Spec 02 section 5.4: "if total_st_hdx.is_zero() { Some(hdx_amount) }"
resets to 1:1. But the spec doesn't address what happens to accumulated fees in gigapot
when all stakers leave.

---

## MEDIUM RISK - Voting Adapter Edge Cases

### Gap 15: Vote, transfer free GIGAHDX, then vote again

**Scenario:**

- 1000 GIGAHDX, vote 500 on referendum A
- Transfer 300 free GIGAHDX to someone else (now have 700 GIGAHDX, 500 locked)
- Vote on referendum B with 600
- Adapter should cap GIGAHDX portion at 700 (current balance)

**Not tested.**

**Spec reference:** Spec 03 section 6.3 describes the adapter's `set_lock` with GIGAHDX-first
priority based on current balance. If balance changes between votes, the adapter must
recalculate correctly.

### Gap 16: Receive GIGAHDX while having active votes

**Scenario:**

- 100 GIGAHDX, vote all 100 (all locked)
- Someone transfers 200 GIGAHDX to you (now 300 total, 100 locked)
- Can you transfer the free 200? Does the system report correct free balance?

**Not tested.**

**Spec reference:** Spec 04 section 5 describes the balance model: "free = balanceOf - locked".
Receiving new GIGAHDX should increase the free balance without affecting existing locks.

### Gap 17: Lock recalculation when GIGAHDX balance changes between votes

**Scenario:**

- 500 GIGAHDX + 500 HDX, vote 800 on ref A (500 GIGAHDX + 300 HDX locked)
- Stake more HDX -> get 200 more GIGAHDX (now 700 GIGAHDX)
- Vote on ref B with 800 -> should re-split: 700 GIGAHDX + 100 HDX

**Not tested.**

**Spec reference:** Spec 03 section 6.3 set_lock always recalculates from current balance:
"gigahdx_balance = Currency::free_balance(GigaHdxAssetId, who)". So voting again after
staking more should correctly use the new higher GIGAHDX balance.

---

## MEDIUM RISK - Liquidation

### Gap 18: Liquidation with mixed vote states (ongoing + finished referenda)

**Scenario:**

- Vote on referendum A (ongoing) and referendum B (finished/approved)
- Position gets liquidated via `prepare_for_liquidation`

**Expected:**

- Referendum B (finished): rewards recorded in PendingRewards
- Referendum A (ongoing): no rewards, vote just cleared
- All locks cleared

**Not tested in integration tests.** Only mock-based unit test exists.

**Spec reference:** Spec 03 section 10.1 explicitly describes this: "Force-remove ALL votes
from conviction-voting (including ongoing referenda). Each removal triggers on_remove_vote
hook -> records rewards for finished referenda." Ongoing referenda votes are invalidated
with no rewards.

### Gap 19: Liquidation followed by re-stake and re-vote

**Scenario:**

- Liquidation clears all votes and locks
- User stakes again, receives new GIGAHDX
- User votes again on new referendum

**Expected:** Clean slate, everything works as if fresh start.

**Not tested.**

**Spec reference:** Spec 03 section 10.1: after liquidation, all votes are force-removed and
locks cleared. The spec implies the user should be able to start fresh.

---

## LOWER RISK - Fee Processor Edge Cases

### Gap 20: Fee distribution rounding with very small fees

**Scenario:**

- Trade generates 1 unit fee in HDX
- 70% of 1 = 0 (rounds down), 20% of 1 = 0, 10% of 1 = 0
- All receivers get 0, fee is lost?

**Not tested.** (All existing fee tests use amounts >= 500 * ONE)

**Spec reference:** Spec 01 section 5.3 uses `percentage.mul_floor(total)` which rounds down.
For tiny amounts, all receivers could get 0.

---

## LOWER RISK - Lock Manager Precompile

### Gap 21: Precompile with wrong token address

**Scenario:**

- Call `getLockedBalance` with a token address that isn't GIGAHDX

**Expected:** Returns 0 (no lock for that token). But current implementation may return the
GIGAHDX lock regardless of token parameter.

**Not tested.** (All precompile tests use the same dummy token address)

**Spec reference:** Spec 04 section 3.2 shows the precompile takes a `token` parameter.
The current implementation ignores the token parameter and always reads GigaHdxVotingLock.
This means any token address returns the same lock amount.

---

## LOWER RISK - Conviction Voting Combinator

### Gap 22: VotingHooks combinator with both old staking and GigaHDX hooks

**Scenario:**

- User has both old HDX staking position AND GIGAHDX
- Votes -> both StakingConvictionVoting and GigaHdxVotingHooks should fire
- One hook fails -> does it roll back the other?

**Status:** Integration test `staking_hooks_still_work` verifies both fire, but doesn't test
failure scenarios where one hook succeeds and the other fails.

**Spec reference:** gigahdx.spec section 9.2 shows VotingHooks is configured as a tuple:
"(StakingConvictionVoting, GigaHdxVotingHooks)". Both hooks fire on every vote.

---

## NOT IN SCOPE (deferred per spec)

The following features are mentioned in the spec but explicitly deferred:

- **Migration from HDX staking** (gigahdx.spec section 12): "Workstream 6 - After 2 is working"
- **Treasury-only liquidation mechanism** (gigahdx.spec section 11.3): "Workstream 5 - After 2, 3, 4 are working"
- **Immortal loans, reduced borrow APY, self-repaying loans** (section 11.4): "Out of scope for initial launch"

No tests should be written for deferred features.

---

## Priority Order for Test Writing

| Priority | Gaps           | Description                                                   | Spec Section     |
|----------|----------------|---------------------------------------------------------------|------------------|
| 1        | 1, 2, 3, 4     | Partial unstake with voted GIGAHDX (CTO's bug)                | 7.1.2, CTO notes |
| 2        | 5              | Reward excludes HDX portion (end-to-end)                      | Issue #1362      |
| 3        | 8, 9           | Full unstake flow with force-removed votes + dynamic cooldown | Spec 03 s10.2    |
| 4        | 7              | Reward pot depletion across referenda                         | Spec 03 s8.1     |
| 5        | 18             | Liquidation with mixed vote states                            | Spec 03 s10.1    |
| 6        | 11, 12, 13, 14 | Exchange rate edge cases                                      | Spec 02 s6.1     |
| 7        | 15, 16, 17     | Voting adapter with balance changes                           | Spec 03 s6.3     |
| 8        | 6, 10          | Multiple referenda rewards, interleaved operations            | Spec 03 s9.1     |
| 9        | 20             | Fee distribution rounding                                     | Spec 01 s5.3     |
| 10       | 19, 21, 22     | Liquidation re-entry, precompile, combinator                  | Spec 03/04       |
