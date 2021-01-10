# Exchange

Exchange pallet implements in-block order-matching algorithm

### Overview

Trades coming through exchange pallet are matched in a block which leads to lower slippage per trade.

If we sum all AMM transactions of one pair in one block, we need to only trade resulting sum through the AMM module. The current spot price of the pair is used. Other transactions can be traded directly from account to account. Fee will still be sent to the pool as Liquidity provisioning reward. Completely matched transactions will have 0 slippage (excluding fee). This pallet is pool design agnostic. If the AMM pallet implements required API, it can be connected.
