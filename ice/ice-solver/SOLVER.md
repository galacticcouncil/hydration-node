# ICE Solver

## Overview

The ICE (Intent Composing Engine) solver takes a batch of swap intents and produces a `Solution`: which intents to resolve, at what rates, and which AMM pool trades to execute.

The solver is generic over `AMMInterface` — any AMM simulator that can provide spot prices and simulate sell trades.

## Design Principle: Uniform Pricing, No Exploitation

The solver enforces a single clearing rate per direction — all intents selling A→B get the same price, regardless of their individual slippage tolerance. This is a deliberate design choice that prevents exploitation:

- **No slippage extraction**: if one intent sets a loose limit (e.g., 50% slippage) and another sets a tight limit (e.g., 5%), both receive the same rate. A counterparty cannot take advantage of the loose limit to capture the difference — the surplus above each intent's minimum contributes to the solution score, not to any individual participant.
- **Spot price as the ceiling**: the AMM spot price is the best rate theoretically achievable (the marginal price at zero volume). Any trade with actual volume gets equal or worse due to price impact. The satisfiability filter in Phase 1 uses this fact — if an intent's minimum output exceeds what spot prices would give, no combination of matching or AMM routing can ever fill it, so it is removed before it can distort clearing prices for other intents.
- **Fair matching**: direct matching gives the scarce side approximately spot rate (peer-to-peer, no AMM impact). The excess side bears slippage only on the net imbalance sent to the AMM — still better than routing everything through the pool.

## Algorithm — Per-Direction Clearing Prices with Direct Matching

### Phase 1: Spot Prices & Satisfiability

Fetch spot prices for all assets referenced by intents (relative to a denominator asset). Filter out intents whose minimum output exceeds what spot prices would give — these can never be satisfied.

If only one satisfiable intent remains, short-circuit to a direct AMM trade.

### Phase 2: Iterative Clearing Price Discovery (Simulation)

Group intents by unordered asset pair `(A, B)` and split into forward (A→B) and backward (B→A) directions. For each pair:

1. **Analyze net flow** via `analyze_pair_flow` to classify the pair as:
   - `SingleForward` / `SingleBackward` — only one direction has volume, pure AMM trade
   - `ExcessForward` / `ExcessBackward` — both directions, one side has more value at spot; the scarce side is fully matched peer-to-peer, the excess remainder goes to AMM
   - `PerfectCancel` — volumes cancel exactly at spot, no AMM needed

2. **Simulate AMM sell** for the net imbalance to get per-direction clearing rates (with `adjust_amm_output` tolerance applied).

3. **Filter**: remove intents whose minimum output exceeds what their direction's clearing rate would give.

4. **Repeat** until the set stabilizes or 10 iterations pass.

### Phase 3: Ring Trade Detection

Build a directed flow graph from remaining intents. Detect feasible 3-asset cycles (A→B→C→A) where all participants can be filled at spot-rate-consistent prices.

Ring fills are peer-to-peer — no AMM interaction. Each ring is filled at the bottleneck volume (smallest edge converted to a common denomination). Multiple rings can be found iteratively.

### Phase 4: AMM Execution

For each asset pair, subtract ring-filled volumes and execute the remaining net imbalance through the AMM. The AMM state is mutated sequentially across pairs.

Clearing rates from this phase are per-direction:
- **Scarce side**: gets spot-rate output from direct matching
- **Excess side**: gets (direct match output + adjusted AMM output) / total input

On AMM failure, the excess side falls back to spot rate.

### Phase 5: Rate Unification

Blend ring fills and AMM fills into a single per-direction rate:

```
unified_rate = (ring_total_out + amm_portion_out) / total_in
```

This ensures all intents in the same direction get the same rate, regardless of individual ring fill proportions.

### Phase 6: Intent Resolution

Apply the unified rate to each intent. The first intent per direction establishes a canonical `Ratio`; subsequent intents derive amounts from it, guaranteeing on-chain `validate_price_consistency` tolerance of ≤ 1.

Intents whose computed output falls below their minimum (due to rounding or rate adjustments) are dropped. Score = sum of surplus across all resolved intents.

## Key Properties

- **Per-direction clearing prices**: all intents selling A→B get the same rate. All intents selling B→A get the same rate. These two rates do NOT need to be inverses — the spread is surplus from direct matching.
- **Asymmetric matching benefit**: the scarce side (less volume) gets approximately spot rate (matched peer-to-peer, no AMM slippage). The excess side bears AMM impact only on the net imbalance.
- **Direct pair routing**: AMM trades go directly A→B (router finds optimal route), not forced through a hub/denominator asset.
- **Ring trades**: 3-asset cycles are filled peer-to-peer at spot prices, avoiding AMM entirely. Longer cycles (4+) are not attempted.
- **Simulation-execution consistency**: `adjust_amm_output` is applied in both the filtering phase (simulation) and the execution phase, preventing marginal intents from passing filtering but failing at resolution.

## AMM Simulation Tolerance

The solver simulates AMM trades off-chain. On-chain execution may produce slightly different results due to rounding differences (e.g., slip fee calculations, intermediate precision).

`AMM_SIMULATION_TOLERANCE_BPS` (currently 1 bps = 0.01%) is subtracted from simulated AMM output. This adjusted value is used for both `PoolTrade.amount_out` (on-chain `min_amount_out`) and clearing rate computation, ensuring the pallet account always has enough tokens from AMM trades to pay resolved intents.

For very small outputs (< 10,000 units), integer truncation means no deduction is applied. This is acceptable since production token amounts are typically 10^12+.

## Overflow-Safe Arithmetic

Real AMM spot prices use 128-bit `Ratio` values (numerator/denominator). Cross-products can reach ~10^76, near U256 max (~1.15 × 10^77).

`calc_amount_out` uses multiple strategies to avoid overflow while preserving precision:

1. **Direct**: `amount_in * (pi.n * po.d) / (pi.d * po.n)` — most precise, tried first
2. **Split**: when direct overflows but ratio ≥ 1, decompose into quotient + remainder
3. **Cross-cancel**: `(amount_in * pi.n / po.n) * (po.d / pi.d)` — divides similar-magnitude values first
4. **Step-by-step**: `(amount_in * pi.n / pi.d) * po.d / po.n` — divide early to keep values small

`mul_div(a, b, c)` computes `a * b / c` with the same overflow-protection approach.

## Structure

```
ice/ice-solver/src/
├── lib.rs
├── common/
│   ├── mod.rs              calc_amount_out, mul_div, is_satisfiable,
│                            collect_unique_assets, FlowDirection,
│                            analyze_pair_flow
│   ├── flow_graph.rs       FlowGraph, IntentEntry, MatchFill,
│                            build_flow_graph
│   └── ring_detection.rs   RingTrade, detect_rings, fills_meet_limits,
│                            fill_intent
└── v1/
    ├── mod.rs
    └── solver.rs            Solver<A>, PairClearing, DirAccum,
                              solve, solve_single_intent,
                              compute_pair_clearing
```
