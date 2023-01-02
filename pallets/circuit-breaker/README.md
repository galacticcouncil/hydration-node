### Circuit Breaker pallet

By using this pallet, we can limit the percentage of the liquidity of a pool that can be traded (net volume) or added in a block.

Two different limits are tracked independently for all assets: trading limit and liquidity added.
The limits are updated by calling a corresponding handler.

All trading volumes and amounts of provided liquidity are reset to zero at the end of block execution, so no values are actually stored in the database.

The default percentage limits are set for all assets in the pallet config.
To set a specific trade limit for a given asset, the `set_trade_volume_limit` extrinsic can be executed by `TechnicalOrigin`.
To set a specific limit for liquidity that can be added for a given asset, the `set_liquidity_limit` extrinsic can be executed by `TechnicalOrigin`.
