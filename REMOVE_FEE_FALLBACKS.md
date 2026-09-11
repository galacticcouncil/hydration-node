# Removal of the DOT/XYK fee fallbacks and redesign of the insufficient-asset ED

**Worktree:** `/Volumes/T9/workspace/gc/remove-fee-fallbacks`
**Branch:** `feat/remove-fee-fallbacks` (off `master` @ `94b5af674`, merged up to `cc1bc978b` on 2026-08-20)
**Status:** implemented, uncommitted.

---

## 1. What was removed

| # | Feature | Outcome |
|---|---|---|
| A | Any asset becomes a fee currency if an XYK `(X, DOT)` pool exists | removed |
| B | ED for insufficient assets (charged in the payer's fee asset, refunded in HDX from a shared treasury lock) | **removed entirely** — no ED is charged or refunded, and permissionless registration (`register_external`) is deleted |
| C | DCA pays its execution fee by selling a non-accepted `asset_in` into XYK for DOT | removed |
| D | Adjacent DOT-routed valuation used by the withdraw circuit breaker and EVM | replaced with a route + EMA-oracle valuation |

Governance fallback pricing for **accepted** currencies (`§4` of the original plan) is **explicitly
deferred** — `on_initialize` still substitutes the stored fallback price when the oracle has none,
and `get_currency_price` still has its three-level chain. That is the accepted-tier analogue of the
hole closed here and is the natural follow-up PR.

---

## 2. A — the swappable fee-asset tier

`is_transaction_fee_currency` meant "HDX or in `AcceptedCurrencies`"; every `else` branch behind it
was tier 2, gated only by `XYK::exists((X, DOT))`. All of them are gone:

- `traits/src/fee.rs` — `SwappablePaymentAssetTrader` deleted. `InspectTransactionFeeCurrency` was
  deleted too: once the adapters and DCA stopped needing the predicate it had zero users.
- `runtime/hydradx/src/assets.rs` — `XykPaymentAssetSupport` (the only impl) and `DotAssetId` deleted.
- `pallets/transaction-multi-payment/src/lib.rs`
  - `do_set_currency` and `AccountFeeCurrency::is_payment_currency` collapse to a single
    `currency == NativeAssetId || AcceptedCurrencies::contains_key(currency)` check.
  - `withdraw_fee` loses the X→DOT buy; it prices in the chosen currency or fails.
  - `can_withdraw_fee` loses its early `Ok` bypass, so **every** fee currency now gets a
    pre-dispatch balance check.
  - `Config::SwappablePaymentAssetSupport` and `Config::PolkadotNativeAssetId` removed.
  - **`get_currency_price` now gates on whitelisting before consulting any oracle.** Deleting the
    tier-2 branch had left it reachable for any asset with a live oracle route, and
    `resolve_currency_from_call` will hand it whatever currency a batched `set_currency` names —
    i.e. the XYK gate came out and an oracle-route gate quietly took its place. Caught by
    `set_currency_in_batch_should_fail_for_unaccepted_asset_with_oracle_price`.
  - **`account_currency` filters a stored-but-de-listed currency back to the default.** The EVM
    path resolves its fee currency separately (through `ConvertBalance`, not `get_currency_price`),
    so without this the two paths would disagree about what is payable. It also makes the
    multi-block purge below pure cleanup rather than a correctness prerequisite.

Both of those add one `AcceptedCurrencies` read to the fee hot path, so
**`pallet_transaction_multi_payment` weights need regenerating** alongside `pallet_dca`.
- `runtime/hydradx/src/evm/evm_fee.rs` — `TransferEvmFees` loses its X→DOT branch and two type params.
- `runtime/adapters/src/price.rs` — `ConvertBalance` is now a plain oracle conversion with one type
  param. Its two remaining callers (EVM runner balance check, EVM fee charge) only ever see
  accepted↔accepted pairs once account fee currencies are constrained, so the XYK branches were dead
  weight. `FeeAssetBalanceInCurrency` drops the two DB reads it charged for the membership checks.
- `pallets/dca/src/lib.rs` — `Config::SwappablePaymentAssetSupport` and
  `Config::PolkadotNativeAssetId` removed.
- Runtime wiring in `system.rs` and `assets.rs` removed accordingly.

**Migration — `PurgeUnsupportedFeeCurrencies`** (`runtime/hydradx/src/migrations/remove_fee_fallbacks.rs`).
Multi-block (`SteppedMigration`, registered in `MultiBlockMigrationsList`, which was empty) because
`AccountCurrencyMap` is unbounded and a single-block scan of it is not safe. `pallet-migrations`
holds normal extrinsics back until it finishes, so no affected account can transact against a
half-purged map. Entries pointing at a now-invalid asset are removed; those accounts fall back to
HDX, or WETH for EVM accounts, via `account_currency`.

---

## 3. B — insufficient-asset ED, removed entirely

Old: charge 1.1 × HDX ED **in the payer's fee asset** (or via a DOT/XYK swap) to the treasury, lock
1 ED of HDX on the treasury, refund `locked / counter` HDX to the **killed account**. The
pay-in-one-asset / refund-in-HDX cycle was the surface of the reported treasury-drain shape and the
reason the whole mechanism was judged a security liability. An earlier iteration of this branch
redesigned it as a reserve held on the payer; that was dropped before shipping — **no ED cycle of
any kind survives**.

New: **nothing is charged and nothing is refunded.** Creating a token account for an insufficient
asset costs only the transaction fee. Account existence never depended on the toll — orml-tokens
itself takes a `providers` ref for every token account — so removal strands nobody.

What made this safe is closing the front door at the same time:

- `AssetRegistry::register_external` (call index 4, permissionless, always insufficient) is
  **deleted**, together with `Config::RegExternalWeightMultiplier`. The set of insufficient assets
  is now closed: governance `register`/`update` (which can still set or promote the flag,
  deliberately — the flag now only gates the oracle whitelist, fee-currency eligibility and
  xyk-liquidity-mining reward currencies) and the internal registry trait fns used by the protocol
  itself for XYK share tokens and bonds (`register_insufficient_asset`,
  `get_or_register_insufficient_asset` — protocol-derived assets, not XCM-importable spam).
- Dust-spam control over the closed legacy set is **reactive**: `ban_asset` blocks every
  transfer and deposit of a banned asset. The ban check is the only thing left of
  `SufficiencyCheck`, renamed to `BannedAssetCheck` (`runtime/hydradx/src/assets.rs`). Note the
  economics: the old deterrent was one *refundable* HDX per account — barely stronger than the
  transaction fee that still applies.

### Legacy `sufficients` refs are grandfathered forever

The ~104k refs created by the old scheme are **never released** — the kill-hook release shim was
removed, not kept. Rationale: nothing on chain attributes a ref to the scheme, and `sufficients` is
shared with `pallet-evm-accounts` bindings and Frontier contract accounts. A release-on-kill shim
without a payment record would let anyone consume a foreign ref by receiving and dusting a spam
token (reachable by *any* account post-change, no longer only via root `set_balance`). The cost of
grandfathering: an emptied legacy holder lingers as an ~80-byte empty system account instead of
being reaped. Bounded by the legacy set, harms nothing, and matches the refund reality (zero).

**Migration — `RetireInsufficientEdPool`** (single-block): removes the treasury `SUFFICIENCY_LOCK`
and kills the `ExistentialDepositCounter` key. No one is owed anything: historical payments were
never recorded per-payer, and the pooled scheme was already refunding zero (mainnet: counter
104,185 against an empty treasury lock).

### Behavioural consequences to flag

- Receiving a first insufficient asset no longer requires the receiver (or sender) to hold HDX at
  all — a fresh address can now be funded with only an insufficient asset.
- **The app's permissionless "import external asset" flow dies with `register_external`.** New
  external assets are onboarded exclusively through governance `register`.
- Governance gains a working spam lever it already had (`ban_asset`) as the only dust-spam control;
  a proactive ban list for known junk can ship as a normal referendum any time, independent of the
  upgrade.

---

## 4. C — DCA

**Selling an asset that is not a fee currency stays supported.** Only the way its fee is paid changes.

- `asset_in` **is** an accepted fee currency → unchanged: fee priced in `asset_in` via the oracle,
  `unallocate`d from the reserved budget, sent to the fee receiver.
- `asset_in` **is not** → fee is priced in the owner's own fee currency
  (`AccountFeeCurrency::get(owner)`, always accepted so always oracle-priced) and taken from their
  **free balance**. The reserved budget is never touched — it is denominated in an asset the protocol
  refuses to price. No swap, no XYK, no DOT.

The predicate is `AccountFeeCurrency::is_payment_currency`, the same `AcceptedCurrencies` source of
truth as the rest of the fee system. New `pallet-dca` config type `AccountFeeCurrency`, wired to
`MultiTransactionPayment`.

Knock-on effects, because `get_transaction_fee` also sizes the budget:

- New `budget_transaction_fee` returns 0 when the sold asset is unpriceable, so `schedule`'s reserve
  is `amount_in * 2` rather than `(amount_in + fee) * 2`, and `replan_or_complete` is governed by
  `MinimumTradingLimit` alone. Without this, every such schedule would terminate after its first trade.
- `MinBudgetInNativeCurrency` is **skipped** for an unpriceable `asset_in` — it is native-denominated
  and cannot be expressed in an asset with no price. Spam is still bounded by the per-execution fee
  and `MinimumTradingLimit`.
- The fee-currency transfer uses `KeepAlive`: failing to pay terminates the schedule and returns the
  budget rather than reaping the owner's account.

Also: `convert_to_polkadot_native_asset` deleted; `get_trade_weight` has a single weight variant per
order kind; `on_initialize_with_{buy,sell}_trade_with_insufficient_fee_asset` removed from
`pallets/dca/src/weights.rs`, `runtime/hydradx/src/weights/pallet_dca.rs` and
`runtime/hydradx/src/benchmarking/dca.rs`. **`pallet_dca` weights need regenerating** — the two
branches do different storage work, so benchmark the heavier one.

No migration. Existing schedules keep working: an accepted `asset_in` is unaffected, and a
non-accepted one simply starts charging the owner's fee currency.

---

## 5. D — withdraw circuit-breaker valuation

`WithdrawCircuitBreaker::convert_to_hdx` (`runtime/hydradx/src/circuit_breaker.rs`) keeps its
accepted-tier fast path and replaces the raw XYK/DOT quote with `TenMinutesOraclePrice` over the
asset's on-chain route — the same mechanism `MultiTransactionPayment::get_oracle_price` uses, just
generalised past the accepted tier. Manipulation-resistant (10-minute EMA over a registered route)
where the thing it replaces read instantaneous pool reserves.

**Fail-closed is preserved**: an asset with no accepted price and no oracle route has no valuation
and the operation is rejected, exactly as today. That is the protective behaviour for the bridged
assets the withdraw breaker exists to guard — but it means **on-chain routes must be registered for
every External/Erc20 asset the breaker accounts for before this upgrade goes out**, or their
withdrawals brick. See `scripts/onchain-routes/README.md`.

Note the blast radius is wider than the original plan recorded: `OnTransferHook` propagates the
conversion error too (not just `OnWithdrawHook`), so transfers to egress accounts are affected as
well. `OnDepositHook` still swallows it.

---

## 6. Tests

- `integration-tests/src/insufficient_assets_ed.rs` → **renamed `insufficient_assets.rs`** and
  rewritten for the no-toll semantics: transfer/deposit of an insufficient asset charges nothing
  and bumps no `sufficients` ref; a fresh address with zero HDX can receive one
  (`deposit_should_create_token_account_when_receiver_has_no_hdx` — previously impossible);
  killing a token account leaves a simulated legacy ref untouched
  (`token_account_kill_should_not_touch_sufficients_refs`); ban checks cover transfer to new and
  existing accounts plus deposit.
- `integration-tests/src/router.rs` — the four ED-refund/ED-charging router tests removed; the two
  kept insufficient-asset routing tests now assert HDX balances stay untouched.
- `integration-tests/src/bonds.rs` — the scaffolding that funded the bonds pallet account for the
  toll removed. `integration-tests/src/xyk_liquidity_mining.rs` and `runtime/hydradx/src/tests.rs`
  register external assets via governance `register` instead of `register_external`.
- `pallets/asset-registry` — the three `register_external` tests and its benchmark removed.
- `integration-tests/src/multi_payment.rs` — the two "insufficient/non-accepted asset can be used as
  fee currency" tests became negative tests
  (`set_currency_should_fail_when_insufficient_asset_has_xyk_dot_pool`,
  `set_currency_should_fail_when_sufficient_asset_is_not_accepted`).
- `integration-tests/src/dca.rs` — four swappable-tier tests and the whole `mod fee` weight-comparison
  module removed; `create_schedule_should_fail_when_asset_in_is_not_an_accepted_fee_currency` added.
- `integration-tests/src/evm_permit.rs` — the four `ConvertBalance`/insufficient-fee tests removed.
- Pallet mocks: `MockedInsufficientAssetSupport` deleted from both
  `pallets/transaction-multi-payment/src/mock.rs` and `pallets/dca/src/tests/mock.rs`.

---

## 7. Verified

Re-verified after the full ED removal (2026-08-15):

| Suite | Result |
|---|---|
| `pallet-asset-registry` | 53 passed (3 `register_external` tests removed) |
| `hydradx-runtime` unit tests | 51 passed, 6 ignored |
| integration `insufficient_assets` (new file) + router insufficient | 9 passed |
| integration router `receiving_shitcoin` | 1 passed |
| integration `bonds` | 3 passed |
| integration `multi_payment` | 5 passed |
| integration `create_schedule_should*` | 4 passed |
| integration `circuit_breaker` | 19 passed |
| integration `non_native_fee` | 14 passed |
| integration `evm_permit` | 39 passed |
| integration `xyk_liquidity_mining` (insufficient + bonds) | 3 passed — two master-era exact-balance pins re-pinned: DAVE keeps the 5.6/6.7 HDX the old scheme took (6−7 × 1.1 ED charges minus one 1.0 refund) |
| `cargo fmt --check` | clean |
| clippy `-D warnings` `--all-targets`: `pallet-asset-registry`, `hydradx-runtime` | clean |
| clippy `-D warnings` `--all-targets`: `runtime-integration-tests` | 28 pre-existing clippy-1.88 lints (`gigahdx*`, `stableswap_curve_comparison`, `dca.rs:5993`, `evm_permit.rs:2884`) — all verified to predate this branch, none in code this branch touches; left as-is |

From the earlier round, unchanged today: `pallet-transaction-multi-payment` 44 passed,
`pallet-dca` 110 + 51 passed.

Not run here: the full `dca::` integration suite (multi-hour, dominated by
stableswap/xyk/aave/gigahdx cases unrelated to this change) and `make build-benchmarks`
(though the runtime compiles under `--features runtime-benchmarks`).

Note: `cargo test -p <crate downstream of hydradx-adapters>` cannot build, because
`runtime/adapters/Cargo.toml` declares `orml-vesting` and `pallet-democracy` but never forwards
their `std` features. Pre-existing and invisible to CI, which builds the whole workspace and
unifies features. Worth a separate one-line PR.

## 8. Before merging

- [ ] Register on-chain routes for every External/Erc20 asset the withdraw breaker accounts for (§5).
- [ ] Pre-flight mainnet counts: `AccountCurrencyMap` entries with a non-accepted asset (sizes the
      multi-block migration); DCA schedules with a non-accepted `asset_in`.
- [ ] Regenerate `pallet_dca`, `pallet_transaction_multi_payment` **and `pallet_asset_registry`**
      weights (registry lost an extrinsic; its weights file was hand-trimmed to compile).
- [ ] `make build-benchmarks`.
- [ ] Full `cargo test --locked` in CI.
- [ ] Flag to the apps/wallet team: the selectable fee-asset list shrinks to `AcceptedCurrencies`,
      **and the permissionless "import external asset" flow is gone** (`register_external` deleted).
- [ ] Decide whether a proactive `ban_asset` referendum for known spam assets ships alongside the
      upgrade (reactive banning is the only dust-spam control now).
- [ ] Decide whether §4 (governance fallback pricing) lands as the follow-up.

Version bumps applied: `hydradx-traits` 5.0.0, `pallet-transaction-multi-payment` 11.0.0,
`pallet-dca` 2.0.0, `pallet-asset-registry` 4.0.0, `hydradx-adapters` 2.0.0, `hydradx-runtime`
440.0.0, runtime `spec_version` 440, and `transaction_version` 1 → 2 (extrinsic removed).
The runtime was re-bumped 439 → 440 at the master merge: master shipped its own 439 (pepl-v2 +
NTT mint lockdown), and both sides had written the same number, so the collision was silent.

Suggested PR title:
`feat(multi-payment)!: remove non-whitelisted fee assets and DOT/XYK fee fallbacks`
