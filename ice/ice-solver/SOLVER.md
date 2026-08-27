# ICE Solver — How It Works

`ice-solver` takes a batch of swap intents (users wanting to trade one asset for
another) and produces a `Solution`: which intents are resolved, at what amounts,
and which AMM trades the pallet must execute to make that settlement work.

There is one solver generation, **v4** (`ice_solver::v4::Solver`). It is generic
over `AMMInterface`, so the same code runs in the runtime (against the simulator
snapshot) and in the node worker (against the decoded snapshot). Earlier
generations (v2's per-pair `t`-scaling and v3's per-pair price crossing) have
been removed.

Two mechanisms deliver value that individual trades cannot:

- **Direct matching** — users wanting opposite trades settle against each other,
  with no pool and no slippage.
- **Global netting** — matching is computed at the *asset* level across the whole
  batch, so chains (A→B, B→C) and cycles of any length internalize, not just
  opposing pairs or 3-asset rings.

Only each asset's true residual imbalance reaches the AMM.

## Core principle: everyone in a direction gets the same price

All intents trading in the same direction settle at the same rate, regardless of
the limit each user was willing to accept. A user with a loose limit cannot be
picked off — they receive the same rate as everyone else, and the surplus above
their stated minimum is what the solution is scored on.

The two directions of a pair can settle at different rates. That gap is the
surplus captured by matching — value that would otherwise have been lost to pool
slippage.

Limits decide **inclusion and fill volume only**. They never become a payout.

## The pipeline

### 1. Spot prices

Every asset appearing in the batch is priced in `AMMInterface::price_denominator`
(the hub asset). The best rate across the discovered routes wins. An asset with
no viable route has no price; that fact changes which engine runs in step 4.

### 2. Candidate filter

An intent that cannot plausibly clear is dropped up front: a fully-filled
partial, or a non-partial whose minimum is unreachable both at spot and at a
direct route quote for its full volume. Partials are always kept — step 3 will
either trim them to a viable fill or drop them.

### 3. Per-pair crossing

Intents are grouped by unordered asset pair and split by direction. Each
direction is sorted by limit rate, loosest first. While a direction's uniform
rate fails its tightest included limit, that tightest intent is either

- **trimmed** — a bisection finds the largest fill at which the direction's
  uniform rate still clears its limit (partials, once), or
- **dropped** — all-or-nothing (non-partials, and partials with no feasible fill).

Volumes only ever ratchet down, so the loop is bounded and the rate monotonically
improves for the survivors.

Once both directions clear, the **existential-deposit remainder rule** applies:
a partial is never filled in a way that leaves `0 < remaining < ED`, because that
remainder could never be traded again. Enforcing the rule lowers a fill, and a
lower fill in one direction can starve the other direction of matched volume — so
the fit is re-checked afterwards and the affected partial is re-fitted rather
than left carrying a fill it can no longer be paid for.

### 4. Netting and trade building

**Global netting** (the normal path, when every intent asset has a spot price):

1. Every fill is valued in the hub numeraire, giving each asset a *sold* and a
   *demanded* total. Chains and cycles cancel here automatically.
2. Each asset with a surplus is routed **directly** to the assets in deficit (no
   forced hub hop), in ascending asset-id order for determinism. A residual trade
   is emitted only if it would actually execute on chain — see *Trades the pallet
   will run*, below — and an attempt that produces no trade does not consume the
   imbalance, so the surplus stays available for the next deficit asset.
3. For each output asset B, the distributable pot is
   `sold[B] + pool_out[B] − pool_in[B] − matched[B]·fee`, where
   `matched[B] = sold[B] − pool_in[B]`. Each directed pair ending in B receives a
   pro-rata share of that pot by hub-value claim. This is conservation-safe by
   construction: the pallet's `residual ≥ matched·fee` invariant holds for every
   asset.

**Pairwise fallback** (when any intent asset lacks a spot price, so the batch
cannot be valued globally): 3-asset ring detection, then per-pair flow analysis —
one-sided flow goes entirely through the pool; opposing flow absorbs the smaller
side at the reference rate and routes only the excess; exactly cancelling volumes
need no pool trade at all. Ring fills and AMM output are then blended into one
unified rate per direction.

