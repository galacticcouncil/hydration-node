use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{ConstU32, TypeInfo};
use frame_support::BoundedVec;
use sp_arithmetic::Permill;
use sp_std::vec::Vec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

pub trait CallExecutor<AccountId> {
	fn execute(who: AccountId, ident: u128, call: CallData) -> DispatchResult;
}

impl<AccountId> CallExecutor<AccountId> for () {
	fn execute(_who: AccountId, _ident: u128, _call: CallData) -> DispatchResult {
		Ok(())
	}
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct OmnipoolAssetInfo<AssetId> {
	pub asset_id: AssetId,
	pub reserve: u128,
	pub hub_reserve: u128,
	pub decimals: u8,
	pub fee: Permill,
	pub hub_fee: Permill,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StableswapAssetInfo<AssetId> {
	pub pool_id: AssetId,
	pub asset_id: AssetId,
	pub reserve: u128,
	pub decimals: u8,
	pub fee: Permill,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum AssetInfo<AssetId> {
	Omnipool(OmnipoolAssetInfo<AssetId>),
	StableSwap(StableswapAssetInfo<AssetId>),
}

/// Trait to gather all Hydration AMM information - each pool, each asset
pub trait AmmState<AssetId> {
	fn state<F: Fn(&AssetId) -> bool>(retain: F) -> Vec<AssetInfo<AssetId>>;
}
