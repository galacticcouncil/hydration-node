# ICE Solver — How It Works

## What It Does

The ICE solver takes a batch of swap intents (users wanting to trade one asset for another) and figures out the best way to fulfill as many as possible. It does this by combining two strategies:

- **Direct matching**: pairing users who want opposite trades (Alice sells HDX for HOLLAR, Bob sells HOLLAR for HDX — they trade with each other, no pool needed)
- **AMM routing**: sending leftover volume through on-chain liquidity pools to complete the trades

The solver runs off-chain, produces a solution, and submits it to the chain for execution.

## Core Principle: Everyone Gets the Same Price

All users trading in the same direction get the same rate. If ten people are all selling HDX for HOLLAR, they all receive the same HOLLAR-per-HDX rate — regardless of how much slippage each individual was willing to accept.

This prevents exploitation. A user who sets a loose limit (willing to accept a bad rate) cannot be taken advantage of — they receive the same rate as everyone else, and any surplus above their minimum contributes to the solution score.

The two directions of a pair can have different rates. If HDX→HOLLAR sellers get 2000 HOLLAR per HDX, the HOLLAR→HDX sellers might get a slightly different rate. The gap between these rates is surplus captured from direct matching — value that would have been lost to pool slippage if everyone traded individually.

## The Algorithm

### 1. Filter Out Hopeless Intents

First, the solver checks each intent against current market prices. If selling 100 HDX can only get you 180 HOLLAR at the best available rate, but you're asking for 250 HOLLAR minimum — your intent is unsatisfiable and gets removed immediately. No point including it in the calculations.

This check uses two methods:
- **Route simulation**: actually simulate the trade through available pool routes to see what output you'd get
- **Spot price check**: compare against the best marginal price (the price you'd get for an infinitely small trade). If you can't meet the minimum even at this theoretical best, you definitely can't be satisfied

### 2. Discover Clearing Prices

The solver groups intents by asset pair and computes a **clearing price** — the rate at which all included intents can be fulfilled.

Here's the key insight: when multiple users sell the same asset through the same pool, they share the price impact. Ten users each selling 100 HDX is equivalent to one trade of 1000 HDX, which moves the price more than any individual trade would. The clearing price reflects this combined impact.

This creates a tension: including more intents means more volume, which means worse rates, which means some intents can't meet their minimums. The solver resolves this by iterating:

1. Start with all satisfiable intents
2. Compute the clearing price for the combined volume
3. Remove any intents whose minimum can't be met at this rate
4. Recompute with the smaller set — the rate improves since there's less volume
5. Repeat until stable — no more intents need to be removed

This converges quickly because each round can only remove intents, never add them.

**Important**: This batch rate is why an intent might be rejected even though it would succeed as an individual trade. Trading alone, your 10 USDT might get you 5.25 HDX. But when 15 other people are also selling USDT for HDX at the same time, the combined 150 USDT pushes the pool price down, and everyone only gets 3.69 HDX per 10 USDT. If your minimum was 5.47 HDX, you get filtered out.

### 3. Find Ring Trades

Before touching any pool, the solver looks for **ring trades** — cycles of three assets where users can trade peer-to-peer in a circle.

Example: Alice sells HDX for HOLLAR, Bob sells HOLLAR for BNC, Charlie sells BNC for HDX. These three can trade directly with each other — Alice's HDX goes to Charlie, Charlie's BNC goes to Bob, Bob's HOLLAR goes to Alice. No pool needed, no slippage, no fees.

The solver finds these 3-asset cycles, checks that all participants would get at least their minimum at spot prices, and fills them at the bottleneck volume (limited by the smallest leg of the cycle).

Ring trades are strictly better than pool trades — they capture maximum value from multi-asset matching.

### 4. Execute AMM Trades for the Remainder

After ring matching, there's usually leftover volume that can't be matched peer-to-peer. For each asset pair, the solver computes the **net imbalance** — how much more is being sold in one direction than the other — and routes that excess through the AMM.

The flow analysis for each pair works like this:

- **One-sided flow** (only sellers in one direction): the entire volume goes through the pool
- **Opposing flow with excess**: the smaller side is fully absorbed by direct matching (they get approximately spot rate — no slippage). Only the excess on the larger side goes to the pool
- **Perfect cancellation**: volumes match exactly, no pool trade needed at all

For each AMM trade, the solver discovers available routes (which may go through multiple pools), simulates each one, and picks the route with the best output. A small safety margin (0.01%) is subtracted from the simulated output to account for tiny differences between the simulator and the real pool math.

If the leftover amount is smaller than the asset's existential deposit (dust from near-perfect cancellation), the trade is skipped entirely.

### 5. Blend Rates and Resolve Intents

Now the solver has two sources of output for each direction: ring fills (peer-to-peer) and AMM output. It blends these into a single **unified rate** per direction:

> unified rate = (ring output + AMM output) / total input

Every intent in the same direction gets this same rate applied to their individual amount. This ensures fairness — no one gets a better deal just because their portion happened to be matched via a ring.

The solver then checks each intent: does the unified rate give them at least their minimum? If yes, they're resolved. If not, they're dropped.

### 6. Stabilization

Sometimes an intent passes the initial clearing price check (step 2) but fails after the actual rates are computed (step 5), because the rates differ slightly due to ring fill blending, rounding, and the safety margin.

If any intents are dropped during resolution, their volume was already baked into the AMM trades. The solver handles this by re-running steps 3–5 with only the actually-resolved intents. This produces new trades that match the real volumes, and new rates that might allow different intents to resolve.

This loop repeats until stable — no intents drop during resolution, meaning the trades perfectly match the resolved set.

### 7. Compute Score

The **score** is the total surplus across all resolved intents: how much more each user receives compared to their stated minimum. Higher score means more value delivered to users.

## On-Chain Execution

The solution is submitted as an unsigned transaction. The pallet executes it in three phases:

1. **Unlock and collect**: For each resolved intent, unreserve the user's tokens (locked when they submitted the intent) and transfer them to the ICE holding account

2. **Execute AMM trades**: Run each pool trade from the holding account. The holding account now has a mix of assets from step 1 and AMM outputs from this step

3. **Pay out**: For each resolved intent, deduct the protocol fee from their output amount and transfer the remainder from the holding account to the user. Verify that all intents in the same direction received the same rate, and that the score matches

## Summary of Matching Strategies

| Strategy | How it works | Slippage | When used |
|----------|-------------|----------|-----------|
| **Direct matching** | Opposing intents trade with each other | None | When both directions have volume in a pair |
| **Ring matching** | 3-asset cycles trade peer-to-peer | None | When A→B, B→C, C→A intents all exist |
| **AMM routing** | Excess volume goes through liquidity pools | Yes — shared across all same-direction intents | For net imbalance after matching |

## Known Limitations

**Batch slippage can exclude viable intents.** All intents in the same direction share the AMM slippage from their combined volume. An intent that would succeed as an individual trade may be rejected because the batch rate is worse. The solver does not currently optimize which subset to include to maximize the number of resolved intents.

**No partial fills.** An intent is either fully resolved or fully excluded. An intent that could be 90% filled at a good rate is excluded entirely rather than being partially satisfied.

**Only 3-asset rings.** Longer cycles (4+ assets) are not detected. Some multi-asset matching opportunities are missed.

**Single-pass route selection.** The solver picks the best route per pair independently. It does not consider how one trade's impact on pool state affects the available routes for other pairs.

**Simulation tolerance.** A 0.01% safety margin covers typical rounding differences between the off-chain simulator and on-chain pool math. Larger divergences (from pool state changes between simulation and execution) could cause the on-chain execution to fail.
