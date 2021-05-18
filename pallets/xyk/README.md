### AMM XYK pallet

## Overview
AMM pallet provides functionality for managing liquidity pool and executing trades.

This pallet implements AMM Api trait therefore it is possible to plug this pool implementation
into the exchange pallet.

### Terminology

- **Currency** - implementation of fungible multi-currency system
- **AssetPairAccount** / **AssetPairAccountId** - support for creating share accounts for asset pairs.
- **NativeAssetId** - asset id native currency
- **ShareToken** - asset id from asset registry for an asset pair
- **TotalLiquidity** - total liquidity in a pool identified by asset pair account id
- **PoolAssets** - asset pair in a pool identified by asset pair account id

### Interface

#### Dispatchable functions
- `create_pool`
- `add_liquidity`
- `remove_liquidity`
- `sell`
- `buy`
