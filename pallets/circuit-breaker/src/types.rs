use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;

#[derive(Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
pub enum AssetLockdownState<BlockNumber, Balance> {
	Locked(BlockNumber),
	Unlocked((BlockNumber, Balance)),
}
