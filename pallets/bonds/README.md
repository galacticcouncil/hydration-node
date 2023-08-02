# Bonds pallet

## Overview

This pallet provides functionality to issue fungible bonds.
Once the bonds are mature, they can be redeemed for the underlying asset.
The pallet uses `Time` trait to get the timestamp of the last block, provided by the timestamp pallet.

## Assumptions

* When issuing new bonds, new nameless asset of the `AssetType::Bond` is registered for the bonds.
* New amount of bonds is issued when the underlying asset and maturity matches already registered bonds.
* It's possible to create multiple bonds for the same underlying asset.
* Bonds can be issued for all available asset types.
* The existential deposit of the bonds is the same as of the underlying asset.
* A user receives the same amount of bonds as the amount of the underlying asset he provided, minus the protocol fee.
* Maturity of bonds is represented using the Unix time in milliseconds.
* Underlying assets are stored in the pallet account until redeemed.
* Protocol fee is applied to the amount of the underlying asset and transferred to the fee receiver.