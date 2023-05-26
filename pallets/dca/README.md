# DCA pallet

## Overview

A dollar-cost averaging pallet that enables users to perform repeating orders.

When an order is submitted, it will reserve the total amount (budget) specified by the user.

A named reserve is allocated for the reserved amount of all DCA held by each user.

The DCA plan is executed as long as there is remaining balance in the budget.

If a trade fails due to specific errors whitelisted in the pallet config, 
then retry happens up to the maximum number of retries specified also as config. 
Once the max number of retries reached, the order is terminated permanently.

If a trade fails due to other kind of errors, the order is terminated permanently without any retry logic.

Orders are executed on block initialize and they are sorted based on randomness derived from relay chain block number.