For every AMM trade the solver simulates each discovered route against the
state as threaded through the preceding trades — the same order the pallet will
execute them in — and picks the best output. A 1 bps safety margin is subtracted
from the simulated output, so on-chain execution can never undershoot what the
solution claims.

### 5. Resolution

Per directed pair, the price is anchored on the pair's *largest* fill and that
intent is emitted first, so the pallet's own first-resolution anchor recomputes
an identical price and every smaller fill stays inside its ±1 rounding
tolerance. An intent is resolved only if the uniform rate pays at least its
pro-rata minimum (and its admission floor, when one is supplied) and both legs
clear their existential deposits.

### 6. Stabilization

Trades execute sequentially against a mutating state, so a later pair can drift
enough that an intent no longer clears. Any intent dropped at resolution is
removed and the round is re-run with the volumes that actually settled. If the
rounds are exhausted, the solver falls back to the best single-intent solution
rather than returning nothing.

### 7. Score

The score is the total surplus over all resolved intents: how much more each user
receives than their stated minimum. The pallet recomputes it from storage and
rejects any mismatch, so the score is always derived from the intent's own stored
`amount_out` — never from an externally supplied admission floor.

## Trades the pallet will run

`submit_solution` **skips** any trade whose input is below the ED of its first
asset or whose output is below the ED of its last asset. A skipped trade never
pays into the holding pot, so a solution that counted its output would promise
users more than the pot receives and abort on the conservation check. Both
engines therefore refuse to emit such a trade in the first place, and neither
counts its output.

## Admission floors

`solve_with_limits` accepts a per-intent minimum that the chain enforces on top of
the intent's own `amount_out` (the DCA oracle floor, for example). Floors gate
*admission* only: an intent that cannot be paid its floor is excluded, exactly as
one whose own minimum cannot be met. The score stays on the stored `amount_out`,
because the chain re-derives it from storage and any divergence is a
`ScoreMismatch`.

## Arithmetic

All rate application, hub valuation and price conversion is exact: products are
evaluated in `U512` and divided once, so no intermediate overflows and no
remainder term is silently dropped. When an exact result genuinely does not fit
128 bits, the helper returns `None` and the caller treats the flow as *unvaluable*
— it is never read as a zero-valued flow, which would classify a pair as one-sided
excess and hand the scarce side a zero rate. Fill bisections use an
overflow-safe midpoint and a budget large enough to converge exactly anywhere in
the `u128` range.

## Observability

Every solve emits one `info` line on the `solver::v4` target naming the outcome
(`NoIntents`, `NoCandidates`, `NoFillsAfterCrossing`, `SingleIntent`,
`Stabilized`, `SingleIntentFallback`, `Exhausted`) together with the intent,
candidate, resolved, trade, route and unroutable-pair counts — so an empty
solution says *which stage* emptied the batch instead of being indistinguishable
from an empty block. Arithmetic that cannot be represented and residual
imbalance that cannot be routed are logged at `warn`.

## On-chain execution

The solution is submitted as an unsigned transaction. The pallet:

1. **Unlocks and collects** — unreserves each resolved intent's input and
   transfers it to the ICE holding account.
2. **Executes the AMM trades** — from the holding account, in solution order,
   skipping any dust trade.
3. **Pays out** — transfers each resolved output to its owner, checks one price
   per directed pair, sweeps the matched-volume fee, and verifies the recomputed
   score matches.

## Known limitations

**Batch slippage can exclude viable intents.** Same-direction intents share the
AMM slippage of their combined volume, so an intent that would clear as an
individual trade can be excluded by the batch rate. The solver does not search
for the subset that maximises the number of resolved intents.

**Rings only in the fallback engine.** Explicit cycle detection is limited to 3
assets and only runs in the pairwise fallback. The global-netting path does not
need it — cycles of any length cancel at the asset level — but a batch that falls
back because one asset is unpriced gets the narrower treatment.

**Single-pass route selection.** Routes are chosen per pair against the state at
that point in the trade sequence; the solver does not search for a globally
optimal ordering.

**Bounded route discovery.** Route enumeration is capped in both routes returned
and paths explored (`route-findr`'s `SearchLimits`), shortest routes first. A
pathologically connected pool set can therefore hide a long route that would have
priced marginally better.

**Simulation tolerance.** The 1 bps margin covers rounding differences between
the off-chain simulator and on-chain pool math. Larger divergence — pool state
changing between simulation and execution — can still make execution fail.
