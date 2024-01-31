#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Pallet, MAX_ASSETS_IN_POOL};
use sp_runtime::Permill;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::num::NonZeroU16;
use sp_std::prelude::*;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::ConstU32;
use frame_support::weights::Weight;
use frame_support::BoundedVec;
use hydra_dx_math::stableswap::types::AssetReserve;
use orml_traits::MultiCurrency;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime::DispatchResult;
pub(crate) type Balance = u128;

/// Pool properties for 2-asset pool (v1)
/// `assets`: pool assets
/// `amplification`: amp parameter
/// `fee`: trade fee to be withdrawn on sell/buy
#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct PoolInfo<AssetId, BlockNumber> {
	pub assets: BoundedVec<AssetId, ConstU32<MAX_ASSETS_IN_POOL>>,
	pub initial_amplification: NonZeroU16,
	pub final_amplification: NonZeroU16,
	pub initial_block: BlockNumber,
	pub final_block: BlockNumber,
	pub fee: Permill,
}

fn has_unique_elements<T>(iter: &mut T) -> bool
where
	T: Iterator,
	T::Item: Ord,
{
	let mut uniq = BTreeSet::new();
	iter.all(move |x| uniq.insert(x))
}

impl<AssetId, Blocknumber> PoolInfo<AssetId, Blocknumber>
where
	AssetId: Ord + Copy,
{
	pub(crate) fn find_asset(&self, asset: AssetId) -> Option<usize> {
		self.assets.iter().position(|v| *v == asset)
	}

	pub(crate) fn is_valid(&self) -> bool {
		self.assets.len() >= 2 && has_unique_elements(&mut self.assets.iter())
	}

	pub(crate) fn reserves_with_decimals<T: Config>(&self, account: &T::AccountId) -> Option<Vec<AssetReserve>>
	where
		T::AssetId: From<AssetId>,
	{
		self.assets
			.iter()
			.map(|asset| {
				let reserve = T::Currency::free_balance((*asset).into(), account);
				let decimals = Pallet::<T>::retrieve_decimals((*asset).into())?;
				Some(AssetReserve {
					amount: reserve,
					decimals,
				})
			})
			.collect()
	}
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo, Default)]
pub struct AssetAmount<AssetId> {
	pub asset_id: AssetId,
	pub amount: Balance,
}

impl<AssetId: Default> AssetAmount<AssetId> {
	pub fn new(asset_id: AssetId, amount: Balance) -> Self {
		Self { asset_id, amount }
	}
}

impl<AssetId> From<AssetAmount<AssetId>> for u128 {
	fn from(value: AssetAmount<AssetId>) -> Self {
		value.amount
	}
}
impl<AssetId> From<&AssetAmount<AssetId>> for u128 {
	fn from(value: &AssetAmount<AssetId>) -> Self {
		value.amount
	}
}

bitflags::bitflags! {
	/// Indicates whether asset can be bought or sold to/from Omnipool and/or liquidity added/removed.
	#[derive(Encode,Decode, MaxEncodedLen, TypeInfo)]
	pub struct Tradability: u8 {
		/// Asset is frozen. No operations are allowed.
		const FROZEN = 0b0000_0000;
		/// Asset is allowed to be sold into stable pool
		const SELL = 0b0000_0001;
		/// Asset is allowed to be bought into stable pool
		const BUY = 0b0000_0010;
		/// Adding liquidity of asset is allowed
		const ADD_LIQUIDITY = 0b0000_0100;
		/// Removing liquidity of asset is not allowed
		const REMOVE_LIQUIDITY = 0b0000_1000;
	}
}

impl Default for Tradability {
	fn default() -> Self {
		Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AssetId> {
	fn register_asset(asset_id: AssetId, decimals: u8) -> DispatchResult;
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PoolState<AssetId> {
	pub assets: Vec<AssetId>,
	pub before: Vec<Balance>,
	pub after: Vec<Balance>,
	pub delta: Vec<Balance>,
	pub issuance_before: Balance,
	pub issuance_after: Balance,
	pub share_prices: Vec<(Balance, Balance)>,
}

/// Interface for populating oracle from stableswap, and getting their weights
pub trait StableswapHooks<AssetId> {
	fn on_liquidity_changed(pool_id: AssetId, state: PoolState<AssetId>) -> DispatchResult;
	fn on_trade(pool_id: AssetId, asset_in: AssetId, asset_out: AssetId, state: PoolState<AssetId>) -> DispatchResult;

	fn on_liquidity_changed_weight(n: usize) -> Weight;
	fn on_trade_weight(n: usize) -> Weight;
}

impl<AssetId> StableswapHooks<AssetId> for () {
	fn on_liquidity_changed(_pool_id: AssetId, _state: PoolState<AssetId>) -> DispatchResult {
		Ok(())
	}

	fn on_trade(
		_pool_id: AssetId,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_state: PoolState<AssetId>,
	) -> DispatchResult {
		Ok(())
	}

	fn on_liquidity_changed_weight(_n: usize) -> Weight {
		Weight::zero()
	}

	fn on_trade_weight(_n: usize) -> Weight {
		Weight::zero()
	}
}
