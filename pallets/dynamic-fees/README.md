# pallet-dynamic-fees

Implementation of a fee level mechanism that supports both fixed and dynamic fee configurations based on oracle data.

### Overview

This module provides functionality to compute an asset fee and a protocol fee within a block. The pallet supports per-asset fee configuration, allowing each asset to use either:
- Fixed fees that remain constant
- Dynamic fees that change based on oracle data

To use it in the runtime, implement the pallet's[`pallet_dynamic_fees::Config`]()

and integrate provided [`UpdateAndRetrieveFees`]().

#### Terminology

* **Fee:** The type representing a fee. Must implement PerThing.
* **Oracle:** Implementation of an oracle providing volume in and out as well as liquidity for an asset.
* **Asset decay:** The decaying parameter for an asset fee.
* **Protocol decay:** The decaying parameter for a protocol fee.
* **Asset fee amplification:** The amplification parameter for asset fee.
* **Protocol fee amplification:** The amplification parameter for protocol fee.
* **Minimum and maximum fee:** The minimum and maximum fee value for asset or protocol fee.
* **Fixed fee configuration:** Static fee values that don't change based on oracle data.
* **Dynamic fee configuration:** Fee calculation using oracle data and custom parameters.

#### Storage

The module stores last calculated fees as tuple of `(Fee, Fee, Block number)` where the first item is asset fee,
the second one is protocol fee and the third one is block number indicating when the two fees were updated.

Additionally, the pallet stores per-asset fee configurations that determine whether to use fixed or dynamic fees.

### Interface

#### Update and retrieve fee

The module provides implementation of GetByKey trait for `UpdateAndRetrieveFee` struct.
This can be used to integrate the dynamic fee mechanism where desired.

On first retrieve call in a block, the asset fee as well as the protocol are updated and new fees are returned.

#### Fee Configuration

The pallet supports two types of fee configurations per asset:

1. **Fixed Fees**: Static fee values that remain constant regardless of oracle data
2. **Dynamic Fees**: Fee calculation based on oracle data using custom parameters

##### Setting Fixed Fees

```rust
// Set fixed fees for an asset
DynamicFees::set_asset_fee(
    origin,
    asset_id,
    AssetFeeConfig::Fixed {
        asset_fee: fee_value,
        protocol_fee: fee_value,
    }
)?;
```

##### Setting Dynamic Fees

```rust
// Set dynamic fees with custom parameters
DynamicFees::set_asset_fee(
    origin,
    asset_id,
    AssetFeeConfig::Dynamic {
        asset_fee_params: FeeParams {
            min_fee: min_asset_fee,
            max_fee: max_asset_fee,
            decay: decay_factor,
            amplification: amplification_factor,
        },
        protocol_fee_params: FeeParams {
            min_fee: min_protocol_fee,
            max_fee: max_protocol_fee,
            decay: decay_factor,
            amplification: amplification_factor,
        },
    }
)?;
```

##### Removing Configuration

```rust
// Remove custom configuration (falls back to default parameters)
DynamicFees::remove_asset_fee(origin, asset_id)?;
```

#### Prerequisites

- An oracle which provides volume in and out of an asset and liquidity (for dynamic fees)
- Default fee parameters configured in the runtime (used when no custom configuration is set)

License: Apache 2.0
