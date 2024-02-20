# EVM accounts pallet

## Terminology

* **Truncated address:** * A substrate address created from an EVM address by prefixing it with "ETH\0" and appending with eight 0 bytes.
* **Full Substrate address:** * Original 32 bytes long native address (not a truncated address).
* **EVM address:** * First 20 bytes of a Substrate address.

## Overview

The pallet allows users to bind their Substrate account to the EVM address.
The purpose of this pallet is to make interaction with the EVM easier.
Binding an address is not necessary for interacting with the EVM.

Without binding, we are unable to get the original Substrate address from the EVM address inside
of the EVM. Inside of the EVM, we have access only to the EVM address (first 20 bytes of a Substrate account).
In this case we create and use a truncated version of the original Substrate address that called the EVM.
The original and truncated address are two different Substrate addresses.

With binding, we store the last 12 bytes of the Substrate address. Then we can get the original
Substrate address by concatenating these 12 bytes stored in the storage to the EVM address.

### Dispatchable Functions

* `bind_evm_address` - Binds a Substrate address to EVM address.