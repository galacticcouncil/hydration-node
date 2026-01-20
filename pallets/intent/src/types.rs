use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use ice_support::IntentData;
use sp_runtime::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum CallbackType {
	OnSuccess,
	OnFailure,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent {
	pub data: IntentData,
	pub deadline: Moment,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
}
