# pallet-xcm-rate-limiter

### XCM Rate Limiter Pallet

#### Overview

This pallet provides an implementation of `XcmDeferFilter` that tracks incoming tokens and defers iff they exceed
the rate limit configured in `RateLimitFor`.

#### Integration

The `RateLimitFor` associated type is supposed to be provided by the `AssetRegistry`, but could work with any other
implementation.

This pallet does not provide any extrinsics of its own, but it is meant to provide the implementation of
`XcmDeferFilter` for the `XcmpQueue`.

#### Implementation

The defer duration for an XCM is the maximum of the individual durations for its included assets.

The duration for deferring an asset is calculated based on:
- the incoming amount
- the rate limit of the asset
- the configured `DeferDuration`
- the amounts of tokens accumulated over time but decayed based on time and rate limit

The tokens are deferred once the rate limit is exceeded, with 2 times the rate limit corresponding to deferred
duration. For example, if the rate limit is 1000 tokens per 10 blocks, then 1500 tokens will be deferred by 5
blocks.

The accumulated amounts decay linearly at the rate limit. For example: With rate limit 1000 tokens per 10 blocks,
the accumulated amount will be reduced by 100 tokens per block.

The filter works with XCM v3 and so assumes that other versions can be converted to it.

The filter processes only the first instruction of the XCM message, because that is how assets will arrive on chain.
This is guaranteed by `AllowTopLevelExecution` which is standard in the ecosystem.

#### Limitations

- The filter assumes that it is fine to ignore (neither track nor limit) tokens that don't have a defined local id
  or don't have a configured rate limit.
- It counts accumulated amounts via `MultiLocation`s of incoming messages without reanchoring or canonicalizing.
- It only tracks and limits incoming tokens, not outgoing.
- Only tracks and limits `ReserveAssetDeposited` and `ReceiveTeleportedAsset`, meaning that core asset tokens
  "returning" from other chains are not tracked or limited.

License: Apache-2.0
