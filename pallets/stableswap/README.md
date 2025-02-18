# Stableswap pallet

Curve/stableswap AMM implementation.

## Overview

Curve style AMM at is designed to provide highly efficient and low-slippage trades for stablecoins.

### Drifting peg
It is possible to create a pool with so called drifting peg.
Source of target peg for each asset must be provided. Either constant value or external oracle.
First asset in the pool is considered as a base asset and all other assets are pegged to it. Therefore peg of the first asset must be 1.

### Stableswap Hooks

Stableswap pallet supports multiple hooks which are triggerred on certain operations:
- on_liquidity_changed - called when liquidity is added or removed from the pool
- on_trade - called when trade is executed

This is currently used to update on-chain oracle.

### Terminology

* **LP** - liquidity provider
* **Share Token** - a token representing share asset of specific pool. Each pool has its own share token.
* **Amplification** - curve AMM pool amplification parameter

## Assumptions

Maximum number of assets in pool is 5 (`MAX_ASSETS_IN_POOL` constant).

A pool can be created only by allowed `AuthorityOrigin`.

First LP to provide liquidity must add initial liquidity of all pool assets. Subsequent calls to add_liquidity, LP can provide only 1 asset.

Initial liquidity is first liquidity added to the pool (that is first call of `add_liquidity`).

LP is given certain amount of shares by minting a pool's share token.

When LP decides to withdraw liquidity, it receives selected asset or all assets proportionality.
