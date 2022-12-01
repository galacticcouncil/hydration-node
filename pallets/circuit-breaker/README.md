### Circuit Breaker pallet

This pallet is used to keep track initial liquidity of assets (of an AMM), 
then to validate the next liquidity volume per block per asset,

Before using validation, the initial liquidity for an asset must be registered with the pallet, otherwise it results in error.