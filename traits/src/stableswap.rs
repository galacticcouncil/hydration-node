use codec::{Decode, Encode};
use frame_support::__private::DispatchError;
use frame_support::pallet_prelude::TypeInfo;
use sp_std::vec::Vec;

pub trait StableswapAddLiquidity<AccountId, AssetId, Balance> {
	fn add_liquidity(
		who: AccountId,
		pool_id: AssetId,
		assets_amounts: Vec<AssetAmount<AssetId>>,
	) -> Result<Balance, DispatchError>;
}

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo)]
pub struct AssetAmount<AssetId> {
	pub asset_id: AssetId,
	pub amount: u128,
}

impl<AssetId: Default> AssetAmount<AssetId> {
	pub fn new(asset_id: AssetId, amount: u128) -> Self {
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
