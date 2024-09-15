use crate::engine::Instruction;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::ConstU32;
use frame_support::weights::Weight;
use sp_runtime::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub const MAX_RESOLVED_INTENTS: u32 = 128;
pub const MAX_PRICES: u32 = 128;
pub const MAX_INSTRUCTIONS: u32 = 128;

pub type NamedReserveIdentifier = [u8; 8];
pub type Balance = u128;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type IntentId = u128;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;
pub type BoundedResolvedIntents = BoundedVec<ResolvedIntent, ConstU32<MAX_RESOLVED_INTENTS>>;
pub type BoundedInstructions<AccountId, AssetId> =
	BoundedVec<Instruction<AccountId, AssetId>, ConstU32<MAX_INSTRUCTIONS>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent<AccountId, AssetId> {
	pub who: AccountId,
	pub swap: Swap<AssetId>,
	pub deadline: Moment,
	pub partial: bool,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
	//TODO: nonce?!
	// nonce: Nonce,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Swap<AssetId> {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub swap_type: SwapType,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum SwapType {
	ExactIn,
	ExactOut,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ResolvedIntent {
	pub intent_id: IntentId,
	pub amount_in: Balance,
	pub amount_out: Balance,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Solution<AccountId, AssetId> {
	pub proposer: AccountId,
	pub intents: BoundedResolvedIntents,
	pub instructions: BoundedInstructions<AccountId, AssetId>,
	pub score: u64,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ProposedSolution<AccountId, AssetId> {
	pub intents: BoundedResolvedIntents,
	pub instructions: BoundedInstructions<AccountId, AssetId>,
}
