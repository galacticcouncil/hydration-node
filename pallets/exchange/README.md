# Exchange

Exchange pallet implements in-block order-matching algorithm

### Overview

Trades coming through exchange pallet are matched in a block which leads to lower slippage per trade.

If we sum all AMM transactions of one pair in one block, we need to only trade resulting sum through the AMM module. 
The current spot price of the pair is used. Other transactions can be traded directly from account to account. 
Fee will still be sent to the pool as Liquidity provisioning reward. Completely matched transactions will have 0 slippage (excluding fee). 
This pallet is pool design agnostic. If the AMM pallet implements required API, it can be connected.

### Implementation details

#### Dispatchable functions
- `buy` - Register buy intention  
- `sell` - Register sell intention 

#### Handling and storing intention 

Registering intention means storing the intention's info in substrate storage. All intentions within the current block are resolved prior to block finalization, 
therefore none is actually committed to the storage. 

#### Resolving Intention 

Intentions are resolved in `on_finalize`. 

Resolving an intention means trying to match one or more intentions following the order matching algorithm.
If one or more such intentions are matched - amounts can be traded directly between the corresponding accounts and resulting difference is then traded through AMM module.

### Order-matching algorithm

The algorithm works as follows:

Intentions are stored in two groups for each asset pair. For example , for asset_a and asset_b, there would be:

 - (asset_a, asset_b) - includes all intentions where asset_a is being sold
 - (asset_b, asset_a) - includes all intentions where asset_b is being sold 

During block finalization, these paired groups are processed, intentions matched and resolved in following steps:

1. Intentions in each group are sorted by sold amount
2. For each intention from the first group - `Intention_A` _( note: possible improvements can be done here as it always takes first group regardless of number of intentions, amounts etc...)_
    - Find and match as many as intentions from the second group such that `Intention_A.amount >= Sum(Intention_B.amount)`
3. As a result of 2, there is one intention `Intention_A` on one side and list of matched intentions `Intention_B` on the other side.
4. For each matched `Intention_B` - there might be 3 possible scenarios:
   - `Intention_A.amount left == Intention_B.amount`
        - Direct trade between intention A and B accounts
   - `Intention_A amount left > Intention_B.amount`
        -  B amount is traded directly between intentions A,B accounts
        -  `Intention_A.amount = Intention_A.amount - Intention_B.amount`
   - `Intention_A.amount left < Intention_B.amount` - _note: should not happen in the current implement, however such case is still handled as follows_:
      - Intention_A amount is traded directly between intention_b and intention_a accounts
      - `Intention_B.amount - Intention_B.amount` - difference is traded through AMM.
5. After all matched intentions are resolved, if there is anything left for intention A - it is traded through AMM.    
6. If there are any intentions left in the second group( have not been matched ) - all are traded through AMM.


##### Fees 

Fees are paid to the pool account for each direct trade - 0.2% of amount - by each intention's account involved in the direct trade. 
   




