# pallet-stableswap

## Stableswap pallet

Curve/stableswap AMM implementation.

#### Terminology

* **LP** - liquidity provider
* **Share Token** - a token representing share asset of specific pool. Each pool has its own share token.
* **Amplification** - curve AMM pool amplification parameter

### Assumptions

Maximum number of assets in pool is 5.

A pool can be created only by allowed `CreatePoolOrigin`.

First LP to provided liquidity must add initial liquidity of all pool assets. Subsequent calls to add_liquidity, LP can provide only 1 asset.

Initial liquidity is first liquidity added to the pool (that is first call of `add_liquidity`).

LP is given certain amount of shares by minting a pool's share token.

When LP decides to withdraw liquidity, it receives selected asset.


License: Apache 2.0
