use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use ice_support::{IntentData, IntentDataInput};
use sp_runtime::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 512;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

/// User-facing intent for extrinsic submission.
/// Uses IntentDataInput which excludes internal DCA state fields.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, DecodeWithMemTracking, TypeInfo)]
pub struct IntentInput {
	pub data: IntentDataInput,
	pub deadline: Option<Moment>,
	pub on_resolved: Option<CallData>,
}

/// Internal intent representation stored on-chain.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, DecodeWithMemTracking, TypeInfo)]
pub struct Intent {
	pub data: IntentData,
	pub deadline: Option<Moment>,
	pub on_resolved: Option<CallData>,
}
