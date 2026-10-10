use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use hydradx_traits::lazy_executor::MAX_FORWARD_DATA;
use ice_support::{IntentData, IntentDataInput};
use primitives::EvmAddress;
use sp_runtime::BoundedVec;

pub type Moment = u64;
pub type IncrementalIntentId = u64;

/// Typed, append-only post-resolution action.
///
/// Replaces the former raw-`RuntimeCall` callback. On (per-trade) resolution the runtime pushes the
/// resolved output to `contract` and invokes its receiver interface, handing it `data` opaquely.
/// New variants may only be appended (index-stable); never reorder or remove.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, DecodeWithMemTracking, TypeInfo)]
pub enum OnResolved {
	Forward {
		contract: EvmAddress,
		data: BoundedVec<u8, ConstU32<MAX_FORWARD_DATA>>,
	},
}

/// User-facing intent for extrinsic submission.
/// Uses IntentDataInput which excludes internal DCA state fields.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, DecodeWithMemTracking, TypeInfo)]
pub struct IntentInput {
	pub data: IntentDataInput,
	pub deadline: Option<Moment>,
	pub on_resolved: Option<OnResolved>,
}

/// Internal intent representation stored on-chain.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, DecodeWithMemTracking, TypeInfo)]
pub struct Intent {
	pub data: IntentData,
	pub deadline: Option<Moment>,
	pub on_resolved: Option<OnResolved>,
}
