# Dynamic EVM Fee

## Overview

The goal of this pallet to have EVM transaction fees in tandem with Substrate fees.

This pallet enables dynamic adjustment of the EVM transaction fee, leveraging two primary metrics:
- Current network congestion
- The oracle price difference between ETH and HDX

Fees are calculated with the production of each new block, ensuring responsiveness to changing network conditions.

### Fee Adjustment Based on Network Congestion
The formula for adjusting fees in response to network congestion is as follows:
```
BaseFeePerGas = DefaultBaseFeePerGas + (DefaultBaseFeePerGas * Multiplier * 3)
```
- `DefaultBaseFeePerGas`: This represents the minimum fee payable for a transaction, set in pallet configuration.
- `Multiplier`: Derived from current network congestion levels, this multiplier is computed within the `pallet-transaction-payment`.

### Fee Adjustment Based on ETH-HDX Price Fluctuations

The transaction fee is also adjusted in accordance with in ETH-HDX oracle price change:
- When HDX increases in value against ETH, the fee is reduced accordingly.
- When HDX decreases in value against ETH, the fee is increased accordingly.

This dual-criteria approach ensures that transaction fees remain fair and reflective of both market conditions and network demand.