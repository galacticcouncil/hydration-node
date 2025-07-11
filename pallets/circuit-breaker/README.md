### Circuit Breaker pallet

By using this pallet, we can track and limit the percentage of the liquidity of a pool that can be traded (net volume),
added and removed in a block.

Three different limits are tracked independently for all assets: trading limit, liquidity added, and liquidity removed.

All trading volumes and amounts of liquidity are reset to zero at the end of block execution, so no values are actually
stored in the database.

The default percentage limits are set for all assets in the pallet config.
To set a specific trade limit for a given asset, the `set_trade_volume_limit` extrinsic can be executed by
`UpdateLimitsOrigin`.
To set a specific limit for liquidity that can be added for a given asset, the `set_liquidity_limit` extrinsic can be
executed by `UpdateLimitsOrigin`.

### Issuance and Deposit Lockdown

The pallet also provides a mechanism to limit asset deposits based on total issuance. 
This is achieved by implementing the `OnDeposit` hook, which is triggered whenever new assets are minted by orml tokens pallet.

The core logic is as follows:
- The pallet tracks the total issuance of an asset over a configurable period.
- If the issuance increase within that period exceeds a specified limit, the asset is put into lockdown for a configured duration.
- When the limit is breached, the amount of the deposit that exceeded the limit is reserved on the depositor's account.
- While an asset is in lockdown, further deposits are not permitted, and funds will be reserved further
- Once the lockdown period has expired, anyone is able to reclaim reserved funds on behalf of the user, by calling the `save_deposit` extrinsic.

Additionally, an authorized origin has the ability to manage lockdowns manually:
- `lockdown_asset`: This extrinsic allows an authorized account to manually place an asset into lockdown.
- `force_lift_lockdown`: This extrinsic allows an authorized account to remove an asset from lockdown.