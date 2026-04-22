# Enforce pool invariants in production — implementation spec

## Goal

Run Omnipool and Stableswap invariant checks in production. Today they
are gated to `#[cfg(any(feature = "try-runtime", test))]` and use
`panic!` / `assert!` / `.unwrap()`. We want:

- **Production:** on violation → return a `DispatchError`; the existing
  `#[transactional]` scope rolls back all storage writes.
- **Debug / test / try-runtime (fuzzer):** on violation → panic loudly
  so CI catches it.

## Requirements

- Every `ensure_*_invariant` helper returns `DispatchResult`.
- **Zero** `.unwrap()` / `assert!` / `panic!` reachable from a
  production dispatch path.
- On violation: return `Error::<T>::InvariantError` (one variant per
  pallet — Omnipool, Stableswap).

## Changes

### 1. New error variant

- `pallets/omnipool/src/lib.rs` → append `InvariantError` to
  `Error<T>`.
- `pallets/stableswap/src/lib.rs` → append `InvariantError` to
  `Error<T>`.

### 2. Omnipool — `ensure_trade_invariant` (lib.rs:2654–2702)

Replace the current body with the O(1) delta check.

**New signature:**

```rust
fn ensure_trade_invariant(
    asset_in: (T::AssetId, AssetReserveState<Balance>, AssetReserveState<Balance>),
    asset_out: (T::AssetId, AssetReserveState<Balance>, AssetReserveState<Balance>),
    balance_one: Balance,   // P_before − Ra_before − Rb_before, captured by caller
) -> DispatchResult
```

**Body (sketch):**

```rust
// Per-asset R·Q monotone (input side — existing check)
let rq_before = U256::from(old_in.reserve)
.checked_mul(U256::from(old_in.hub_reserve))
.ok_or(Error::<T>::InvariantError) ?;
let rq_after = U256::from(new_in.reserve)
.checked_mul(U256::from(new_in.hub_reserve))
.ok_or(Error::<T>::InvariantError) ?;
ensure!(rq_after >= rq_before, Error::<T>::InvariantError);

// LRNA accounting — O(1) delta check (replaces the Assets::iter().fold(..))
let p_after = T::Currency::free_balance(T::HubAssetId::get(), & Self::protocol_account());
let balance_two = balance_one
.checked_add(new_in.hub_reserve).ok_or(Error::<T>::InvariantError) ?
.checked_add(new_out.hub_reserve).ok_or(Error::<T>::InvariantError) ?;
ensure!(balance_two == p_after, Error::<T>::InvariantError);
Ok(())
```

**Caller changes** (both `do_sell`/`do_buy` at `lib.rs:1065` and
`do_hub_trade` at `lib.rs:1313`):

```rust
// Before any transfers
let p_before = T::Currency::free_balance(T::HubAssetId::get(), & Self::protocol_account());
let balance_one = p_before
.checked_sub(old_in_state.hub_reserve).ok_or(Error::<T>::InvariantError) ?
.checked_sub(old_out_state.hub_reserve).ok_or(Error::<T>::InvariantError) ?;

// … trade executes …

let r = Self::ensure_trade_invariant(
(asset_in,  old_in_state,  new_in_state),
(asset_out, old_out_state, new_out_state),
balance_one,
);
debug_assert!(r.is_ok(), "Omnipool trade invariant: {:?}", r);
r?;
```

**Edge cases the delta formula must handle:**

- **Hub-asset trade path** (`lib.rs:1313`): only one non-hub asset;
  user transfers LRNA directly. Formula adjusted to include the
  user-side LRNA flow (already known to the trade path).
- **Add/remove liquidity** (`ensure_liquidity_invariant` call sites at
  `lib.rs:2487` and `:2649`): single asset involved; delta on that
  asset + known `delta_hub_reserve` from the LP op.

### 3. Omnipool — `ensure_liquidity_invariant` (lib.rs:2704–2729)

Same refactor pattern: helper returns `DispatchResult`, caller uses
`debug_assert!(r.is_ok()); r?;`. Fix all `checked_mul(..).unwrap()` and
`checked_add(..).unwrap()` → `.ok_or(Error::<T>::InvariantError)?`.
`debug_assert!` at lines 2724 and 2725–2728 → `ensure!` with the same
comparisons, same `one = 1e12` slack.

### 4. Stableswap — all three `ensure_*_invariant` helpers

Refactor signatures to accept pre-resolved state from the caller.

**New signature (trade):**

```rust
fn ensure_trade_invariant(
    pool: &PoolInfo<T::AssetId, BlockNumberFor<T>>,
    pegs: &[PegType],
    initial_reserves: &[AssetReserve],
) -> DispatchResult
```

