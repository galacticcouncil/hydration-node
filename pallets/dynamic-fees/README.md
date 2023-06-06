# pallet-dynamic-fees

Implementation of a fee level mechanism that dynamically changes based on the values provided by an oracle.

### Overview

This module provides functionality to compute an asset fee and a protocol fee within a block.

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

#### Storage

The module stores last calculated fees as tuple of `(Fee, Fee, Block number)` where the first item is asset fee,
the second one is protocol fee and the third one is block number indicating when the two fees were updated.

### Interface

#### Update and retrieve fee

The module provides implementation of GetByKey trait for `UpdateAndRetrieveFee` struct.
This can be used to integrate the dynamic fee mechanism where desired.

On first retrieve call in a block, the asset fee as well as the protocol are updated and new fees are returned.

#### Prerequisites

An oracle which provides volume in and out of an asset and liquidity.

License: Apache 2.0
