// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

//! Weights for `pallet_meta_tx`.

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for `pallet_meta_tx`.
pub trait WeightInfo {
    fn dispatch_meta_tx() -> Weight;
    fn dispatch_evm_meta_tx() -> Weight;
}

/// Weights for `pallet_meta_tx` using the HydraDX node and recommended hardware.
pub struct HydraWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for HydraWeight<T> {
    /// Storage: `MetaTx::Nonces` (r:1 w:1)
    /// Proof: `MetaTx::Nonces` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
    fn dispatch_meta_tx() -> Weight {
        // Placeholder: dominated by one sr25519 verification plus a single storage read/write.
        // Regenerate with scripts/benchmarking.sh before this pallet carries value on mainnet.
        Weight::from_parts(75_000_000, 3517)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }

    /// Storage: `MetaTx::Nonces` (r:1 w:1)
    /// Proof: `MetaTx::Nonces` (`max_values`: None, `max_size`: Some(52), added: 2527, mode: `MaxEncodedLen`)
    fn dispatch_evm_meta_tx() -> Weight {
        // Placeholder: dominated by one secp256k1 recovery plus a single storage read/write.
        // Regenerate with scripts/benchmarking.sh before this pallet carries value on mainnet.
        Weight::from_parts(120_000_000, 3517)
            .saturating_add(T::DbWeight::get().reads(1_u64))
            .saturating_add(T::DbWeight::get().writes(1_u64))
    }
}

impl WeightInfo for () {
    fn dispatch_meta_tx() -> Weight {
        Weight::from_parts(75_000_000, 3517)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }

    fn dispatch_evm_meta_tx() -> Weight {
        Weight::from_parts(120_000_000, 3517)
            .saturating_add(RocksDbWeight::get().reads(1_u64))
            .saturating_add(RocksDbWeight::get().writes(1_u64))
    }
}
