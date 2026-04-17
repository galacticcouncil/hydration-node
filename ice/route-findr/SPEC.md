# route-suggester — Architecture Spec

> **Crate:** `route-suggester` | **Date:** 2026-03-31
> **Scope:** `src/lib.rs`, `src/types.rs`, `src/graph.rs`, `src/bfs.rs`, `src/strategy.rs`
> **Origin:** Ported from TypeScript SDK `packages/sdk-next/src/sor/route/`

---

## 1. Purpose

Enumerates **all valid multi-hop trading routes** between two assets on Hydration DEX. This is the route _discovery_ layer — it finds paths, not prices. Downstream consumers (ICE solver, on-chain router, RPC endpoints) use these routes to compute quotes and select the optimal path.

The existing `RouteProvider::get_route()` in `hydration-node` returns a **single** stored or default route. This crate fills the gap: discovering **every** viable route for a given asset pair.

---

## 2. Types & Dependencies

All pool routing types come from `hydradx-traits` and `primitives` — **no local duplicates**:

| Type                   | Source                   | Description                                        |
| ---------------------- | ------------------------ | -------------------------------------------------- |
| `AssetId`              | `primitives`             | Concrete asset identifier (`u32`)                  |
| `PoolType<AssetId>`    | `hydradx_traits::router` | Pool type discriminant                             |
| `Trade<AssetId>`       | `hydradx_traits::router` | Single trade step: `{ pool, asset_in, asset_out }` |
| `Route<AssetId>`       | `hydradx_traits::router` | `BoundedVec<Trade<AssetId>, ConstU32<9>>`          |
| `MAX_NUMBER_OF_TRADES` | `hydradx_traits::router` | `9` — max hops per route                           |

Types introduced by this crate:

| Type           | Description                                                                                |
| -------------- | ------------------------------------------------------------------------------------------ |
| `PoolEdge`     | Pool instance for graph building: `{ pool_type: PoolType<AssetId>, assets: Vec<AssetId> }` |
| `PoolProvider` | Trait with associated `State`: `fn get_all_pools(state: &Self::State) -> Vec<PoolEdge>`    |

### Pool identity via `PoolType<AssetId>`

`PoolType` is a discriminant, not a unique pool ID. The `<AssetId>` generic exists solely for `Stableswap(AssetId)` where the value is the pool's share token:

```rust
pub enum PoolType<AssetId> {
    XYK,                   // bare — resolved by (asset_in, asset_out) pair
    LBP,                   // bare — resolved by asset pair
    Stableswap(AssetId),   // unique per pool instance
    Omnipool,              // singleton
    Aave,                  // bare
    HSM,                   // bare
}
```

The on-chain `pallet-route-executor` resolves the concrete pool from `Trade { pool, asset_in, asset_out }`. For cycle prevention during BFS, this crate uses an internal `pool_index` (position in the input `Vec<PoolEdge>`).

---

## 3. State & the `PoolProvider` Trait

### How `PoolProvider` fits in

`PoolProvider` mirrors this pattern — it accepts `&State` so route discovery uses the same snapshot:

```rust
pub trait PoolProvider {
    type State: Clone;
    fn get_all_pools(state: &Self::State) -> Vec<PoolEdge>;
}
```

`State` is an associated type because this crate **cannot know its shape**. The composed state is a tuple whose arity depends on which simulators the runtime configures (`(A, B, C)` vs `(A, B, C, D)`). Only the runtime can destructure it.

---

## 4. Architecture Overview

### Component Diagram

```
Consumer (ICE solver / RPC / pallet)
  │
  ▼
RouteSuggester<P: PoolProvider>         [lib.rs]
  │
  ├── strategy::suggest_routes()        [strategy.rs]
  │     │
  │     ├── Partition pools: trusted vs isolated
  │     ├── Select search strategy based on token placement
  │     │
  │     ├── graph::build_graph()        [graph.rs]
  │     │     └── Vec<PoolEdge> → AdjacencyMap (BTreeMap<AssetId, Vec<Edge>>)
  │     │
  │     └── bfs::find_all_paths()       [bfs.rs]
  │           └── BFS over adjacency map → Vec<Route<AssetId>>
  │
  └── P::get_all_pools(state)           [types.rs — trait, impl in runtime]
        └── Extracts tradeable assets from SimulatorSet::State snapshot
```

### Standalone Alternative

