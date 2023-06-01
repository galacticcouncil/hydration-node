# pallet-ema-oracle

## EMA Oracle Pallet

### Overview

This pallet provides exponential moving average (EMA) oracles of different periods for price,
volume and liquidity for a combination of source and asset pair based on data coming in from
different sources.

#### Integration

Data is ingested by plugging the provided `OnActivityHandler` into callbacks provided by other
pallets (e.g. xyk pallet).

It is meant to be used by other pallets via the `AggregatedOracle` and `AggregatedPriceOracle`
traits.

When integrating with this pallet take care to use the `on_trade_weight`,
`on_liquidity_changed_weight` and `get_entry_weight` into account when calculating the weight
for your extrinsics (that either feed data into or take data from this pallet).

#### Concepts

- *EMA*: Averaging via exponential decay with a smoothing factor; meaning each new value to
  integrate into the average is multiplied with a smoothing factor between 0 and 1.
- *Smoothing Factor*: A factor applied to each value aggregated into the averaging oracle.
  Implicitly determines the oracle period.
- *Period*: The window over which an oracle is averaged. Certain smoothing factors correspond to
  an oracle period. E.g. ten minutes oracle period ≈ 0.0198
- *Source*: The source of the data. E.g. xyk pallet.

#### Implementation

This pallet aggregates data in the following way: `on_trade` or `on_liquidity_changed` a new
entry is created for the incoming data. This then updates any existing entries already present
in storage for this block (for this combination of source and assets) or inserts it. Note that
this aggregation is NOT based on EMA, yet, it just sums the volume and replaces price and
liquidity with the most recent value.

At the end of the block, all the entries are merged into
permanent storage via the exponential moving average logic defined in the math package this
pallet depens on. There is one oracle entry for each combination of `(source, asset_pair,
period)` in storage.

Oracle values are accessed lazily. This means that the storage does not contain the most recent
value, but the value calculated the last time it was updated via trade or liquidity change. On a
read the values are read from storage and then fast-forwarded (assuming the volume to be zero
and the price and liquidity to be constant) to the last block. Note: The most recent oracle
values are always from the last block. This avoids e.g. sandwiching risks. If you want current
prices you should use a spot price or similar.

License: Apache 2.0
