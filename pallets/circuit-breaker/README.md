### Circuit Breaker pallet

By using this pallet, we can limit the percentage of the liquidity of a pool, to be traded or to be moved per block.

The default percentage limit is set by a configuration for all assets.
To set a specific limit for a given asset, the `set_trade_volume_limit` extrinsic can be executed by technical committee.