```rust
// When you already have the pool list — no PoolProvider/State needed
get_routes(asset_in, asset_out, pools) -> Vec<Route<AssetId>>
```

---

## 5. Module Breakdown

### 5.1 `graph.rs` — Graph Construction

**Ported from:** `packages/sdk-next/src/sor/route/graph.ts` → `getNodesAndEdges()`

Converts `Vec<PoolEdge>` into a directed adjacency map.

**Edge generation:** For a pool with N assets, N×(N-1) directed edges are created — every asset can be swapped for every other asset within that pool.

| Pool type                    | Graph behavior                            |
| ---------------------------- | ----------------------------------------- |
| Omnipool (40 assets)         | 1 pool_index, 40×39 = 1560 directed edges |
| Stableswap(100) with [A,B,C] | 1 pool_index, 3×2 = 6 directed edges      |
| XYK with [A,B]               | 1 pool_index, 2 directed edges            |

**Internal types (crate-private):**

```rust
struct Edge {
    pool_index: usize,            // position in input Vec — for cycle prevention
    pool_type: PoolType<AssetId>, // flows into Trade output
    asset_out: AssetId,
}

type AdjacencyMap = BTreeMap<AssetId, Vec<Edge>>;
```

### 5.2 `bfs.rs` — Breadth-First Search

**Ported from:** `packages/sdk-next/src/sor/route/bfs.ts` → `Bfs` class

Finds all acyclic paths up to `MAX_NUMBER_OF_TRADES` (9) hops. Returns `Vec<Route<AssetId>>` — directly usable by `pallet-route-executor`.

