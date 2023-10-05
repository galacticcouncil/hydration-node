# DCA Pallet

## Overview

The DCA pallet provides dollar-cost averaging functionality, allowing users to perform repeating orders. 
This pallet enables the creation, execution and termination of schedules.

## Creating a Schedule

Users can create a DCA schedule, which is planned to execute in a specific block. 
If the block is not specified, the execution is planned for the next block. 
In case the given block is full, the execution will be scheduled for the subsequent block.

Upon creating a schedule, the user specifies a budget (`total_amount`) that will be reserved. 
The currency of this reservation is the sold (`amount_in`) currency.

### Executing a Schedule

Orders are executed during block initialization and are sorted based on randomness derived from the relay chain block hash.

A trade is executed and replanned as long as there is remaining budget from the initial allocation.

For both successful and failed trades, a fee is deducted from the schedule owner. 
The fee is deducted in the sold (`amount_in`) currency.

A trade can fail due to two main reasons:

1. Price Stability Error: If the price difference between the short oracle price and the last block oracle price 
exceeds the specified threshold. The user can customize this threshold, 
or the default value from the pallet configuration will be used.
2. Slippage Error: If the minimum amount out (sell) or maximum amount in (buy) slippage limits are not reached. 
These limits are calculated based on the last block's oracle price and the user-specified slippage. 
If no slippage is specified, the default value from the pallet configuration will be used.

If a trade fails due to these errors, the trade will be retried. 
If the number of retries reaches the maximum number of retries, the schedule will be permanently terminated. 
In the case of a successful trade, the retry counter is reset.

If a trade fails due to other types of errors, the order is terminated without any retry logic.

## Terminating a Schedule

Both users and technical origin can terminate a DCA schedule. However, users can only terminate schedules that they own.

Once a schedule is terminated, it is completely and permanently removed from the blockchain.