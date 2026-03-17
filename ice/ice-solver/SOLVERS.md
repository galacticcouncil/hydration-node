# ICE Solver

## Overview

The ICE (Intent Composing Engine) solver takes a batch of swap intents and produces a solution: which intents to resolve, at what rates, and which AMM trades to execute. The goal is to maximize user surplus while satisfying all limit prices.

## Algorithm — Per-Direction Clearing Prices with Direct Matching

1. Get spot prices, filter satisfiable intents
2. Single intent fast path → direct AMM trade
3. Group intents by unordered pair, compute net flow
4. Simulate selling net imbalance through AMM → per-direction clearing prices
5. Iteratively filter intents unsatisfied at clearing price until stable
6. Ring trade detection (3-cycles) for cross-pair flows
7. Execute actual AMM trades for net imbalances
8. Resolve intents: same direction = same rate, opposite directions may differ

## Key Properties

- **Per-direction clearing prices**: all intents selling A→B get the same rate. All intents selling B→A get the same rate. These two rates do NOT need to be inverses.
- **Direct matching benefit is asymmetric**: the scarce side (less volume) gets approximately spot rate (matched peer-to-peer, no AMM slippage). The excess side bears the AMM impact on the net imbalance — but less than without matching since the matched volume doesn't touch the AMM.
- **Direct pair routing**: AMM trades go directly A→B (router finds optimal route), not forced through denominator. Less slippage than a hub-and-spoke approach.
- **Iterative filtering**: removes intents that can't be satisfied at the actual clearing rate (worse than spot due to AMM slippage), recomputes until stable.
- **Ring detection**: identifies 3-asset cycles (A→B→C→A) and fills them peer-to-peer at spot-rate-consistent prices, avoiding any AMM interaction.
- **Canonical price rounding**: first intent in each direction establishes a canonical Ratio; all subsequent intents derive amounts from it, guaranteeing on-chain `validate_price_consistency` (tolerance ≤ 1).
- **Unified rates**: ring fills and AMM fills are blended into a single per-direction rate, ensuring price consistency regardless of individual ring fill proportions.

## AMM Simulation Tolerance

The solver simulates AMM trades off-chain to compute expected outputs. The on-chain execution may produce slightly different results due to rounding differences between the simulator and the real AMM math (e.g., slip fee calculations).

A configurable tolerance (`AMM_SIMULATION_TOLERANCE_BPS`) is applied to both trade `min_amount_out` and clearing rates to ensure on-chain execution succeeds. Currently set to 1 basis point (0.01%).

## Overflow Handling

Real AMM spot prices use 128-bit Ratio values (numerator/denominator). Cross-products of these can reach ~10^76, near U256 max (~1.15 × 10^77). The `calc_amount_out` function uses multiple computation strategies to avoid overflow:

1. **Direct**: `amount_in * (pi.n * po.d) / (pi.d * po.n)` — most precise
2. **Split**: when `amount_in * n` overflows but `n >= d`, split into quotient + remainder
3. **Cross-cancel**: `(amount_in * pi.n / po.n) * (po.d / pi.d)` — divides similar-magnitude values first
4. **Step-by-step**: `(amount_in * pi.n / pi.d) * po.d / po.n` — divide early, accumulate

The `mul_div(a, b, c)` helper computes `a * b / c` with overflow protection.

## Structure

```
ice/ice-solver/src/
├── common/
│   ├── mod.rs            (calc_amount_out, mul_div, is_satisfiable, etc.)
│   ├── flow_graph.rs     (FlowGraph, IntentEntry, MatchFill, build_flow_graph)
│   └── ring_detection.rs (RingTrade, detect_rings)
├── lib.rs
└── v1/
    ├── mod.rs
    └── solver.rs         (Solver — main solver implementation)
```