**Cycle prevention** (mirrors SDK's `Bfs.isNotVisited`):

1. **Asset revisit** — destination asset already in current path → rejected
2. **Pool reuse** — pool_index already in current path → rejected

This prevents circular routes (A → B → A) and redundant multi-hop through the same pool (Omnipool A→B→C when A→C is direct).

**Termination guarantees:**

- Max path length: 9 hops
- No cycles: asset + pool visited checks
- Finite pool set → finite graph → BFS terminates

### 5.3 `strategy.rs` — Pool Partitioning Strategy

**Ported from:** `packages/sdk-next/src/sor/route/suggester.ts` → `RouteSuggester.getProposals()`

Pools are partitioned:

| Category     | Pool types                           | Rationale                          |
| ------------ | ------------------------------------ | ---------------------------------- |
| **Trusted**  | Omnipool, Stableswap, LBP, Aave, HSM | Deeper liquidity, protocol-managed |
| **Isolated** | XYK                                  | Permissionless, lower liquidity    |

Strategy selection:

| `asset_in` trusted? | `asset_out` trusted? | Search over                                           |
| ------------------- | -------------------- | ----------------------------------------------------- |
| No                  | No                   | XYK pools containing `asset_in` OR `asset_out`        |
| Yes                 | Yes                  | All trusted pools                                     |
| Mixed               | Mixed                | All trusted + XYK pools containing the isolated asset |

### 5.4 `lib.rs` — Public API

```rust
// Trait-based: pool list from PoolProvider + state snapshot
RouteSuggester::<P>::get_routes(asset_in, asset_out, &state) -> Vec<Route<AssetId>>

// Standalone: pool list provided directly
get_routes(asset_in, asset_out, pools) -> Vec<Route<AssetId>>
```

---

## 6. Data Flow

### `RouteSuggester::<AllPools>::get_routes(A, B, &state)`

```
1. P::get_all_pools(&state) → Vec<PoolEdge>
   │  (runtime destructures SimulatorSet::State,
   │   extracts tradeable asset lists from each AMM snapshot)
   │
2. strategy::suggest_routes(A, B, pools)
   │
   ├── Partition: trusted[], isolated[]
   ├── Check: A in trusted? B in trusted?
   ├── Select pool subset
   │
   ├── graph::build_graph(selected_pools) → AdjacencyMap
   │
   └── bfs::find_all_paths(adjacency, A, B)
       │
       ├── Queue ← [PathNode { asset: A }]
       │
       └── While queue not empty:
           │ path = queue.pop_front()
           ├── path.last == B? → results.push(path_to_route(path)); continue
           ├── trade_count > 9? → continue
           └── For each edge from path.last.asset:
               ├── is_valid_extension? (no asset revisit, no pool reuse)
               └── Yes → queue.push(path + edge)

3. Return: Vec<Route<AssetId>>
   (BoundedVec<Trade<AssetId>, ConstU32<9>> — directly compatible
    with pallet-route-executor and AMMInterface::sell/buy)
```

---

## 7. Type Mapping: SDK → Rust

| SDK (TypeScript)                | Rust (this crate)                                  | Notes                                         |
| ------------------------------- | -------------------------------------------------- | --------------------------------------------- |
| `PoolBase`                      | `PoolEdge`                                         | Simplified: only pool_type + assets needed    |
| `PoolType` enum                 | `PoolType<AssetId>`                                | From `hydradx_traits::router`                 |
| `Edge = [address, from, to]`    | `graph::Edge { pool_index, pool_type, asset_out }` | address → pool_index for cycle checks         |
| `Node = [id, from]`             | `bfs::PathNode { asset, pool_index, pool_type }`   | Carries metadata for cycle prevention         |
| `RouteProposal = Edge[]`        | `Route<AssetId>`                                   | `BoundedVec<Trade, ConstU32<9>>`              |
| `Bfs.isNotVisited()`            | `bfs::is_valid_extension()`                        | Same dual check: asset + pool                 |
| `Bfs.findPaths()`               | `bfs::find_all_paths()`                            | Queue-based BFS                               |
| `getNodesAndEdges()`            | `graph::build_graph()`                             | Pool → adjacency map                          |
| `RouteSuggester.getProposals()` | `strategy::suggest_routes()`                       | 3-case strategy dispatch                      |
| `Queue<T>`                      | `VecDeque<T>`                                      | stdlib FIFO queue                             |
| `MAX_SIZE_OF_PATH = 10`         | `MAX_NUMBER_OF_TRADES = 9`                         | SDK counts nodes (10), Rust counts trades (9) |

---

## 8. Constraints & Invariants

### Route constraints

| Constraint           | Enforced by               | Value                       |
| -------------------- | ------------------------- | --------------------------- |
| Max trades per route | `bfs::find_all_paths`     | 9 (`MAX_NUMBER_OF_TRADES`)  |
| No asset revisits    | `bfs::is_valid_extension` | Checked against full path   |
| No pool reuse        | `bfs::is_valid_extension` | Tracked by `pool_index`     |
| Output is `Route`    | `bfs::path_to_route`      | `BoundedVec::truncate_from` |

### `no_std` compatibility

| Concern      | Solution                                                                     |
| ------------ | ---------------------------------------------------------------------------- |
| Collections  | `BTreeMap` / `VecDeque` from `alloc`                                         |
| Feature gate | `#![cfg_attr(not(feature = "std"), no_std)]`                                 |
| Dependencies | `hydradx-traits`, `primitives`, `sp-runtime`, `frame-support` (all `no_std`) |

---

## 9. Complexity Analysis

For P pools with at most A assets each:

- **Graph construction:** O(P × A²)
- **BFS:** O(V × E × L) worst case — V = unique assets, E = total edges, L = 9. Cycle prevention prunes aggressively.
- **Strategy partitioning:** O(P)

**Practical bounds (Hydration mainnet):**

- Omnipool: ~40-60 assets → 1 pool, ~2500 edges
- Stableswap: ~5-10 pools, 2-4 assets → ~30-80 edges
- XYK: ~20-50 pairs → ~40-100 edges

Total graph is small. BFS completes in microseconds for typical queries.

---

## 10. Integration Guide (hydration-node)

### Step 1: Add dependency

In the target crate within [`hydration-node`](https://github.com/galacticcouncil/hydration-node):

```toml
[dependencies]
route-suggester = { git = "https://github.com/galacticcouncil/sdk", subdirectory = "crates/route-suggester", default-features = false }

[features]
std = ["route-suggester/std"]
```

### Step 2: Implement `PoolProvider`

The implementation lives in the runtime because only the runtime knows the concrete `SimulatorSet::State` shape. It destructures the state and extracts tradeable assets from each AMM's snapshot:

```rust
use route_suggester::types::{PoolEdge, PoolProvider};
use hydradx_traits::router::PoolType;
use hydradx_traits::amm::SimulatorSet;
use primitives::AssetId;

pub struct AllPools;

impl PoolProvider for AllPools {
    // Same State as SimulatorSet — composed tuple of all AMM snapshots
    type State = <Simulators as SimulatorSet>::State;

    fn get_all_pools(state: &Self::State) -> Vec<PoolEdge> {
        let (omni_state, stable_state, xyk_state) = state;
        let mut pools = Vec::new();

        // Omnipool — single pool, all tradeable assets
        let omni_assets: Vec<AssetId> = omni_state
            .iter()
            .filter(|(_, s)| s.tradeable.contains(Tradability::SELL | Tradability::BUY))
            .map(|(id, _)| *id)
            .collect();
        if !omni_assets.is_empty() {
            pools.push(PoolEdge {
                pool_type: PoolType::Omnipool,
                assets: omni_assets,
            });
        }

        // Stableswap — one PoolEdge per pool
        for pool in stable_state {
            pools.push(PoolEdge {
                pool_type: PoolType::Stableswap(pool.pool_id),
                assets: pool.assets.clone(),
            });
        }

        // XYK — one PoolEdge per pair
        for pool in xyk_state {
            pools.push(PoolEdge {
                pool_type: PoolType::XYK,
                assets: vec![pool.asset_a, pool.asset_b],
            });
        }

        pools
    }
}
```

### Step 3: Use with the ICE solver

Within the `AMMInterface` implementation, use `RouteSuggester` for route discovery when no route is provided:

```rust
use route_suggester::RouteSuggester;

type RouteFinder = RouteSuggester<AllPools>;

// Inside AMMInterface::sell implementation:
fn sell(
    asset_in: AssetId,
    asset_out: AssetId,
    amount_in: Balance,
    route: Option<Route<AssetId>>,
    state: &Self::State,
) -> Result<(Self::State, TradeExecution), Self::Error> {
    let route = match route {
        Some(r) => r,
        None => {
            // Discover all viable routes from the current state
            let routes = RouteFinder::get_routes(asset_in, asset_out, state);
            // Pick the best one (e.g., simulate each, select highest output)
            select_best_route(routes, asset_in, amount_in, state)?
        }
    };

    execute_along_route(route, amount_in, state)
}
```

### Step 4: Standalone use (tests, RPC, off-chain workers)

```rust
use route_suggester::{get_routes, types::PoolEdge};
use hydradx_traits::router::PoolType;

let pools = vec![
    PoolEdge { pool_type: PoolType::Omnipool, assets: vec![0, 1, 2, 5, 10] },
    PoolEdge { pool_type: PoolType::Stableswap(100), assets: vec![10, 11, 12] },
    PoolEdge { pool_type: PoolType::XYK, assets: vec![20, 5] },
];

let routes = get_routes(20, 12, pools);
// Returns: [
//   Route [XYK 20→5, Omnipool 5→10, Stableswap(100) 10→12],
//   ... other viable paths
// ]
```

---

## 11. Test Coverage

20 tests, all passing:

| Category             | Tests | What's verified                                       |
| -------------------- | ----- | ----------------------------------------------------- |
| Basic routing        | 5     | Direct, reverse, multi-hop, multiple routes, no route |
| Edge cases           | 2     | Same asset, empty pools                               |
| Omnipool             | 2     | Direct route, no multi-hop through same pool          |
| Stableswap           | 1     | Direct route with pool ID                             |
| Cross-pool           | 2     | XYK→Omnipool bridge, Stableswap→Omnipool chain        |
| Strategy             | 2     | Trusted-only excludes XYK, isolated-only filtering    |
| Cycle prevention     | 2     | No asset revisit in triangle, different pools OK      |
| Max trades           | 2     | Exactly 9 hops succeeds, 10 hops returns empty        |
| PoolProvider + State | 1     | End-to-end with trait-based provider and `&state`     |

---

## 12. File Reference

| File              | Purpose                                                                    |
| ----------------- | -------------------------------------------------------------------------- |
| `Cargo.toml`      | Deps: `hydradx-traits`, `primitives`, `sp-runtime`, `frame-support`        |
| `src/lib.rs`      | Public API (`RouteSuggester`, `get_routes`) + all tests                    |
| `src/types.rs`    | Re-exports from `hydradx-traits`/`primitives` + `PoolEdge`, `PoolProvider` |
| `src/graph.rs`    | `build_graph()` → `AdjacencyMap`                                           |
| `src/bfs.rs`      | `find_all_paths()`, cycle checks                                           |
| `src/strategy.rs` | Trusted/isolated partitioning, 3-case dispatch                             |
