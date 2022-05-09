# Omnipool pallet

Omnipool implementation

## Overview

Omnipool is type of AMM where all assets are pooled together into one single pool.

Liquidity provider can provide any aset of their choice to the Omnipool and in return 
they will receive pool shares for this single asset.

The position is represented with a NFT token which saves the amount of shares distributed 
and the price of the asset at the time of provision.

For traders this means that tehy can benefit from the fill asset position
which can be used for trades with all other assets - there is no fragmented liquidity.
They can send any token to the pool using the swap mechanism 
and in return they will receive the token of their choice in the appropriate quantity.

Omnipool is implemented with concrete Balance type: u128.

### Terminology

* **LP:**  liquidity provider
* **Position:**  a moment when LP added liquidity to the pool. It stores amount,shares and price at the time
 of provision
* **Hub Asset:** dedicated 'hub' token for trade executions (LRNA)
* **Native Asset:** governance token

## Assumptions

Below are assumptions that must be held when using this pallet.

* First two asset in pool must be Stable Asset and Native Asset. This must be achieved by calling
  `initialize_pool` dispatchable.
* Stable asset balance and native asset balance must be transffered to omnipool account manually.
* All tokens added to the pool must be first registered in Asset Registry.

## Interface

### Dispatchable Functions

* `initialize_pool` - Initializes Omnipool with Stable and Native assets. This must be executed first.
* `set_asset_tradable_state` - Updates state of an asset in the pool to allow/disallow trading.
* `add_token` - Adds token to the pool.
* `add_liquidity` - Adds liquidity of selected asset to the pool. Mints corresponding position NFT.
* `remove_liquidity` - Removes liquidity of selected position from the pool. Partial withdrawals are allowed.
* `sell` - Trades an asset in for asset out by selling given amount of asset in.
* `buy` - Trades an asset in for asset out by buying given amount of asset out.

License: Apache-2.0
