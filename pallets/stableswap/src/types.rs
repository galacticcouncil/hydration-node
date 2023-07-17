#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, MAX_ASSETS_IN_POOL};
use sp_runtime::Permill;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::num::NonZeroU16;
use sp_std::prelude::*;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::ConstU32;
use frame_support::BoundedVec;
use orml_traits::MultiCurrency;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

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
	pub trade_fee: Permill,
	pub withdraw_fee: Permill,
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
	pub fn find_asset(&self, asset: AssetId) -> Option<usize> {
		self.assets.iter().position(|v| *v == asset)
	}

	pub(crate) fn is_valid(&self) -> bool {
		self.assets.len() >= 2 && has_unique_elements(&mut self.assets.iter())
	}

	pub fn balances<T: Config>(&self, account: &T::AccountId) -> Vec<Balance>
	where
		T::AssetId: From<AssetId>,
	{
		self.assets
			.iter()
			.map(|asset| T::Currency::free_balance((*asset).into(), account))
			.collect()
	}
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo)]
pub struct AssetBalance<AssetId> {
	pub asset_id: AssetId,
	pub amount: Balance,
}

bitflags::bitflags! {
	/// Indicates whether asset can be bought or sold to/from Omnipool and/or liquidity added/removed.
	#[derive(Encode,Decode, MaxEncodedLen, TypeInfo)]
	pub struct Tradability: u8 {
		/// Asset is frozen. No operations are allowed.
		const FROZEN = 0b0000_0000;
		/// Asset is allowed to be sold into omnipool
		const SELL = 0b0000_0001;
		/// Asset is allowed to be bought into omnipool
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
