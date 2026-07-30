# ICE solver fuzzing / property-soak harness

A standalone manual utility (`ice-fuzz` binary in this crate) that continuously
generates trading scenarios, solves them with the ICE solver, and — Tier 2 —
submits the solutions through the pallet, asserting solution invariants the
whole time. It runs against the **same `mainnet_apr` snapshot the integration
tests use**, so there are no mocks or simplifications.

It is **not** wired into CI. Run it manually after changing the solver, the
submission path, or the conservation/fee logic.

## Quick start

```sh
# 10-minute soak, both tiers, comparing v3 vs v4, stop on first violation:
FUZZ_SECONDS=600 FUZZ_TIER=both FUZZ_SOLVER=diff \
  cargo run -p ice-solver-bench --release --bin ice-fuzz

# Fast solver-only soak (thousands of scenarios):
FUZZ_SECONDS=120 FUZZ_TIER=solver cargo run -p ice-solver-bench --release --bin ice-fuzz

# Reproduce a single reported scenario by its seed:
FUZZ_SEED=123456789 FUZZ_ITERS=1 cargo run -p ice-solver-bench --bin ice-fuzz
```

Use `--release` for throughput. Use a **debug** build (or
`RUSTFLAGS="-C overflow-checks=on"`) when hunting arithmetic-overflow panics —
release builds wrap silently and the no-panic oracle won't see them.

## Two tiers

- **Tier 1 (`FUZZ_TIER=solver`)** — solves in-memory against the live simulator
  state and checks solution invariants. Fast (~thousands/sec in release). Finds
  logic bugs: limit violations, conservation breaks, non-determinism, panics.
- **Tier 2 (`FUZZ_TIER=submit`)** — submits intents as extrinsics, runs the
  solver, submits the solution to `pallet-ice`, and relies on the on-chain
  re-check (per-asset conservation + `score == exec_score`) as an authoritative
  oracle, plus a limit-respect check the chain itself skips. Slower (full
  execution per scenario). Each scenario's storage writes are rolled back
  (`with_transaction`) so the next starts from the pristine snapshot.

## The oracle (what "correct" means)

An optimizer has no cheap ground-truth optimum, so we assert properties every
*valid* solution must satisfy:

- **Limit respect** — every resolved user receives ≥ their minimum (pro-rata for
  partials). The pallet does NOT re-check this on submit, so the fuzzer is the
  only thing that catches a solver shortchanging a user.
- **Conservation** — per asset, `intent_in + pool_out − intent_out − pool_in ≥
  fee · matched` (mirrors `pallet-ice`'s `settle_matched_fees`; Tier 2 defers to
  the pallet's own check).
- **Determinism** — identical input gives a byte-identical solution (collators
  must agree).
- **Bounds / id-validity** — resolved ⊆ submitted, `amount_in ≤ original`,
  no duplicates, within `MAX_*` limits.
- **No panic / overflow** — the solver must never panic on any generated input.
- **Submission executes** (Tier 2) — a solution the solver produced must be
  accepted by the pallet; a deliberately over-paying solution must be rejected.
- **Differential (soft, reported)** — v4 trades ≤ v3, score ≥ v3. Reported as
  counters, not failures (v4 is not universally ≥ v3 — e.g. stableswap-routed
  pairs and some unbalanced flows).

## Scenario generation

A weighted grammar of archetypes biased toward ICE-relevant structure (pure
random intents just route independently and never exercise matching/netting):
`independent`, `opposing_pair`, `ring` (3–6 assets), `chain`, `whale_dust`,
`boundary` (limits on the feasibility edge), `degenerate` (empty / single /
duplicate / self-pair / max-size). Core asset universe is all-Omnipool —
HDX, DOT, BNC, WETH, ETH — for reliable routing.

## Reproducing & triaging a finding

Every scenario is derived from a single `u64` seed printed in the run header
and on every violation. On a violation the harness prints the seed, the
archetype, the broken invariants, and writes a SCALE-hex fixture to
`FUZZ_OUT` (default `./fuzz-findings/`). To replay, re-run with that
`FUZZ_SEED` and `FUZZ_ITERS=1`. Promote real findings to a committed regression
test via the `ice-solver-capture` workflow / `ice-solver/src/tests/regressions.rs`.

## Environment variables

| Var | Default | Meaning |
|-----|---------|---------|
| `FUZZ_SECONDS` | 60 | soak duration (0 = use `FUZZ_ITERS`) |
| `FUZZ_ITERS` | 0 | fixed scenario count (0 = use `FUZZ_SECONDS`) |
| `FUZZ_SEED` | 0 | run seed (0 = derive from clock, printed) |
| `FUZZ_TIER` | both | `solver` \| `submit` \| `both` |
| `FUZZ_SOLVER` | v4 | `v3` \| `v4` \| `diff` |
| `FUZZ_MAX_INTENTS` | 30 | max intents per scenario |
| `FUZZ_MAX_SLIP_PCT` | 5 | slip-fee cap percent |
| `FUZZ_KEEP_GOING` | 0 | `1` = don't stop on first violation |
| `FUZZ_REPORT_EVERY` | 200 | progress print cadence (scenarios) |
| `FUZZ_SNAPSHOT` | `mainnet_apr` | snapshot path override |
| `FUZZ_OUT` | `./fuzz-findings` | failure-fixture dir |

## Not yet covered (future work)

- Automatic delta-debugging shrinker (minimise a failing intent set).
- Tier-2 per-user balance-delta assertions (currently relies on the pallet's
  on-chain conservation/score check for execution correctness).
- DCA intents (only swap intents are generated).
- USDT / stableswap assets in the core generation set.
