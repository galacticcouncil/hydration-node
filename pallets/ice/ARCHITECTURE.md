# ICE Solver Architecture

## Overview

The ICE (Intent Componsing Engine) system enables intent-based trading on Hydration. Users submit trade intents, and an off-chain solver finds optimal execution paths, potentially matching intents directly to reduce AMM fees and slippage.

## Core Components

```mermaid
graph TB
    subgraph "On-Chain"
        Intent[Intent Pallet]
        ICE[ICE Pallet]
        Router[Router]
        Omnipool[Omnipool]
        Stableswap[Stableswap]
        Other[...]
    end

    subgraph "Off-Chain"
        Solver[Solver]
        Simulator[AMM Simulator]
    end

    User -->|submit_intent| Intent
    Intent -->|valid intents| ICE
    ICE -->|snapshot| Simulator
    Simulator -->|state| Solver
    Solver -->|solution| ICE
    ICE -->|execute trades| Router
    Router --> Omnipool
    Router --> Stableswap
    Router --> Other
```

## Component Responsibilities

| Component | Role |
|-----------|------|
| **Intent Pallet** | Stores user intents with deadlines and parameters |
| **ICE Pallet** | Orchestrates solving, validates and executes solutions |
| **Simulator** | Captures AMM state snapshots, simulates trades off-chain |
| **Solver** | Finds optimal intent resolution with matching algorithm |

## Traits and Integration

```mermaid
graph TB
    subgraph "Solver Layer"
        Solver[SolverV1]
    end

    subgraph "Interface Layer"
        AMM[AMMInterface]
    end

    subgraph "Compositor Layer"
        HS[HydrationSimulator]
        SC[SimulatorConfig]
    end

    subgraph "Simulator Layer"
        SS[SimulatorSet]
    end

    subgraph "AMM Simulators"
        OmniSim[Omnipool::AmmSimulator]
        StableSim[Stableswap::AmmSimulator]
    end

    Solver -->|"sell/buy/spot_price"| AMM
    AMM -.->|implements| HS
    HS -->|uses| SC
    SC -->|"type Simulators"| SS
    SC -->|"type RouteProvider"| Router[Router]
    SS -.->|"impl for (A,B)"| OmniSim
    SS -.->|"impl for (A,B)"| StableSim
```

## Trait Hierarchy

```mermaid
classDiagram
    class AMMInterface {
        +sell(asset_in, asset_out, amount, route, state)
        +buy(asset_in, asset_out, amount, route, state)
        +get_spot_price(asset_in, asset_out, state)
        +price_denominator()
    }

    class AmmSimulator {
        +pool_type()
        +matches_pool_type(pool_type)
        +snapshot()
        +simulate_sell(in, out, amount, min, snapshot)
        +simulate_buy(in, out, amount, max, snapshot)
        +get_spot_price(in, out, snapshot)
    }

    class SimulatorSet {
        +initial_state()
        +simulate_sell(pool_type, ...)
        +simulate_buy(pool_type, ...)
        +get_spot_price(pool_type, ...)
    }

    class SimulatorConfig {
        +Simulators: SimulatorSet
        +RouteProvider
        +PriceDenominator
    }

    AMMInterface <|.. HydrationSimulator : implements
    HydrationSimulator --> SimulatorConfig : uses
    SimulatorConfig --> SimulatorSet : type
    SimulatorSet <|.. Tuple : impl for A,B
    AmmSimulator <|.. Omnipool : implements
    AmmSimulator <|.. Stableswap : implements
```

## Solver Algorithm (Matching)

```mermaid
flowchart TD
    A[Get spot prices for all assets] --> B[Filter satisfiable intents]
    B --> C[Calculate net flows per asset]
    C --> D{Net surplus/deficit?}
    D -->|Surplus| E[Sell excess to AMM]
    D -->|Deficit| F[Buy needed from AMM]
    E --> G[Distribute at clearing price]
    F --> G
    G --> H[Return Solution]
```

**Matching Benefit:**
- Without matching: Each intent trades through AMM separately
- With matching: Matching intents settle directly, only net imbalance hits AMM
- Result: Lower fees, reduced slippage, better execution for all users

## Solution Structure

```rust
Solution {
    resolved_intents: Vec<ResolvedIntent>,  // What each user gets
    trades: Vec<PoolTrade>,                  // AMM trades to execute
    clearing_prices: Map<Asset, Price>,      // Uniform prices used
    score: u128,                             // Solution quality metric
}
```

## Key Design Decisions

1. **Snapshot-based Simulation** - Capture chain state once, simulate multiple scenarios off-chain
2. **Tuple-based SimulatorSet** - Compose multiple AMM simulators with automatic type-safe dispatch
3. **Router Integration** - Use on-chain router for route discovery, simulator for execution
4. **HDX as Price Denominator** - All prices computed relative to HDX for intent matching
5. **Uniform Clearing Price** - All matched intents execute at same price for fairness
