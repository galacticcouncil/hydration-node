# OTC Settlements pallet
## Description
The pallet provides implementation of the offchain worker for closing existing arbitrage opportunities between OTC 
orders and the Omnipool.
Two main parts of this pallet are methods to find the correct amount in order to close an existing arbitrage opportunity 
and an extrinsic. The extrinsic is mainly called by the offchain worker as unsigned extrinsic, but can be also called 
by any user using signed origin. In the former case, the block producer doesn't pay the fee.

## Notes
If the OTC order is partially fillable, the pallet tries to close the arbitrage opportunity by finding the amount that 
aligns the OTC and the Omnipool prices. Executing this trade needs to be profitable, but we are not trying to maximize 
the profit.
In the case of not partially fillable OTC orders, the pallet tries to maximize the profit.

## Dispatachable functions
* `settle_otc_order` -  Executes a trade between an OTC order and some route.