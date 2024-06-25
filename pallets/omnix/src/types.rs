use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::ConstU32;
use sp_runtime::BoundedVec;

const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;

pub type Balance = u128;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type IntentId = u128;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent<AccountId, AssetId> {
	pub(crate) who: AccountId,
	pub(crate) swap: Swap<AssetId>,
	pub(crate) deadline: Moment,
	pub(crate) partial: bool,
	pub(crate) on_success: Option<CallData>,
	pub(crate) on_failure: Option<CallData>,
	//TODO: nonce?!
	// nonce: Nonce,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Swap<AssetId> {
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	swap_type: SwapType,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SwapType {
	ExactInput,
	ExactOutput,
}
