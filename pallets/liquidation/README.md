# Liquidation (Money market) pallet

## Description
The pallet uses mechanism similar to a flash loan to liquidate a MM position.

## Notes
The pallet requires the money market contract to be deployed and enabled.

## Dispatchable functions
* `liquidate` - Liquidates an existing MM position. Performs flash loan to get funds.