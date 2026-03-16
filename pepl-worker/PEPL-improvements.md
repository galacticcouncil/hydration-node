# PEPL Worker — Potential Improvements

## 1. Oracle Update: Skip Unaffected Borrowers

**Current behavior**: When an oracle price update arrives (e.g., DOT/USD changes), the worker re-scans **all 509 borrowers** — including those who hold no DOT at all.

**Improvement**: Cache which assets each borrower uses (collateral + debt) in the `Borrower` struct (e.g., `active_assets: Vec<AssetAddress>`). On oracle update, skip borrowers whose `active_assets` don't overlap with the `updated_assets` list. This avoids the expensive `UserData::new()` call (~6-8 EVM calls per borrower) for irrelevant users.

**Impact**: If DOT/USD updates and only 80 out of 509 borrowers hold DOT, we skip 429 borrowers entirely — saving ~430 × 6 = ~2,500 EVM calls per oracle update.

**Note**: The filtering must happen at the borrower level (skip the entire user), not inside `calculate_liquidation_options` — cross-asset effects within a single user's positions still need full evaluation (ISSUE 9).

---

## 2. Event-Based Borrower Scanning Instead of Full Scan Every Block

**Current behavior**: Every block, the worker calls `UserData::new()` for **all borrowers** to recalculate their health factors. This is ~509 × 6-8 EVM calls = ~3,000-4,000 calls per block.

**Improvement**: Only re-check a borrower if something changed that affects their HF:
- **Oracle price update** on one of their collateral/debt assets (already detectable via mempool)
- **User action event**: Borrow, Repay, Deposit, Withdraw, LiquidationCall (detectable via block events/logs)
- **Interest accrual**: Slow (~hours/days), can use a periodic full re-scan (e.g., every N blocks) as fallback

Borrowers with no relevant events since last check can be skipped entirely.

**Impact**: On a typical block with no oracle updates and no user actions, the scan drops from ~3,000 calls to near zero. Only the periodic fallback scan would process all borrowers.

**Complexity**: Medium — requires tracking per-borrower "last checked" block and matching events to affected borrowers.

---

## 3. Batch EVM Calls in MoneyMarketData::new()

**Current behavior**: `MoneyMarketData::new()` makes ~100 sequential EVM calls to initialize 19 reserves (fetch pool, oracle, reserve data, prices, symbols, asset IDs, emode, existential deposits). In standalone mode over RPC, this takes ~8-10 seconds.

**Improvement**: Use `eth_call` batching or multicall contract to fetch multiple reserve parameters in a single RPC round-trip. For node mode this matters less (~200ms total), but for standalone mode it would cut initialization from ~8-10s to ~1-2s.

**Impact**: Standalone mode becomes usable without `--no-interrupt`. Node mode sees marginal improvement.

---

## 4. Batch UserData::new() Calls

**Current behavior**: `UserData::new()` makes 2 + (2 × active_reserves) sequential EVM calls per borrower:
1. `fetch_user_configuration` — 1 call (bitmap of which reserves are used)
2. Per active reserve: `get_user_collateral_in_base_currency` + `get_user_debt_in_base_currency` — 2 calls each
3. `fetch_user_emode_id` — 1 call

For a user with 3 active reserves: 8 calls. For 509 borrowers: ~3,000-4,000 calls per block.

**Improvement**: Use a multicall contract or batch RPC to fetch all user data in fewer round-trips. Could also fetch `user_configuration` for all borrowers in one batch to determine which reserves are active before fetching detailed data.

**Impact**: Combined with improvement #2, this would dramatically reduce per-block overhead.

---

## 5. Two-Phase Scan: Configuration Check Then Full Fetch

**Current behavior**: `UserData::new()` fetches everything (configuration + all reserve balances + emode) in one call, even if the user's HF is well above 1.0.

**Improvement**: Split into two phases:
1. **Phase 1 (cheap)**: Fetch only `user_configuration` bitmap — 1 EVM call. Combined with cached prices from `MoneyMarketData`, estimate whether HF could be near liquidation threshold.
2. **Phase 2 (expensive)**: Only for borrowers that pass phase 1, fetch full reserve balances and compute exact HF.

**Impact**: Most borrowers (healthy, HF >> 1.0) would be eliminated after 1 call instead of 6-8.

---

## 6. Cache UserData Between Blocks

**Current behavior**: `UserData` is discarded after each block. Every block starts fresh with `UserData::new()` for each borrower.

**Improvement**: Cache `UserData` per borrower and only invalidate when:
- An oracle update changes a relevant asset price
- A user action event is detected for that borrower
- A configurable staleness TTL expires (e.g., re-fetch every 10 blocks as safety net)

Healthy borrowers (HF > 1.1) with no relevant events can reuse cached data for multiple blocks.

**Impact**: Reduces per-block EVM calls from ~3,000-4,000 to only the borrowers that need refreshing.

