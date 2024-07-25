use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::TypeInfo;

#[derive(Debug, Encode, Decode, Copy, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum SkipEd {
	Lock,
	LockAndUnlock,
	Unlock,
}
