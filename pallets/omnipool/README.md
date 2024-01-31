# pallet-omnipool

## Omnipool pallet

Omnipool implementation

### Overview

Omnipool is type of AMM where all assets are pooled together into one single pool.

Each asset is internally paired with so called Hub Asset ( LRNA ). When a liquidity is provided, corresponding
amount of hub asset is minted. When a liquidity is removed, corresponding amount of hub asset is burned.

Liquidity provider can provide any asset of their choice to the Omnipool and in return
they will receive pool shares for this single asset.

The position is represented as a NFT token which stores the amount of shares distributed
and the price of the asset at the time of provision.

For traders this means that they can benefit from non-fragmented liquidity.
They can send any token to the pool using the swap mechanism
and in return they will receive the token of their choice in the appropriate quantity.

Omnipool is implemented with concrete Balance type: u128.

#### Imbalance mechanism
The Imbalance mechanism is designed to stabilize the value of LRNA. By design it is a weak and passive mechanism,
and is specifically meant to deal with one cause of LRNA volatility: LRNA being sold back to the pool.

Imbalance is always negative, internally represented by a special type `SimpleImbalance` which uses unsigned integer and boolean flag.
This was done initially because of the intention that in future imbalance can also become positive.

#### Omnipool Hooks

Omnipool pallet supports multiple hooks which are triggerred on certain operations:
- on_liquidity_changed - called when liquidity is added or removed from the pool
- on_trade - called when trade is executed
- on_trade_fee - called after successful trade with fee amount that can be taken out of the pool if needed.

This is currently used to update on-chain oracle and in the circuit breaker.

### Terminology

* **LP:**  liquidity provider
* **Position:**  a moment when LP added liquidity to the pool. It stores amount,shares and price at the time
 of provision
* **Hub Asset:** dedicated 'hub' token for trade executions (LRNA)
* **Native Asset:** governance token
* **Imbalance:** Tracking of hub asset imbalance.

### Assumptions

Below are assumptions that must be held when using this pallet.

* Initial liquidity of new token being added to Omnipool must be transferred manually to pool account prior to calling add_token.
* All tokens added to the pool must be first registered in Asset Registry.

### Interface

#### Dispatchable Functions

* `add_token` - Adds token to the pool. Initial liquidity must be transffered to pool account prior to calling add_token.
* `add_liquidity` - Adds liquidity of selected asset to the pool. Mints corresponding position NFT.
* `remove_liquidity` - Removes liquidity of selected position from the pool. Partial withdrawals are allowed.
* `sell` - Trades an asset in for asset out by selling given amount of asset in.
* `buy` - Trades an asset in for asset out by buying given amount of asset out.
* `set_asset_tradable_state` - Updates asset's tradable state with new flags. This allows/forbids asset operation such SELL,BUY,ADD or  REMOVE liquidtityy.
* `refund_refused_asset` - Refunds the initial liquidity amount sent to pool account prior to add_token if the token has been refused to be added.
* `sacrifice_position` - Destroys a position and position's shares become protocol's shares.
* `withdraw_protocol_liquidity` - Withdraws protocol's liquidity from the pool. Used to withdraw liquidity from sacrificed position.

License: Apache-2.0
