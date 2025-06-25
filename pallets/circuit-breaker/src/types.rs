use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;

/// Represents if the asset is locked down or not, untill a specific block number.
/// If unlocked, it contains the last block number and the baseline issuance for the given period
#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
pub enum AssetLockdownState<BlockNumber, Balance> {
	Locked(BlockNumber),
	Unlocked((BlockNumber, Balance)),
}
