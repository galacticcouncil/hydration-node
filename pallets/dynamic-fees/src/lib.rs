// This file is part of pallet-dynamic-fees.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Implementation of a fee level mechanism that dynamically changes based on the values provided by an oracle.
//!
//! ## Overview
//!
//! This module provides functionality to compute an asset fee and a protocol fee within a block.
//!
//! To use it in the runtime, implement the pallet's[`pallet_dynamic_fees::Config`]()
//!
//! and integrate provided [`UpdateAndRetrieveFees`]().
//!
//! ### Terminology
//!
//! * **Fee:** The type representing a fee. Must implement PerThing.
//! * **Oracle:** Implementation of an oracle providing volume in and out as well as liquidity for an asset.
//! * **Asset decay:** The decaying parameter for an asset fee.
//! * **Protocol decay:** The decaying parameter for a protocol fee.
//! * **Asset fee amplification:** The amplification parameter for asset fee.
//! * **Protocol fee amplification:** The amplification parameter for protocol fee.
//! * **Minimum and maximum fee:** The minimum and maximum fee value for asset or protocol fee.
//!
//! ### Storage
//!
//! The module stores last calculated fees as tuple of `(Fee, Fee, Block number)` where the first item is asset fee,
//! the second one is protocol fee and the third one is block number indicating when the two fees were updated.
//!
//! ## Interface
//!
//! ### Update and retrieve fee
//!
//! The module provides implementation of GetByKey trait for `UpdateAndRetrieveFee` struct.
//! This can be used to integrate the dynamic fee mechanism where desired.
//!
//! On first retrieve call in a block, the asset fee as well as the protocol are updated and new fees are returned.
//!
//! ### Prerequisites
//!
//! An oracle which provides volume in and out of an asset and liquidity.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

use frame_support::ensure;
use frame_support::pallet_prelude::DispatchResult;
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::traits::{BlockNumberProvider, Saturating, Zero};
use sp_runtime::{FixedPointOperand, FixedU128, PerThing, SaturatedConversion};

#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks;

pub use pallet::*;

use crate::traits::{Volume, VolumeProvider};
use crate::types::{AssetFeeConfig, FeeEntry, FeeParams};
use hydra_dx_math::dynamic_fees::types::OracleEntry;
use hydra_dx_math::dynamic_fees::{recalculate_asset_fee, recalculate_protocol_fee};
use hydradx_traits::fee::GetDynamicFee;

type Balance = u128;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::traits::VolumeProvider;
	use crate::types::FeeEntry;
	use frame_support::pallet_prelude::*;
	use frame_system::ensure_root;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
	use sp_runtime::traits::{BlockNumberProvider, Zero};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn current_fees)]
	/// Stores last calculated fee of an asset and block number in which it was changed..
	/// Stored as (Asset fee, Protocol fee, Block number)
	pub type AssetFee<T: Config> =
		StorageMap<_, Twox64Concat, T::AssetId, FeeEntry<T::Fee, BlockNumberFor<T>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asset_fee_config)]
	/// Stores per-asset fee configuration (Fixed or Dynamic)
	pub type AssetFeeConfiguration<T: Config> =
		StorageMap<_, Twox64Concat, T::AssetId, AssetFeeConfig<T::Fee>, OptionQuery>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Provider for the current block number.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Fee PerThing type
		type Fee: Parameter + MaybeSerializeDeserialize + MaxEncodedLen + PerThing;

		/// Asset id type
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen;

		/// Volume provider implementation
		type RawOracle: VolumeProvider<Self::AssetId, Balance>;

		#[pallet::constant]
		type AssetFeeParameters: Get<FeeParams<Self::Fee>>;

		#[pallet::constant]
		type ProtocolFeeParameters: Get<FeeParams<Self::Fee>>;

		/// Information on runtime weights.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Asset fee configuration has been set
		AssetFeeConfigSet {
			asset_id: T::AssetId,
			params: AssetFeeConfig<T::Fee>,
		},
		/// Asset fee configuration has been removed
		AssetFeeConfigRemoved { asset_id: T::AssetId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid fee parameters provided
		InvalidFeeParameters,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set fee configuration for an asset
		///
		/// This function allows setting either fixed or dynamic fee configuration for a specific asset.
		///
		/// # Arguments
		/// * `origin` - Root origin required
		/// * `asset_id` - The asset ID to configure
		/// * `config` - Fee configuration (Fixed or Dynamic)
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_asset_fee())]
		pub fn set_asset_fee(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			config: AssetFeeConfig<T::Fee>,
		) -> DispatchResult {
			ensure_root(origin)?;

			Self::validate_fee_config(&config)?;

			AssetFeeConfiguration::<T>::insert(&asset_id, &config);

			Self::deposit_event(Event::AssetFeeConfigSet {
				asset_id,
				params: config,
			});
			Ok(())
		}

		/// Remove fee configuration for an asset (will use default parameters)
		///
		/// This function removes any custom fee configuration for the specified asset.
		/// After removal, the asset will use the default dynamic fee parameters configured in the runtime.
		///
		/// # Arguments
		/// * `origin` - Root origin required
		/// * `asset_id` - The asset ID to remove configuration for
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::remove_asset_fee())]
		pub fn remove_asset_fee(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			ensure_root(origin)?;

			AssetFeeConfiguration::<T>::remove(&asset_id);

			Self::deposit_event(Event::AssetFeeConfigRemoved { asset_id });
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			let asset_fee_params = T::AssetFeeParameters::get();
			let protocol_fee_params = T::ProtocolFeeParameters::get();
			assert!(
				asset_fee_params.min_fee <= asset_fee_params.max_fee,
				"Asset fee min > asset fee max."
			);
			assert!(
				!asset_fee_params.amplification.is_zero(),
				"Asset fee amplification is 0."
			);
			assert!(
				protocol_fee_params.min_fee <= protocol_fee_params.max_fee,
				"Protocol fee min > protocol fee max."
			);
			assert!(
				!protocol_fee_params.amplification.is_zero(),
				"Protocol fee amplification is 0."
			);
		}
	}
}

