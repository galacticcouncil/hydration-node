use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use sp_runtime::DispatchResult;

/// Represents if the asset is locked down or not, untill a specific block number.
/// If unlocked, it contains the last block number and the baseline issuance for the given period
#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
pub enum LockdownStatus<BlockNumber, Balance> {
	Locked(BlockNumber),
	Unlocked((BlockNumber, Balance)),
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId, AssetId, Balance> {
	fn deposit(who: AccountId, asset_id: AssetId, amount: Balance) -> DispatchResult;

	fn register_asset(asset_id: AssetId, deposit_limit: Balance) -> DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId, AssetId, Balance> BenchmarkHelper<AccountId, AssetId, Balance> for () {
	fn deposit(_who: AccountId, _asset_id: AssetId, _amount: Balance) -> DispatchResult {
		Ok(())
	}

	fn register_asset(_asset_id: AssetId, _deposit_limit: Balance) -> DispatchResult {
		Ok(())
	}
}
