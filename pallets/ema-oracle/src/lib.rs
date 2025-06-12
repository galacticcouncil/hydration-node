// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
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

//! # EMA Oracle Pallet
//!
//! ## Overview
//!
//! This pallet provides exponential moving average (EMA) oracles of different periods for price,
//! volume and liquidity for a combination of source and asset pair based on data coming in from
//! different sources.
//!
//! ### Integration
//!
//! Data is ingested by plugging the provided `OnActivityHandler` into callbacks provided by other
//! pallets (e.g. xyk pallet).
//!
//! It is meant to be used by other pallets via the `AggregatedOracle` and `AggregatedPriceOracle`
//! traits.
//!
//! When integrating with this pallet take care to use the `on_trade_weight`,
//! `on_liquidity_changed_weight` and `get_entry_weight` into account when calculating the weight
//! for your extrinsics (that either feed data into or take data from this pallet).
//!
//! ### Concepts
//!
//! - *EMA*: Averaging via exponential decay with a smoothing factor; meaning each new value to
//!   integrate into the average is multiplied with a smoothing factor between 0 and 1.
//! - *Smoothing Factor*: A factor applied to each value aggregated into the averaging oracle.
//!   Implicitly determines the oracle period.
//! - *Period*: The window over which an oracle is averaged. Certain smoothing factors correspond to
//!   an oracle period. E.g. ten minutes oracle period ≈ 0.0198
//! - *Source*: The source of the data. E.g. xyk pallet.
//!
//! ### Implementation
//!
//! This pallet aggregates data in the following way: `on_trade` or `on_liquidity_changed` a new
//! entry is created for the incoming data. This then updates any existing entries already present
//! in storage for this block (for this combination of source and assets) or inserts it. Note that
//! this aggregation is NOT based on EMA, yet, it just sums the volume and replaces price and
//! liquidity with the most recent value.
//!
//! At the end of the block, all the entries are merged into permanent storage via the exponential
//! moving average logic defined in the math package this pallet depens on. There is one oracle
//! entry for each combination of `(source, asset_pair, period)` in storage.
//!
//! Oracle values are accessed lazily. This means that the storage does not contain the most recent
//! value, but the value calculated the last time it was updated via trade or liquidity change. On a
//! read the values are read from storage and then fast-forwarded (assuming the volume to be zero
//! and the price and liquidity to be constant) to the last block. Note: The most recent oracle
//! values are always from the last block. This avoids e.g. sandwiching risks. If you want current
//! prices you should use a spot price or similar.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::{BlockNumberProvider, One, Zero};
use frame_support::traits::Contains;
use frame_system::pallet_prelude::BlockNumberFor;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::{
	AggregatedEntry, AggregatedOracle, AggregatedPriceOracle, Liquidity, OnCreatePoolHandler,
	OnLiquidityChangedHandler, OnTradeHandler, RawEntry, RawOracle, Volume,
};
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::FixedU128;
use sp_arithmetic::Permill;
use sp_runtime::traits::Convert;
use sp_std::marker::PhantomData;
use sp_std::prelude::*;

#[cfg(test)]
mod tests;

mod types;
pub use types::*;

#[allow(clippy::all)]
pub mod weights;
pub use weights::WeightInfo;
/// The maximum number of periods that could have corresponding oracles.
pub const MAX_PERIODS: u32 = OraclePeriod::all_periods().len() as u32;

pub const BIFROST_SOURCE: [u8; 8] = *b"bifrosto";

const LOG_TARGET: &str = "runtime::ema-oracle";

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId> {
	fn register_asset(asset_id: AssetId) -> DispatchResult;
}
#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper<AssetId> for () {
	fn register_asset(_asset_id: AssetId) -> DispatchResult {
		Ok(())
	}
}