impl<T: Config> Pallet<T>
where
	<T::Fee as PerThing>::Inner: FixedPointOperand,
{
	/// Validates fee configuration parameters
	///
	/// This function ensures that the provided fee configuration is valid:
	/// - Fixed fees: No validation required
	/// - Dynamic fees: Validates that min_fee <= max_fee and amplification > 0
	///
	/// # Arguments
	/// * `config` - The fee configuration to validate
	///
	/// # Returns
	/// DispatchResult indicating success or validation error
	fn validate_fee_config(config: &AssetFeeConfig<T::Fee>) -> DispatchResult {
		match config {
			AssetFeeConfig::Fixed { .. } => {
				// No validation needed for fixed fees
				Ok(())
			}
			AssetFeeConfig::Dynamic {
				asset_fee_params,
				protocol_fee_params,
			} => {
				ensure!(
					asset_fee_params.min_fee <= asset_fee_params.max_fee && !asset_fee_params.amplification.is_zero(),
					Error::<T>::InvalidFeeParameters
				);
				ensure!(
					protocol_fee_params.min_fee <= asset_fee_params.max_fee
						&& !protocol_fee_params.amplification.is_zero(),
					Error::<T>::InvalidFeeParameters
				);

				Ok(())
			}
		}
	}

	/// Updates fee for an asset based on its configuration
	///
	/// This function determines the fee calculation method based on the asset's configuration:
	/// - Fixed fees: Returns the configured static values
	/// - Dynamic fees: Calculates fees using oracle data and custom parameters
	/// - No configuration: Uses default dynamic parameters
	///
	/// # Arguments
	/// * `asset_id` - The asset ID to update fees for
	/// * `asset_liquidity` - Current asset liquidity
	/// * `store` - Whether to store the calculated fees in storage
	///
	/// # Returns
	/// A tuple of (asset_fee, protocol_fee)
	fn update_fee(asset_id: T::AssetId, asset_liquidity: Balance, store: bool) -> (T::Fee, T::Fee) {
		log::trace!(target: "dynamic-fees", "update_fee for asset_id: {:?}", asset_id);
		let block_number = T::BlockNumberProvider::current_block_number();

		let asset_config = Self::asset_fee_config(asset_id);

		match asset_config {
			Some(AssetFeeConfig::Fixed {
				asset_fee,
				protocol_fee,
			}) => {
				log::trace!(target: "dynamic-fees", "using fixed fees: {:?} {:?}", asset_fee, protocol_fee);
				(asset_fee, protocol_fee)
			}
			Some(AssetFeeConfig::Dynamic {
				asset_fee_params,
				protocol_fee_params,
			}) => {
				// Use dynamic calculation with custom parameters
				let current_fee_entry = Self::current_fees(asset_id).unwrap_or(FeeEntry {
					asset_fee: asset_fee_params.min_fee,
					protocol_fee: protocol_fee_params.min_fee,
					timestamp: BlockNumberFor::<T>::default(),
				});

				Self::calculate_dynamic_fee(
					asset_id,
					asset_liquidity,
					block_number,
					current_fee_entry,
					asset_fee_params,
					protocol_fee_params,
					store,
				)
			}
			None => {
				// Use default parameters from config
				let asset_fee_params = T::AssetFeeParameters::get();
				let protocol_fee_params = T::ProtocolFeeParameters::get();

				let current_fee_entry = Self::current_fees(asset_id).unwrap_or(FeeEntry {
					asset_fee: asset_fee_params.min_fee,
					protocol_fee: protocol_fee_params.min_fee,
					timestamp: BlockNumberFor::<T>::default(),
				});

				Self::calculate_dynamic_fee(
					asset_id,
					asset_liquidity,
					block_number,
					current_fee_entry,
					asset_fee_params,
					protocol_fee_params,
					store,
				)
			}
		}
	}

	fn calculate_dynamic_fee(
		asset_id: T::AssetId,
		asset_liquidity: Balance,
		block_number: BlockNumberFor<T>,
		current_fee_entry: FeeEntry<T::Fee, BlockNumberFor<T>>,
		asset_fee_params: FeeParams<T::Fee>,
		protocol_fee_params: FeeParams<T::Fee>,
		store: bool,
	) -> (T::Fee, T::Fee) {
		if block_number == current_fee_entry.timestamp {
			log::trace!(target: "dynamic-fees", "no need to update, same block. Current fees: {:?} {:?}", current_fee_entry.asset_fee, current_fee_entry.protocol_fee);
			return (current_fee_entry.asset_fee, current_fee_entry.protocol_fee);
		}

		let delta_blocks: u128 = block_number
			.saturating_sub(current_fee_entry.timestamp)
			.saturated_into();

		let Some(raw_entry) = T::RawOracle::last_entry(asset_id) else {
			return (current_fee_entry.asset_fee, current_fee_entry.protocol_fee);
		};

		log::trace!(target: "dynamic-fees", "block number: {:?}", block_number);
		log::trace!(target: "dynamic-fees", "delta blocks: {:?}", delta_blocks);
		log::trace!(target: "dynamic-fees", "oracle entry: in {:?}, out {:?}, liquidity: {:?}", raw_entry.amount_in(), raw_entry.amount_out(), raw_entry.liquidity());

		let period = T::RawOracle::period() as u128;
		if period.is_zero() {
			// This should never happen, but if it does, we should not panic.
			debug_assert!(false, "Oracle period is 0");
			return (current_fee_entry.asset_fee, current_fee_entry.protocol_fee);
		}
		let decay_factor = FixedU128::from_rational(4u128, period);
		log::trace!(target: "dynamic-fees", "decay factor: {:?}", decay_factor);

		let fee_updated_at: u128 = current_fee_entry.timestamp.saturated_into();
		if !fee_updated_at.is_zero() {
			debug_assert!(
				fee_updated_at == raw_entry.updated_at(),
				"Dynamic fee update - last fee updated at {:?} but expected to be >= {:?}",
				current_fee_entry.timestamp,
				raw_entry.updated_at()
			);
		}

		let asset_fee = recalculate_asset_fee(
			OracleEntry {
				amount_in: raw_entry.amount_in(),
				amount_out: raw_entry.amount_out(),
				liquidity: raw_entry.liquidity(),
				decay_factor,
			},
			asset_liquidity,
			current_fee_entry.asset_fee,
			delta_blocks,
			asset_fee_params.into(),
		);
		let protocol_fee = recalculate_protocol_fee(
			OracleEntry {
				amount_in: raw_entry.amount_in(),
				amount_out: raw_entry.amount_out(),
				liquidity: raw_entry.liquidity(),
				decay_factor,
			},
			asset_liquidity,
			current_fee_entry.protocol_fee,
			delta_blocks,
			protocol_fee_params.into(),
		);

		if store {
			AssetFee::<T>::insert(
				asset_id,
				FeeEntry {
					asset_fee,
					protocol_fee,
					timestamp: block_number,
				},
			);
		}
		log::trace!(target: "dynamic-fees", "new fees: {:?} {:?}", asset_fee, protocol_fee);
		(asset_fee, protocol_fee)
	}
}

/// Main interface for retrieving dynamic fees
///
/// This struct provides the implementation of `GetDynamicFee` trait that can be used
/// throughout the runtime to retrieve updated fees for assets. The fees are calculated
/// based on the asset's configuration (fixed or dynamic) and oracle data.
pub struct UpdateAndRetrieveFees<T: Config>(sp_std::marker::PhantomData<T>);

impl<T: Config> GetDynamicFee<(T::AssetId, Balance)> for UpdateAndRetrieveFees<T>
where
	<T::Fee as PerThing>::Inner: FixedPointOperand,
{
	type Fee = (T::Fee, T::Fee);

	fn get(k: (T::AssetId, Balance)) -> Self::Fee {
		Pallet::<T>::update_fee(k.0, k.1, false)
	}

	fn get_and_store(k: (T::AssetId, Balance)) -> Self::Fee {
		Pallet::<T>::update_fee(k.0, k.1, true)
	}
}
