
## Transaction multi payment

### Overview

This pallet provides functionality to accepted transaction fees in other currencies.

Extends substrate's `transaction-payment` pallet.

### Interface
Extends `transaction-payment` interface to add functionality to set desired currency and to add members who can add or remove accepted currencies.

- `set_currency` - set selected currency in whci all transactions fees will be paid. Balance of selected currency must be non-zero.
- `add_member` - only root can perform this action
- `remove_member` - only root can perform this action

### Implementation details

Transaction fees are paid in native currency by default. This pallet allows to set a different currency to pay fees with for an account. 

When the transaction fees is being paid and chosen currency is not native currency - swap is executed to obtain fee amount in native currency first.

The swap (or buy) is done via selected AMM pool.

Subsequently, the fee is paid in native currency.


