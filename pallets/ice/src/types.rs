use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::TypeInfo;
use frame_support::pallet_prelude::RuntimeDebug;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Solution{

}