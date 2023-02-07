# DCA pallet

## Overview
//TODO: Dani - rewrite it as the stuff has been changed a lot
A dollar-cost averaging pallet that enables users to perform repeating orders.

When an order is submitted, it will reserve the amount for data storage and for the fee for the next trade. 
The fee is reserved for cases when the next order execution fails. 
Order is, in this case, suspended and has to be resumed by the user.

This allows users to submit orders that they don’t have enough balance to execute immediately and also perpetual orders.

Orders are executed on block initialize. Therefore they cannot be front-ran in the block they are executed.
