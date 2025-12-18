use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use sp_runtime::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type AssetId = u32;
pub type Balance = u128;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type IntentId = u128;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum IntentKind {
	Swap(SwapData),
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent {
	pub kind: IntentKind,
	pub deadline: Moment,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct SwapData {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub swap_type: SwapType,
	pub partial: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SwapType {
	ExactIn,
	ExactOut,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ExecutedIntent<AccountId> {
	pub id: IntentId,
	pub owner: AccountId,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
}
