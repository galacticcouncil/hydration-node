# Route executor

## Overview

### Storing routes
This pallet is responsible for storing the best routes for asset pairs. 

The new route is validated by being executed it in a dry-run mode

If there is no route explicitly set for an asset pair, then we use the omnipool route as default.

When a new route is set, we compare it to the existing (or default) route.
The comparison happens by calculating sell amount_outs for the routes, but also for the inversed routes.

The route is stored in an ordered manner, based on the oder of the ids in the asset pair.

If the route is set successfully, then the fee is payed back.

If the route setting fails, it emits event `RouteUpdateIsNotSuccessful`

### Providing routes
This pallet is also responsible for providing the best routes for asset pairs.

If no on-chain route present, then omnipool route is provided as default.

### Executing routes
This pallet is also responsible for executing a series of trades specified in the route.
The specific price calculations and execution logics are implemented by the AMM pools
configured for the pallet.

If no route is specified for a route execution, then the on-chain route is used.
If not on-chain is present, then omnipool is used as default

Both buy and sell trades are supported. 

### Weight calculation
The extrinsic weights are calculated based on the size of the route.
