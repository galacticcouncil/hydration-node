### Route executor

## Overview
This pallet is responsible for executing a series of trades specified in the route.
The specific price calculations and execution logics are implemented by the AMM pools
configured for the pallet.

Both buy and sell trades are supported. 

The extrinsic weights are calculated based on the size of the route.
