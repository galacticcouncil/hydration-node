# DCA pallet

## Overview
A dollar-cost averaging pallet that enables users to perform repeating orders.

When an order is submitted, it will reserve the total amount (budget) specified by the user.
A named reserve is allocated for the reserved amount of all DCA held by each user.

The DCA plan is executed as long as there is balance in the budget.

If a trade fails then the oder is suspended and has to be resumed or terminated by the user.

Orders are executed on block initialize and they are sorted based on randomness derived from relay chain block number. 
Therefore they cannot be front-ran in the block they are executed.
