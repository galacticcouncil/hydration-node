### Circuit Breaker pallet

By using this pallet, we can track and limit the percentage of the liquidity of a pool that can be traded (net volume), added and removed in a block.

Three different limits are tracked independently for all assets: trading limit, liquidity added, and liquidity removed.

All trading volumes and amounts of liquidity are reset to zero at the end of block execution, so no values are actually stored in the database.

The default percentage limits are set for all assets in the pallet config.
To set a specific trade limit for a given asset, the `set_trade_volume_limit` extrinsic can be executed by `TechnicalOrigin`.
To set a specific limit for liquidity that can be added for a given asset, the `set_liquidity_limit` extrinsic can be executed by `TechnicalOrigin`.
