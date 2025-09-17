# pallet-build-evm-tx

## Overview

The build-evm-tx pallet provides functionality to construct EIP-1559 compliant EVM transactions and encode them using RLP serialization.

### Usage

Other pallets can use the helper function to build EVM transactions:

```rust
let rlp_encoded = pallet_build_evm_tx::Pallet::<T>::build_evm_tx(
    Some(who),           // Optional account for event emission
    Some(to_address),    // Optional recipient (None for contract creation)
    value,               // ETH value in wei
    data,                // Transaction data/calldata
    nonce,               // Transaction nonce
    gas_limit,           // Gas limit
    max_fee_per_gas,     // Maximum total fee per gas
    max_priority_fee,    // Maximum priority fee (tip) per gas
    chain_id,            // Target chain ID
)?;
```

### Configuration

The pallet requires configuring the maximum data length

```rust
parameter_types! {
    pub const MaxEvmDataLength: u32 = 100_000;
}
```