**Body:**

```rust
// Debug-only drift guard — compiled out in release
debug_assert_eq!(
    *pool,
    Pools::<T>::get(pool.id).expect("pool must exist in invariant check"),
);

// final_reserves re-read from storage — this is the invariant's job
let final_reserves = pool
.reserves_with_decimals::<T>( & Self::pool_account(pool.id))
.ok_or(Error::<T>::InvariantError) ?;

let amp = Self::get_amplification(pool);
let initial_d = hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(
initial_reserves, amp, pegs,
).ok_or(Error::<T>::InvariantError) ?;
let final_d = hydra_dx_math::stableswap::calculate_d::<D_ITERATIONS>(
& final_reserves, amp, pegs,
).ok_or(Error::<T>::InvariantError) ?;

ensure!(final_d >= initial_d, Error::<T>::InvariantError);
Ok(())
```

**Add/remove liquidity helpers** follow the same pattern, with the
appropriate D and R checks:

- Add: `final_d >= initial_d` and `final_r >= initial_r`.
- Remove: `final_d <= initial_d` and `final_r >= initial_r`.

All `FixedU128::from_rational(n, d)` calls get guarded:
`ensure!(!d.is_zero(), Error::<T>::InvariantError)` immediately before
the call (or switch to `checked_from_rational`).

**Caller changes** (sell, buy, add_liquidity, add_liquidity_shares,
remove_liquidity, remove_liquidity_one_asset, withdraw_asset_amount):

- Pass `&pool`, the resolved `&pegs` vector, and `&initial_reserves`
  to the invariant helper.
- Wrap every call in the `debug_assert!(r.is_ok()); r?;` pattern.

### 5. Call-site pattern (every invariant call site)

Replace every existing gated call:

```rust
#[cfg(any(feature = "try-runtime", test))]
Self::ensure_*_invariant(..);
```

With the unconditional dual-behaviour pattern:

```rust
let r = Self::ensure_*_invariant(...);
debug_assert!(r.is_ok(), "<pallet> <op> invariant: {:?}", r);
r?;
```

Applies at:

- Omnipool: `lib.rs:1065`, `:1313`, `:2487`, `:2649`.
- Stableswap: `lib.rs:703`, `:807`, `:914`, `:1083`, `:1619`, `:1717`,
  `:1817`, `:1913`.

### 6. Tests

- Existing proptest suites in `math/src/stableswap/tests/invariants.rs`
  and `pallets/omnipool/src/tests/invariants.rs` must continue to pass
  unchanged.
- Add **negative integration tests** per pallet that force an invariant
  violation (e.g. via direct `Assets::<T>::insert` / balance mutation)
  and assert the extrinsic returns `Error::InvariantError` in a
  **release** build without panicking.
- Add one regression test that a normal trade/LP op still succeeds
  with the new checks active.

### 7. Benchmarks

Re-run benchmarks for every extrinsic touched:

- Omnipool: `sell`, `buy`, `add_liquidity`, `remove_liquidity`,
  `sell_hub`, `buy_hub`.
- Stableswap: `sell`, `buy`, `add_liquidity`, `add_liquidity_shares`,
  `remove_liquidity`, `remove_liquidity_one_asset`,
  `withdraw_asset_amount`.

Weights land in `runtime/hydradx/src/weights/pallet_*.rs` — do not
hand-edit (per `CLAUDE.md`).

### 8. Runtime version

- Bump `spec_version` in `runtime/hydradx/src/lib.rs` (new `Error`
  variants are visible in metadata).
- Bump affected crate versions per repo policy.

## Acceptance checklist

- [ ] `Error::<T>::InvariantError` appended to both pallets' `Error`
  enums.
- [ ] All `ensure_*_invariant` helpers return `DispatchResult`.
- [ ] No `.unwrap()` / `assert!` / `panic!` reachable from any
  production dispatch path.
- [ ] Omnipool LRNA check is O(1) — no `Assets::iter()` in the
  invariant.
- [ ] Stableswap invariants accept state as parameters; only
  `final_reserves` is read fresh from storage.
- [ ] `FixedU128::from_rational` guarded against zero denominator
  everywhere it's called inside an invariant.
- [ ] Every invariant call site uses the
  `debug_assert!(r.is_ok()); r?;` pattern.
- [ ] Negative release-build tests pass.
- [ ] Proptest suites pass unchanged.
- [ ] Benchmarks regenerated.
- [ ] `spec_version` bumped; changelog updated.
