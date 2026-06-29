# Hydration Protocol - Attack Vector Reference

This document distills findings from 11 audit reports and bug bounty disclosures (2022-2025) covering Hydration's Omnipool, Stableswap, XYK, EMA Oracle, EVM precompiles, ERC20 mapping, AAVE deployment, liquidation pallet, Hollar Stability Module, and drifting-peg stablepools. It is organized by attack vector category to aid security review.

---

## 1. Pool Draining / Fund Theft

### Stableswap buy() with asset_in == asset_out (Critical)
- **Source:** Code4rena H-01
- **Impact:** Drains entire pool liquidity for 1 wei cost
- **Mechanism:** `buy()` doesn't validate `asset_in != asset_out`. When equal, the math returns `amount_in = 0` while the pool transfers `amount_out` to the attacker. One call drains the full pool balance.
- **Pattern:** Missing input validation on trade pair identity. Also flagged in RV Omnipool (#7: "Assets allowed to be equal when selling").

### Hub asset withdrawal via refund_refused_asset() (High)
- **Source:** RV Omnipool #5
- **Impact:** LRNA (hub asset) can be withdrawn from pool account through `refund_refused_asset()` call
- **Pattern:** Privileged function allows unintended asset extraction

### Cross-Pool LP Share Theft via Missing AssetPair Validation (Critical)
- **Source:** Immunefi bug bounty, August 2025
- **Impact:** Up to ~$200K extractable. Attacker could unlock higher-value LP shares from a different pool using a deposit from a lower-value pool.
- **Mechanism:** The XYK liquidity mining withdrawal path did not validate that the user-supplied `AssetPair` matched the `amm_pool_id` stored in the deposit. Since `AssetPair` derives `amm_pool_id`, an attacker deposits low-value LP shares (e.g., DOT/MYTH), then calls withdrawal with the `AssetPair` of a higher-value pool (e.g., DOT/EWT). The system unlocks and returns higher-value LP shares from the wrong pool.
- **Pattern:** Confused deputy — user-controlled parameter (`AssetPair`) derives a security-critical identifier (`amm_pool_id`) without cross-validation against the stored authoritative value.

### TransferFrom treated as view function (High)
- **Source:** Pashov H-01
- **Impact:** `transferFrom` was marked as `FunctionModifier::View` instead of `NonPayable` in EVM precompile, meaning state-changing ERC20 transfers could fail silently or be called without proper gas/state accounting
- **Pattern:** Incorrect EVM function modifier classification in Substrate precompiles

---

## 2. Oracle Manipulation / Staleness

### Oracle not updated by direct transfers (Medium)
- **Source:** RV EMA Oracle A1, Code4rena M-01
- **Impact:** Sending tokens directly to pool account (via `Currency::transfer`) bypasses trade hooks, so oracle price remains stale while actual pool reserves change
- **Pattern:** Any reserve change that doesn't go through the pallet's extrinsics skips `on_trade`/`on_liquidity_changed` hooks

### Stale oracle after token re-addition (Medium)
- **Source:** Code4rena M-07
- **Impact:** Removing then re-adding a token preserves old oracle data. If price changed, oracle returns wrong price, can block `add_liquidity` (PriceDifferenceTooHigh) or feed wrong prices to consumers
- **Pattern:** Oracle storage not cleaned up on `remove_token()`

### Missing hook call in remove_token() (Medium)
- **Source:** Code4rena M-09
- **Impact:** `remove_token()` changes pool liquidity but doesn't call `on_liquidity_changed`, leaving oracle outdated and circuit breaker limits unenforced
- **Pattern:** Missing hook invocation on state-changing operations

### EMA of reciprocal price diverges (Medium)
- **Source:** RV EMA Oracle A2
- **Impact:** `EMA(1/price) != 1/EMA(price)` — mathematical property of EMA means the oracle's reciprocal price feeds can diverge from true reciprocal, creating arbitrage opportunities
- **Pattern:** Mathematical limitation of EMA applied to reciprocal values

### Missing staleness monitoring (Informational)
- **Source:** OAK #10
- **Impact:** No check if oracle data is stale (e.g., Bifrost feed stops updating). Pallet silently uses outdated prices.
- **Pattern:** No freshness/heartbeat validation on external oracle feeds

### Arbitrage bot operates on outdated values (Low)
- **Source:** Cantina HSM
- **Impact:** HSM's arbitrage bot may execute trades based on stale pool state
- **Pattern:** Time-of-check-time-of-use on pool price reads

---

## 3. Price Manipulation / Sandwich Attacks

### TVL manipulation via stablecoin price (High)
- **Source:** RV Omnipool #2
- **Impact:** TVL is calculated using the stablecoin's spot price in the Omnipool. Manipulating the stablecoin price lets attacker disable `add_liquidity` by making `TotalTVL >= TVLCap` with relatively small capital
- **Pattern:** Using manipulatable spot price for critical protocol limits

### Amplification factor sandwich attacks (Medium)
- **Source:** RV Stableswap A10, OAK #6
- **Impact:** Consecutive rapid modifications of the amplification factor allow sandwich attacks. Unlike Curve's MIN_RAMP_TIME (86400s), no minimum timeframe between changes is enforced
- **Pattern:** Missing rate-limiting on pool parameter changes accessible by privileged origin

### Liquidation avoidable by pool manipulation (Low)
- **Source:** Cantina Liquidations
- **Impact:** Attacker can manipulate swap pool prices to make liquidation transactions revert (slippage check fails), delaying liquidation
- **Pattern:** Liquidation depends on market-manipulatable swap execution

### Max buy price check compares different denominations (High)
- **Source:** Cantina HSM
- **Impact:** HSM compared buy price in hollar denomination against a limit in collateral denomination — unit mismatch allows buying at wrong price
- **Pattern:** Currency denomination mismatch in price comparison

---

## 4. Slippage / MEV / Withdrawal Fee Attacks

### No slippage check in remove_liquidity (Medium)
- **Source:** Code4rena M-03, RV Stableswap A6, OAK #5
- **Impact:** `remove_liquidity` has no user-specified minimum output. Frontrunners can sandwich the transaction for ~1-2% extraction. Also applies to `add_liquidity` (no `min_shares_out`).
- **Pattern:** Missing slippage protection on liquidity operations

### 100% withdrawal fee via safe_withdrawal bypass (Medium)
- **Source:** Code4rena M-10
- **Impact:** If trading is disabled (tradable = ADD_LIQUIDITY | REMOVE_LIQUIDITY) while price is manipulated, `safe_withdrawal` flag is true, `ensure_price` is skipped, and withdrawal_fee can be 100%. User loses entire withdrawal to fees.
- **Pattern:** `set_asset_tradable_state()` doesn't verify price is within bounds before enabling "safe" withdrawal mode

### No safe_withdrawal in withdraw_protocol_liquidity (Medium)
- **Source:** Code4rena M-05
- **Impact:** Protocol liquidity withdrawal susceptible to MEV/frontrunning since it lacks safe withdrawal protections
- **Pattern:** Inconsistent security checks between user and protocol withdrawal paths

### Hardcoded slippage in liquidations (Low)
- **Source:** Cantina Liquidations
- **Impact:** `min_amount_out=1` hardcoded in liquidation swaps allows sandwich attacks extracting value from liquidation proceeds
- **Pattern:** Hardcoded slippage tolerance instead of dynamic/oracle-based minimum

---

## 5. Denial of Service (DoS)

### Complete liquidity removal permanently disables pool (Medium)
- **Source:** Code4rena M-04, M-06
- **Impact:** Removing all liquidity from an Omnipool asset sets `reserve=0`. Subsequent `add_liquidity` divides by zero (overflow), permanently DoS'ing the pool. If someone sends 1 token directly, new LPs get 0 shares (permanent fund lock).
- **Pattern:** Division by zero when pool reserves reach zero; no minimum reserve enforcement

### Storage bloat via dust positions (Medium)
- **Source:** Code4rena M-08
- **Impact:** Add `MinimumPoolLiquidity` then immediately withdraw all but 1 wei. Creates permanent storage entries at 0.0001% of intended cost. Repeating this bloats chain storage.
- **Pattern:** MinimumPoolLiquidity checked on deposit but not enforced on remaining balance after withdrawal

### LP exit blocked by MinPoolLiquidity (Major)
- **Source:** OAK #2
- **Impact:** If one LP withdraws most liquidity leaving pool just above MinPoolLiquidity, remaining LP can't fully exit (amount > remaining but < total required for pool destruction)
- **Pattern:** Pool minimum liquidity threshold traps remaining LPs

### DoS via non-whitelisted oracle pairs (Major)
- **Source:** OAK #1
- **Impact:** Bifrost oracle can push data for non-whitelisted pairs, consuming MaxUniqueEntries capacity and blocking legitimate updates
- **Pattern:** No filtering on external oracle data ingestion

### Unbounded storage iteration in EVM migration (Medium)
- **Source:** Pashov M-01
- **Impact:** Iterating all EVM address mappings in a runtime upgrade can exceed block weight limits
- **Pattern:** Unbounded iteration in on-chain migrations

### Unbounded memory growth via Box::leak (Medium)
- **Source:** Pashov M-03
- **Impact:** `Box::leak` in error handling paths causes monotonic memory growth, never freed
- **Pattern:** Memory leak in error paths of long-running runtime

### Collateral removal DoS via minimal transfer (Low)
- **Source:** Cantina HSM
- **Impact:** Sending a minimal token amount to HSM can prevent collateral removal
- **Pattern:** Existential deposit / dust amount interfering with pool accounting

### Hardcoded gas limit breaks liquidations (Low)
- **Source:** Cantina Liquidations
- **Impact:** 1M gas limit insufficient for multi-asset liquidations. Fixed to 4M.
- **Pattern:** Hardcoded resource limits instead of dynamic calculation

---

## 6. Rounding / Arithmetic Errors

### Rounding error causes pool losses (High)
- **Source:** RV Omnipool #3
- **Impact:** Integer rounding in remove_liquidity can cause more assets to leave the pool than mathematically correct, creating extractable value
- **Pattern:** Rounding direction should always favor the pool (round against the user)

### Too much hub asset burned on remove_liquidity (High)
- **Source:** RV Omnipool #4
- **Impact:** Excess LRNA burned when removing liquidity, causing protocol losses
- **Pattern:** Incorrect rounding in hub asset burn calculation

### Imbalance calculation deviation (Medium)
- **Source:** RV Omnipool #11
- **Impact:** `add_liquidity` doesn't fully follow the math model when computing LRNA imbalance, causing drift from specification
- **Pattern:** Implementation diverges from formal mathematical specification

### Incorrect sell limit when buying with LRNA (Medium)
- **Source:** RV Omnipool #8
- **Impact:** Wrong limit checking when buying asset_out using LRNA, allowing trades that should be rejected
- **Pattern:** Boundary condition error in limit validation

---

## 7. Pool Invariant Violations

### Malicious LP breaks MinPoolLiquidity invariant (Medium)
- **Source:** Code4rena M-02
- **Impact:** LP transfers share tokens to bring position below MinPoolLiquidity, then withdraws. Pool enters state where invariant is broken, making it susceptible to manipulation
- **Pattern:** Share token transferability allows circumventing deposit minimums

### Asset balance below existential deposit (Medium)
- **Source:** RV Stableswap A4
- **Impact:** Pool asset balance can drop below existential deposit, potentially causing account reaping and total loss of that asset's reserves
- **Pattern:** Missing existential deposit checks on pool accounts

### Duplicate assets in pool operations
- **Source:** RV Stableswap A2, A3
- **Impact:** `create_pool` accepts empty/single asset lists; `add_liquidity` accepts duplicate assets, breaking pool invariants and math assumptions
- **Pattern:** Missing input validation on asset lists

### Pool with identical configuration (Informational)
- **Source:** RV Stableswap A8
- **Impact:** Multiple pools with identical asset sets and parameters can be created, fragmenting liquidity
- **Pattern:** No uniqueness check on pool configuration

---

## 8. Governance / Privileged Role Risks

### Liquidity inflation/deflation by registry owner (Major)
- **Source:** OAK #3
- **Impact:** Registry owner can change asset decimals after pool creation. `normalize_value()` is used in math but not in `do_add_liquidity`, so changing decimals corrupts all pool liquidity calculations. Can inflate/deflate liquidity by 10^K.
- **Pattern:** Mutable asset metadata (decimals) affecting live pool math without normalization

### HSM violates collateralization invariant (Medium)
- **Source:** Cantina HSM
- **Impact:** HSM operations can leave the Hollar stablecoin under-collateralized
- **Pattern:** Stability module operations not checked against collateralization ratio

### Peg selection risks (Medium)
- **Source:** Cantina HSM
- **Impact:** Static vs dynamic peg choice has implications for permanent peg drift
- **Pattern:** Governance parameter selection with long-term systemic implications

### Pool fee unbounded (Informational)
- **Source:** RV Stableswap B2, OAK #9
- **Impact:** Pool fee can be set to 100%, freezing all trading
- **Pattern:** No upper bound on fee parameter

---

## 9. EVM / Substrate Bridge Issues

### ERC20 return value not verified (Medium)
- **Source:** Pashov M-02
- **Impact:** `handle_result()` doesn't verify ERC20 function return values, meaning failed ERC20 operations could be treated as successful
- **Pattern:** Non-standard ERC20 return value handling

### EVM/Substrate address mapping truncation (Informational)
- **Source:** SRLabs
- **Impact:** Converting between 32-byte Substrate and 20-byte EVM addresses truncates entropy, theoretically enabling collisions and fund loss
- **Pattern:** Address space reduction in cross-VM mapping

### Unsigned dispatch_permit spam (Medium)
- **Source:** SRLabs
- **Impact:** `dispatch_permit` as unsigned extrinsic allows feeless transaction spam, potential DoS of transaction pool
- **Pattern:** Unsigned extrinsics without rate limiting or economic cost

### EVM exit status misclassification (Low)
- **Source:** Pashov L-03
- **Impact:** `ExitSucceed::Suicided` treated as error instead of success
- **Pattern:** Incorrect EVM exit code handling

---

## 10. Substrate-Specific Patterns

### Native currency special-casing (Medium)
- **Source:** RV Omnipool #6
- **Impact:** Incorrect asset state update when HDX (native currency) is traded, because native currency uses `NativeCurrency` storage while all others use `MultiCurrency`
- **Pattern:** Dual accounting systems for native vs non-native tokens

### Missing `require_transactional` macro (Low)
- **Source:** Code4rena QA
- **Impact:** Storage mutations without transactional wrapper could leave inconsistent state on partial failure
- **Pattern:** Substrate storage operations outside transactional scope

### Overflow in ORML token balances (Medium)
- **Source:** RV Omnipool #10
- **Impact:** Missing overflow checking for ORML token account balances
- **Pattern:** Assumption that upstream library handles overflow

### Incomplete pool destruction (Minor)
- **Source:** OAK #7
- **Impact:** Pool removal doesn't clean up peg data from storage, leaving stale entries
- **Pattern:** Incomplete cleanup in pool lifecycle operations

---

## Recurring Themes (Priority Checklist for Auditors)

1. **Direct transfers bypass hooks** — Any time assets reach a pool account via `Currency::transfer` instead of pallet extrinsics, oracles, circuit breakers, and TVL accounting are bypassed. Check ALL paths money enters/leaves pool accounts.

2. **asset_in == asset_out** — Always validate that trade/swap pair assets are distinct. This has led to critical pool drains.

3. **Division by zero on empty pools** — When all liquidity is removed, subsequent operations that divide by reserves or shares will panic. Enforce minimum reserves.

4. **Slippage protection gaps** — Every user-facing liquidity/trade operation needs a min/max output parameter. Check `remove_liquidity`, `add_liquidity`, `withdraw_protocol_liquidity`.

5. **Oracle consistency** — Every operation that changes pool reserves MUST call oracle hooks (`on_trade`, `on_liquidity_changed`). Missing calls leave oracle stale and circuit breaker limits unenforced.

6. **Rounding direction** — Integer math must always round in favor of the pool (against the user) to prevent extractable rounding profit.

7. **Amplification factor rate-limiting** — Stableswap amplification changes need minimum time between updates to prevent sandwich attacks.

8. **Existential deposit awareness** — Pool accounts must stay above existential deposit for all assets. Below-ED balances get reaped, causing total loss.

9. **EVM/Substrate boundary** — Function modifiers, return value handling, address truncation, and unsigned extrinsic costs all need special attention at the EVM-Substrate bridge.

10. **Privileged role impact** — Even trusted origins (governance, registry owner) can cause damage. Asset decimal changes, tradability state changes, and fee updates can corrupt pools. Add safety checks even on privileged operations.

11. **`saturating_sub`/`saturating_*` hiding errors** — Saturating math silently returns 0 on underflow instead of failing. This has led to critical exploits where insufficient balances were silently accepted. Default to `checked_*` math; only use saturating with explicit justification.

12. **Multi-block oracle attacks via transaction ordering** — DCA + batch_call can guarantee transaction ordering across blocks (DCA first, fill rest of block). Combined with per-block price limits, attackers can ratchet oracle prices over multiple blocks while keeping net exposure near zero. Rate-limit DCA trade sizes and monitor multi-block price drift.

13. **Scope creep in generic layers** — A "small fix" in a shared transfer function can silently break invariants in all dependent pallets (Stableswap, Router, etc.). Changes to generic/shared code paths need blast-radius analysis and mandatory security review.

14. **Confused deputy via user-supplied resource selectors** — Any function accepting a user-supplied identifier (AssetPair, pool_id, account) to look up or operate on a stored resource MUST cross-validate against the resource's stored association. Trusting user input to select which resource to operate on has led to cross-pool LP share theft.

---

## Audit Sources

| Date | Auditor | Scope | Findings |
|------|---------|-------|----------|
| 2022-09 | Runtime Verification | Omnipool (formal) | 11 notable + 10 informative |
| 2023-06 | Runtime Verification | EMA Oracle | 2 notable + 3 informative |
| 2023-07 | Runtime Verification | Stableswap | 10 notable + 6 informative |
| 2024-04 | Code4rena | Omnipool + Stableswap + EMA Oracle | 1 high + 10 medium + QA |
| 2024-06 | SRLabs | EVM Precompiles | 1 medium + informational |
| 2024-10 | Pashov | ERC20 Mapping | 1 high + 3 medium + 5 low |
| 2025-01 | Cantina | AAVE v3 Deployment | Informational only |
| 2025-04 | Cantina | Liquidation Pallet | 1 medium + 4 low + informational |
| 2025-05 | OAK Security | Stablepools Drifting Peg | 3 major + 7 minor + 7 informational |
| 2025-06 | Cantina | Hollar Stability Module | 1 high + 2 medium + 5 low + informational |

*BlockScience (2022-03) delivered an economic specification report for the Omnipool, not a security audit.*

---

## Bug Bounty Post Mortems

### Risk-Free Oracle Manipulation via DCA + Batch Call (High — $25k bounty)
- **Source:** Immunefi bug bounty, reported July 2024
- **Post mortem:** [jakpan.hashnode.dev](https://jakpan.hashnode.dev/hydration-oracle-manipulation-post-mortem)
- **Impact:** Multi-block oracle manipulation enabling inflated liquidity addition and subsequent value extraction. Estimated potential impact in the tens of millions if all mitigations could be circumvented.
- **Mechanism:**
  1. **DCA pallet** allows scheduling trades that execute at the start of a block (before any other transactions)
  2. **`batch_call`** (utility pallet) can fill remaining block weight, preventing other actors from transacting
  3. **Attack cycle per block:** Use `batch_call` to buy token X (moving price up 50% — the max per-block limit), then DCA executes first in the next block selling X back (moving price down 50%). Oracle sees the manipulated price during block initialization, but the attacker's net position is unchanged.
  4. **Repeat ~14 blocks** to move oracle price to ~7x. Then stabilize for volatility fees to decay, add liquidity at inflated price, reverse manipulation, stabilize again, withdraw at real price for profit.
- **Why not Critical:** Attack required ~$50M in diverse tokens, incurred ~30% in dynamic fees, was exposed to arbitrage bots during each manipulation block (~16% of liquidity open to in-block arbitrage), and was bounded by per-block limits (5% add/remove liquidity caps, 50% max price change, DCA limits).
- **Existing mitigations that limited exploitability:**
  - Per-block max price change limit (50%)
  - DCA price change limit per period
  - Randomized oracle execution order within blocks
  - Dynamic add/remove liquidity fees (up to 1%, then disabled until price stabilizes)
  - Dynamic volatility fees (up to 5% per manipulation block)
  - Per-block liquidity add/remove caps (5% of pool)
  - Token liquidity caps
  - Technical committee ability to pause Omnipool and XCM
- **Fix:** Additional limit on DCA trade sizes introduced, making the attack vector impossible.
- **Pattern:** Multi-block oracle manipulation using transaction ordering guarantees (DCA executes first) combined with block-filling to exclude arbitrageurs. Similar to TWAP oracle attacks via validator control on Ethereum.

### aToken Liquidity Addition Exploit in Stableswap (Critical — $500k bounty)
- **Source:** Immunefi bug bounty, reported June 18, 2025
- **Post mortem:** [jakpan.hashnode.dev](https://jakpan.hashnode.dev/exploiting-atoken-liquidity-addition-in-stableswap-post-mortem)
- **Impact:** Up to $22M at risk. Attacker could mint arbitrary stablepool shares without backing assets. Only the GDOT pool (stablepool with aTokens) was affected.
- **Mechanism:**
  1. AAVE aTokens are rebasing ERC20 tokens. Converting between native tokens and aTokens caused rounding dust that couldn't be cleaned up (no existential deposit in ERC20).
  2. A fix was introduced in the `Currencies` pallet transfer function: when remaining aToken balance after transfer < ED, perform an "AAVE withdraw all" to clean up dust.
  3. The fix used `saturating_sub`: `let diff = atoken_balance.saturating_sub(amount)` — if `amount > balance`, this silently returns 0 instead of failing, triggering the "withdraw all" path.
  4. This meant transfers of aTokens **never failed for insufficient balance** — they just withdrew whatever was available.
  5. `Stableswap::add_liquidity_shares` mints user-specified shares then transfers the corresponding asset amount. With the broken transfer, a user could specify more shares than their balance supported — shares were minted, but the transfer silently succeeded with less than the required amount.
- **Root cause:** `saturating_sub` in balance check silently swallowed underflow. A "small" dust-cleanup fix in the generic transfer layer had scope creep into all transfer-dependent logic.
- **Fix:** Disabled the aToken rounding fix. Proper fix to be audited separately.
- **Timeline:** Report received → funds secured via liquidity addition pause in **2 hours**. Emergency stealth runtime upgrade enacted ~7 hours after report.
- **Lessons (from Hydration team):**
  1. **Never use `saturating_sub` by default** — prefer `checked_sub` / `checked_*` math; saturating arithmetic silently hides critical errors
  2. **Scope creep in generic layers is dangerous** — a transfer-level change affected all pallets that call transfers (Stableswap, Router, etc.)
  3. **Don't trust external subsystems** — add explicit balance sanity checks even when calling trusted pallets (EVM, AAVE integration)
  4. **Classify PRs by blast radius** — changes touching shared transfer logic need mandatory security review
  5. **EVM subsystem requires extra caution** — rebasing tokens, missing ED, and cross-VM interactions create unique attack surfaces
  6. **Circuit breakers for minting** — implement per-asset minting/burning limits per time period
  7. **Fuzz test with mainnet-like state** — fuzzing should cover realistic liquidity levels and invariants
- **Pattern:** `saturating_sub` masking insufficient balance → silent value creation. Generic fix in shared transfer layer causing unintended side effects in dependent pallets. Cross-VM (EVM↔Substrate) integration introducing assumptions that break Substrate-side invariants.

### Omnipool Single-Sided Liquidity Manipulation (Critical)
- **Source:** Bug bounty report, March 2023
- **Impact:** Repeated iterations could progressively extract value from pool reserves. The protocol's hub-asset subsidy on single-sided liquidity provision created a manipulation cycle that was profitable per iteration, compounding with each repetition.
- **Mechanism:**
  1. Omnipool allows single-sided liquidity provision — the user provides one token and the protocol mints the matching hub asset (LRNA) for the other side.
  2. **Basic attack (2-token pool):** (a) Swap token A → B to move B's price up. (b) Add single-sided liquidity in token B at the now-inflated price — the protocol mints LRNA to match, reducing slippage for the next step. (c) Swap back B → A, getting more A than initially spent because the protocol's LRNA subsidy absorbed part of the slippage. (d) Remove liquidity added in (b). Even after the `remove_liquidity` impermanent loss correction (`delta_b` adjustment), the total extracted value exceeds the attacker's input.
  3. **Multi-token variant:** Distribute the initial swap across multiple assets (e.g., swap into 3 tokens simultaneously), add single-sided liquidity in the target asset, then swap back across all assets. This variant is profitable *even without removing the added liquidity* — meaning fixing `remove_liquidity` alone was insufficient.
  4. The protocol's IL correction formula was:
     ```
     delta_b = (p*r - hub_reserve) * shares_removed / (p*r + hub_reserve)
     ```
     This adjustment fell short of the actual profit extracted through the manipulation cycle.
  5. Each iteration was profitable even with modest capital (20% of TVL). Profitability scaled super-linearly with attack capital relative to pool size.
- **Root cause:** The protocol bearing the cost of the hub-asset side of single-sided liquidity provision creates a fundamental accounting asymmetry. In traditional two-sided AMMs (e.g., Uniswap V2), the liquidity provider bears the full cost of reduced slippage on both sides, making this manipulation unprofitable by construction. In Omnipool, the protocol subsidized one side, and the IL correction formula did not fully account for the value extracted during the manipulation cycle.
- **Mitigation approach:** The Omnipool now has multiple layers of defense that make this class of attack impractical:
  - Per-block price change limits (max 50%)
  - Dynamic fees that increase with volatility
  - Per-block liquidity add/remove caps (5% of pool)
  - Circuit breakers on large state changes
  - Oracle-based price deviation checks
  - The IL correction math has been revised
- **Pattern:** Protocol-subsidized single-sided liquidity enabling manipulation cycles where the protocol absorbs impermanent loss that should be borne by the attacker. Fundamental to any AMM design where the protocol provides one side of the liquidity — the correction/fee mechanism must fully capture the value extracted during price manipulation, not just the static IL at removal time.

### Cross-Pool LP Share Theft in XYK Liquidity Mining (Critical)
- **Source:** Immunefi bug bounty, reported August 2025
- **Impact:** Up to ~$200K extractable value. Attacker could unlock higher-value LP shares from a different pool using a deposit from a lower-value pool.
- **Mechanism:**
  1. XYK liquidity mining stores deposits with an `amm_pool_id` identifying which pool the LP shares belong to.
  2. The withdrawal function accepts a user-supplied `AssetPair` parameter, which is used to derive `amm_pool_id` and determine which LP shares to unlock.
  3. **No validation** existed between the user-supplied `AssetPair` and the deposit's stored `amm_pool_id`.
  4. Attacker accumulates low-value LP shares (e.g., DOT/MYTH pair), deposits them into liquidity mining, then calls the withdrawal function with the `AssetPair` of a higher-value pool (e.g., DOT/EWT). The system unlocks and returns higher-value LP shares from the wrong pool.
- **Root cause:** Confused deputy — a user-controlled parameter that derives a security-critical identifier is not validated against the stored authoritative value. The function trusted user input to select which resource to operate on.
- **Fix:** Added validation that the `AssetPair`-derived `amm_pool_id` matches the deposit's stored `amm_pool_id`.
- **Pattern:** Missing cross-validation between user-supplied resource selectors and stored authoritative identifiers. Any function that accepts a user-supplied key to look up or operate on a resource must verify it matches the resource's actual association.