#[allow(clippy::type_complexity)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{BoundedBTreeMap, BoundedBTreeSet};
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Origin that can enable oracle for assets that would be rejected by `OracleWhitelist` otherwise.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Origin that can update bifrost oracle via `update_bifrost_oracle` extrinsic.
		type BifrostOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Provider for the current block number.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// The periods supported by the pallet. I.e. which oracles to track.
		type SupportedPeriods: Get<BoundedVec<OraclePeriod, ConstU32<MAX_PERIODS>>>;

		/// Whitelist determining what oracles are tracked by the pallet.
		type OracleWhitelist: Contains<(Source, AssetId, AssetId)>;

		/// Location to Asset Id converter
		type LocationToAssetIdConversion: sp_runtime::traits::Convert<polkadot_xcm::VersionedLocation, Option<AssetId>>;

		/// Maximum allowed percentage difference for bifrost oracle price update
		#[pallet::constant]
		type MaxAllowedPriceDifference: Get<Permill>;

		/// Maximum number of unique oracle entries expected in one block.
		#[pallet::constant]
		type MaxUniqueEntries: Get<u32>;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: BenchmarkHelper<AssetId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		TooManyUniqueEntries,
		OnTradeValueZero,
		OracleNotFound,
		/// Asset not found
		AssetNotFound,
		///The new price is outside the max allowed range
		PriceOutsideAllowedRange,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Oracle was added to the whitelist.
		AddedToWhitelist { source: Source, assets: (AssetId, AssetId) },
		/// Oracle was removed from the whitelist.
		RemovedFromWhitelist { source: Source, assets: (AssetId, AssetId) },
	}

	/// Accumulator for oracle data in current block that will be recorded at the end of the block.
	#[pallet::storage]
	#[pallet::getter(fn accumulator)]
	pub type Accumulator<T: Config> = StorageValue<
		_,
		BoundedBTreeMap<(Source, (AssetId, AssetId)), OracleEntry<BlockNumberFor<T>>, T::MaxUniqueEntries>,
		ValueQuery,
	>;

	/// Oracle storage keyed by data source, involved asset ids and the period length of the oracle.
	///
	/// Stores the data entry as well as the block number when the oracle was first initialized.
	#[pallet::storage]
	#[pallet::getter(fn oracle)]
	pub type Oracles<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Twox64Concat, Source>,
			NMapKey<Twox64Concat, (AssetId, AssetId)>,
			NMapKey<Twox64Concat, OraclePeriod>,
		),
		(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>),
		OptionQuery,
	>;

	/// Assets that are whitelisted and tracked by the pallet.
	#[pallet::storage]
	#[pallet::getter(fn whitelisted_assets)]
	pub type WhitelistedAssets<T: Config> =
		StorageValue<_, BoundedBTreeSet<(Source, (AssetId, AssetId)), T::MaxUniqueEntries>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub initial_data: Vec<(Source, (AssetId, AssetId), Price, Liquidity<Balance>)>,
		#[serde(skip)]
		pub _marker: PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			for &(source, (asset_a, asset_b), price, liquidity) in self.initial_data.iter() {
				let entry: OracleEntry<BlockNumberFor<T>> = {
					let e = OracleEntry {
						price,
						volume: Volume::default(),
						liquidity,
						updated_at: BlockNumberFor::<T>::zero(),
					};
					if ordered_pair(asset_a, asset_b) == (asset_a, asset_b) {
						e
					} else {
						e.inverted()
					}
				};

				for period in T::SupportedPeriods::get() {
					Pallet::<T>::update_oracle(source, ordered_pair(asset_a, asset_b), period, entry.clone());
				}
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			T::WeightInfo::on_finalize_no_entry()
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			// update oracles based on data accumulated during the block
			Self::update_oracles_from_accumulator();
		}

		fn integrity_test() {
			assert!(
				T::MaxUniqueEntries::get() > 0,
				"At least one trade should be possible per block."
			);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::add_oracle())]
		pub fn add_oracle(origin: OriginFor<T>, source: Source, assets: (AssetId, AssetId)) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let assets = ordered_pair(assets.0, assets.1);

			WhitelistedAssets::<T>::mutate(|list| {
				list.try_insert((source, (assets)))
					.map_err(|_| Error::<T>::TooManyUniqueEntries)
			})?;

			Self::deposit_event(Event::AddedToWhitelist { source, assets });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_oracle())]
		pub fn remove_oracle(origin: OriginFor<T>, source: Source, assets: (AssetId, AssetId)) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			let assets = ordered_pair(assets.0, assets.1);

			WhitelistedAssets::<T>::mutate(|list| {
				ensure!(list.remove(&(source, (assets))), Error::<T>::OracleNotFound);

				Ok::<(), DispatchError>(())
			})?;

			// remove oracle from the storage
			for period in T::SupportedPeriods::get().into_iter() {
				let _ = Accumulator::<T>::mutate(|accumulator| {
					accumulator.remove(&(source, assets));
					Ok::<(), ()>(())
				});
				Oracles::<T>::remove((source, assets, period));
			}

			Self::deposit_event(Event::RemovedFromWhitelist { source, assets });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_bifrost_oracle())]
		pub fn update_bifrost_oracle(
			origin: OriginFor<T>,
			//NOTE: these must be boxed becasue of https://github.com/paritytech/polkadot-sdk/blob/6875d36b2dba537f3254aad3db76ac7aa656b7ab/substrate/frame/utility/src/lib.rs#L150
			asset_a: Box<polkadot_xcm::VersionedLocation>,
			asset_b: Box<polkadot_xcm::VersionedLocation>,
			price: (Balance, Balance),
		) -> DispatchResultWithPostInfo {
			T::BifrostOrigin::ensure_origin(origin)?;

			let asset_a = T::LocationToAssetIdConversion::convert(*asset_a).ok_or(Error::<T>::AssetNotFound)?;
			let asset_b = T::LocationToAssetIdConversion::convert(*asset_b).ok_or(Error::<T>::AssetNotFound)?;

			let ordered_pair = ordered_pair(asset_a, asset_b);
			let entry: OracleEntry<BlockNumberFor<T>> = {
				let e = OracleEntry::new(
					EmaPrice::new(price.0, price.1),
					Volume::default(),
					Liquidity::default(),
					T::BlockNumberProvider::current_block_number(),
				);
				if ordered_pair == (asset_a, asset_b) {
					e
				} else {
					e.inverted()
				}
			};

			if let Some(reference_entry) = Self::oracle((BIFROST_SOURCE, ordered_pair, OraclePeriod::TenMinutes)) {
				if !Self::is_within_range(reference_entry.0.price.into(), price) {
					log::error!(
						target: LOG_TARGET,
						"Updating biforst oracle failed as the price is outside the allowed range"
					);
					return Err(Error::<T>::PriceOutsideAllowedRange.into());
				}
			}

			Self::on_entry(BIFROST_SOURCE, ordered_pair, entry).map_err(|_| Error::<T>::TooManyUniqueEntries)?;

			Ok(Pays::No.into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Insert or update data in the accumulator from received entry. Aggregates volume and
	/// takes the most recent data for the rest.
	pub(crate) fn on_entry(
		src: Source,
		assets: (AssetId, AssetId),
		oracle_entry: OracleEntry<BlockNumberFor<T>>,
	) -> Result<(), ()> {
		if !T::OracleWhitelist::contains(&(src, assets.0, assets.1)) && src.ne(&BIFROST_SOURCE) {
			// if we don't track oracle for given asset pair, don't throw error
			return Ok(());
		}

		Accumulator::<T>::mutate(|accumulator| {
			if let Some(entry) = accumulator.get_mut(&(src, assets)) {
				entry.accumulate_volume_and_update_from(&oracle_entry);
				Ok(())
			} else {
				accumulator
					.try_insert((src, assets), oracle_entry)
					.map(|_| ())
					.map_err(|_| ())
			}
		})
	}

	/// Insert or update data in the accumulator from received entry. Aggregates volume and
	/// takes the most recent data for the rest.
	pub(crate) fn on_trade(
		src: Source,
		assets: (AssetId, AssetId),
		oracle_entry: OracleEntry<BlockNumberFor<T>>,
	) -> Result<Weight, (Weight, DispatchError)> {
		let weight = OnActivityHandler::<T>::on_trade_weight();
		Self::on_entry(src, assets, oracle_entry)
			.map(|_| weight)
			.map_err(|_| (weight, Error::<T>::TooManyUniqueEntries.into()))
	}

	/// Insert or update data in the accumulator from received entry. Aggregates volume and
	/// takes the most recent data for the rest.
	pub(crate) fn on_liquidity_changed(
		src: Source,
		assets: (AssetId, AssetId),
		oracle_entry: OracleEntry<BlockNumberFor<T>>,
	) -> Result<Weight, (Weight, DispatchError)> {
		let weight = OnActivityHandler::<T>::on_liquidity_changed_weight();
		Self::on_entry(src, assets, oracle_entry)
			.map(|_| weight)
			.map_err(|_| (weight, Error::<T>::TooManyUniqueEntries.into()))
	}

	/// Return the current value of the `LastBlock` oracle for the given `source` and `assets`.
	pub(crate) fn last_block_oracle(
		source: Source,
		assets: (AssetId, AssetId),
		block: BlockNumberFor<T>,
	) -> Option<(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>)> {
		Self::oracle((source, assets, OraclePeriod::LastBlock)).map(|(mut last_block, init)| {
			// update the `LastBlock` oracle to the last block if it hasn't been updated for a while
			// price and liquidity stay constant, volume becomes zero
			if last_block.updated_at != block {
				last_block.fast_forward_to(block);
			}
			(last_block, init)
		})
	}

	/// Update oracles based on data accumulated during the block.
	fn update_oracles_from_accumulator() {
		for ((src, assets), oracle_entry) in Accumulator::<T>::take().into_iter() {
			// First we update the non-immediate oracles with the value of the `LastBlock` oracle.
			for period in T::SupportedPeriods::get()
				.into_iter()
				.filter(|p| *p != OraclePeriod::LastBlock)
			{
				Self::update_oracle(src, assets, period, oracle_entry.clone());
			}
			// As we use (the old value of) the `LastBlock` entry to update the other oracles it
			// gets updated last.
			Self::update_oracle(src, assets, OraclePeriod::LastBlock, oracle_entry.clone());
		}
	}

	/// Update the oracle of the given source, assets and period with `oracle_entry`.
	fn update_oracle(
		src: Source,
		assets: (AssetId, AssetId),
		period: OraclePeriod,
		incoming_entry: OracleEntry<BlockNumberFor<T>>,
	) {
		Oracles::<T>::mutate((src, assets, period), |oracle| {
			// initialize the oracle entry if it doesn't exist
			if oracle.is_none() {
				*oracle = Some((incoming_entry.clone(), T::BlockNumberProvider::current_block_number()));
				return;
			}
			if let Some((prev_entry, _)) = oracle.as_mut() {
				let parent = T::BlockNumberProvider::current_block_number().saturating_sub(One::one());
				// update the entry to the parent block if it hasn't been updated for a while
				if parent > prev_entry.updated_at {
					Self::last_block_oracle(src, assets, parent)
						.and_then(|(last_block, _)| {
							prev_entry.update_outdated_to_current(period, &last_block).map(|_| ())
						})
						.unwrap_or_else(|| {
							log::warn!(
								target: LOG_TARGET,
								"Updating EMA oracle ({src:?}, {assets:?}, {period:?}) to parent block failed. Defaulting to previous value."
							);
							debug_assert!(false, "Updating to parent block should not fail.");
						})
				}
				// calculate the actual update with the new value
				prev_entry
					.update_to_new_by_integrating_incoming(period, &incoming_entry)
					.map(|_| ())
					.unwrap_or_else(|| {
						log::warn!(
							target: LOG_TARGET,
							"Updating EMA oracle ({src:?}, {assets:?}, {period:?}) to new value failed. Defaulting to previous value."
						);
						debug_assert!(false, "Updating to new value should not fail.");
					});
			};
		});
	}

	/// Return the updated oracle entry for the given source, assets and period.
	///
	/// The value will be up to date until the parent block, thus excluding trading data from the
	/// current block. Note: It does not update the values in storage.
	fn get_updated_entry(
		src: Source,
		assets: (AssetId, AssetId),
		period: OraclePeriod,
	) -> Option<(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>)> {
		let parent = T::BlockNumberProvider::current_block_number().saturating_sub(One::one());
		// First get the `LastBlock` oracle to calculate the updated values for the others.
		let (last_block, last_block_init) = Self::last_block_oracle(src, assets, parent)?;
		// If it was requested return it directly.
		if period == OraclePeriod::LastBlock {
			return Some((last_block, last_block_init));
		}

		let (entry, init) = Self::oracle((src, assets, period))?;
		if entry.updated_at < parent {
			entry.calculate_current_from_outdated(period, &last_block)
		} else {
			Some(entry)
		}
		.map(|return_entry| (return_entry, init))
	}

	/// Return last stored entry for given period and block number of last updated.
	pub fn get_last_oracle_entry(
		source: Source,
		assets: (AssetId, AssetId),
		period: OraclePeriod,
	) -> Option<(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>)> {
		Self::oracle((source, ordered_pair(assets.0, assets.1), period)).map(|(last_entry, init)| {
			let entry = if (assets.0, assets.1) != ordered_pair(assets.0, assets.1) {
				last_entry.inverted()
			} else {
				last_entry
			};
			(entry, init)
		})
	}

	fn is_within_range(reference_price: (u128, u128), new_price: (u128, u128)) -> bool {
		let reference = FixedU128::from_rational(reference_price.0, reference_price.1);
		let new_value = FixedU128::from_rational(new_price.0, new_price.1);

		let percentage_difference = T::MaxAllowedPriceDifference::get();
		let lower_bound = reference.saturating_mul(FixedU128::one().saturating_sub(percentage_difference.into()));
		let upper_bound = reference.saturating_mul(FixedU128::one().saturating_add(percentage_difference.into()));

		new_value >= lower_bound && new_value <= upper_bound
	}
}

/// A callback handler for trading and liquidity activity that schedules oracle updates.
pub struct OnActivityHandler<T>(PhantomData<T>);

impl<T: Config> OnCreatePoolHandler<AssetId> for OnActivityHandler<T> {
	// Nothing to do on pool creation. Oracles are created lazily.
	fn on_create_pool(_asset_a: AssetId, _asset_b: AssetId) -> DispatchResult {
		Ok(())
	}
}

/// Calculate the weight contribution of one `on_trade`/`on_liquidity_changed` call towards
/// `on_finalize`.
pub(crate) fn fractional_on_finalize_weight<T: Config>(max_entries: u32) -> Weight {
	T::WeightInfo::on_finalize_multiple_tokens(max_entries)
		.saturating_sub(T::WeightInfo::on_finalize_no_entry())
		.saturating_div(max_entries.into())
}

impl<T: Config> OnTradeHandler<AssetId, Balance, Price> for OnActivityHandler<T> {
	fn on_trade(
		source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		amount_a: Balance,
		amount_b: Balance,
		liquidity_a: Balance,
		liquidity_b: Balance,
		price: Price,
	) -> Result<Weight, (Weight, DispatchError)> {
		// We assume that zero liquidity values are not valid and can be ignored.
		if liquidity_a.is_zero() || liquidity_b.is_zero() {
			log::warn!(
				target: LOG_TARGET,
				"Liquidity amounts should not be zero. Source: {source:?}, liquidity: ({liquidity_a},{liquidity_b})"
			);
			return Err((Self::on_trade_weight(), Error::<T>::OnTradeValueZero.into()));
		}

		let price = determine_normalized_price(asset_a, asset_b, price);
		let volume = determine_normalized_volume(asset_a, asset_b, amount_a, amount_b);
		let liquidity = determine_normalized_liquidity(asset_a, asset_b, liquidity_a, liquidity_b);

		let updated_at = T::BlockNumberProvider::current_block_number();
		let entry = OracleEntry {
			price,
			volume,
			liquidity,
			updated_at,
		};
		Pallet::<T>::on_trade(source, ordered_pair(asset_a, asset_b), entry)
	}

	fn on_trade_weight() -> Weight {
		let max_entries = T::MaxUniqueEntries::get();
		// on_trade + on_finalize / max_entries
		T::WeightInfo::on_trade_multiple_tokens(max_entries)
			.saturating_add(fractional_on_finalize_weight::<T>(max_entries))
	}
}

impl<T: Config> OnLiquidityChangedHandler<AssetId, Balance, Price> for OnActivityHandler<T> {
	fn on_liquidity_changed(
		source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		_amount_a: Balance,
		_amount_b: Balance,
		liquidity_a: Balance,
		liquidity_b: Balance,
		price: Price,
	) -> Result<Weight, (Weight, DispatchError)> {
		if liquidity_a.is_zero() || liquidity_b.is_zero() {
			log::trace!(
				target: LOG_TARGET,
				"Liquidity is zero. Source: {source:?}, liquidity: ({liquidity_a},{liquidity_a})"
			);
		}

		let price = determine_normalized_price(asset_a, asset_b, price);
		let liquidity = determine_normalized_liquidity(asset_a, asset_b, liquidity_a, liquidity_b);
		let updated_at = T::BlockNumberProvider::current_block_number();
		let entry = OracleEntry {
			price,
			// liquidity provision does not count as trade volume
			volume: Volume::default(),
			liquidity,
			updated_at,
		};
		Pallet::<T>::on_liquidity_changed(source, ordered_pair(asset_a, asset_b), entry)
	}

	fn on_liquidity_changed_weight() -> Weight {
		let max_entries = T::MaxUniqueEntries::get();
		// on_liquidity + on_finalize / max_entries
		T::WeightInfo::on_liquidity_changed_multiple_tokens(max_entries)
			.saturating_add(fractional_on_finalize_weight::<T>(max_entries))
	}
}

/// Calculate price from ordered assets
pub fn determine_normalized_price(asset_in: AssetId, asset_out: AssetId, price: Price) -> Price {
	if ordered_pair(asset_in, asset_out) == (asset_in, asset_out) {
		price
	} else {
		price.inverted()
	}
}

/// Construct `Volume` based on unordered assets.
pub fn determine_normalized_volume(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
) -> Volume<Balance> {
	if ordered_pair(asset_in, asset_out) == (asset_in, asset_out) {
		Volume::from_a_in_b_out(amount_in, amount_out)
	} else {
		Volume::from_a_out_b_in(amount_out, amount_in)
	}
}

/// Construct `Liquidity` based on unordered assets.
pub fn determine_normalized_liquidity(
	asset_in: AssetId,
	asset_out: AssetId,
	liquidity_asset_in: Balance,
	liquidity_asset_out: Balance,
) -> Liquidity<Balance> {
	if ordered_pair(asset_in, asset_out) == (asset_in, asset_out) {
		Liquidity::new(liquidity_asset_in, liquidity_asset_out)
	} else {
		Liquidity::new(liquidity_asset_out, liquidity_asset_in)
	}
}

/// Return ordered asset tuple (A,B) where A < B
/// Used in storage
/// The implementation is the same as for AssetPair
pub fn ordered_pair(asset_a: AssetId, asset_b: AssetId) -> (AssetId, AssetId) {
	match asset_a <= asset_b {
		true => (asset_a, asset_b),
		false => (asset_b, asset_a),
	}
}

/// Possible errors when requesting an oracle value.
#[derive(RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, TypeInfo)]
pub enum OracleError {
	/// The oracle could not be found
	NotPresent,
	/// The oracle is not defined if the asset ids are the same.
	SameAsset,
}

impl<T: Config> AggregatedOracle<AssetId, Balance, BlockNumberFor<T>, Price> for Pallet<T> {
	type Error = OracleError;

	/// Returns the entry corresponding to the given assets and period.
	/// The entry is updated to the state of the parent block (but not trading data in the current
	/// block). It is also adjusted to make sense for the asset order given as parameters. So
	/// calling `get_entry(HDX, DOT, LastBlock, Omnipool)` will return the price `HDX/DOT`, while
	/// `get_entry(DOT, HDX, LastBlock, Omnipool)` will return `DOT/HDX`.
	fn get_entry(
		asset_a: AssetId,
		asset_b: AssetId,
		period: OraclePeriod,
		source: Source,
	) -> Result<AggregatedEntry<Balance, BlockNumberFor<T>, Price>, OracleError> {
		if asset_a == asset_b {
			return Err(OracleError::SameAsset);
		};
		Self::get_updated_entry(source, ordered_pair(asset_a, asset_b), period)
			.ok_or(OracleError::NotPresent)
			.map(|(entry, initialized)| {
				let entry = if (asset_a, asset_b) != ordered_pair(asset_a, asset_b) {
					entry.inverted()
				} else {
					entry
				};
				entry.into_aggregated(initialized)
			})
	}

	fn get_entry_weight() -> Weight {
		T::WeightInfo::get_entry()
	}
}

impl<T: Config> AggregatedPriceOracle<AssetId, BlockNumberFor<T>, Price> for Pallet<T> {
	type Error = OracleError;

	fn get_price(
		asset_a: AssetId,
		asset_b: AssetId,
		period: OraclePeriod,
		source: Source,
	) -> Result<(Price, BlockNumberFor<T>), Self::Error> {
		Self::get_entry(asset_a, asset_b, period, source)
			.map(|AggregatedEntry { price, oracle_age, .. }| (price, oracle_age))
	}

	fn get_price_weight() -> Weight {
		Self::get_entry_weight()
	}
}

/// Oracle whitelist based on the pallet's storage.
pub struct OracleWhitelist<T>(PhantomData<T>);
impl<T: Config> Contains<(Source, AssetId, AssetId)> for OracleWhitelist<T> {
	fn contains(t: &(Source, AssetId, AssetId)) -> bool {
		WhitelistedAssets::<T>::get().contains(&(t.0, (t.1, t.2)))
	}
}

impl<T: Config> RawOracle<AssetId, Balance, BlockNumberFor<T>> for Pallet<T> {
	type Error = OracleError;

	fn get_raw_entry(
		source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		period: OraclePeriod,
	) -> Result<RawEntry<Balance, BlockNumberFor<T>>, Self::Error> {
		if asset_a == asset_b {
			return Err(OracleError::SameAsset);
		}
		let assets = ordered_pair(asset_a, asset_b);
		let (entry, _) = Self::oracle((source, assets, period)).ok_or(OracleError::NotPresent)?;
		let entry = if (asset_a, asset_b) == assets {
			entry
		} else {
			entry.inverted()
		};
		Ok(RawEntry {
			price: (entry.price.n, entry.price.d),
			volume: entry.volume,
			liquidity: entry.liquidity,
			updated_at: entry.updated_at,
		})
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: Config> Pallet<T> {
	// Helper function for runtime-benchmarking to directly set oracle value.
	pub fn add_entry(
		src: Source,
		assets: (AssetId, AssetId),
		oracle_entry: OracleEntry<BlockNumberFor<T>>,
	) -> Result<(), DispatchError> {
		Self::on_entry(src, assets, oracle_entry).map_err(|_| Error::<T>::OracleNotFound.into())
	}
}
