# OTC pallet
## General description
This pallet provides basic over-the-counter (OTC) trading functionality.
It allows anyone to `place_order` by specifying a pair of assets (in and out), their respective amounts, and
whether the order is partially fillable. The order price is static and calculated as `amount_out / amount_in`.

## Notes
The pallet implements a minimum order size as an alternative to storage fees. The amounts of an open order cannot
be lower than the existential deposit for the respective asset, multiplied by `ExistentialDepositMultiplier`.
This is validated at `place_order` but also at `partial_fill_order` - meaning that a user cannot leave dust amounts
below the defined threshold after filling an order (instead they should fill the order completely).

## Dispatachable functions
* `place_order` -  create a new OTC order.
* `partial_fill_order` - fill an OTC order (partially).
* `fill_order` - fill an OTC order (completely).
* `cancel_order` - cancel an open OTC order.