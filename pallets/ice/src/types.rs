use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use hydradx_traits::router::Trade;
use sp_runtime::traits::Convert;
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

pub type BoundedRoute<AssetId> = BoundedVec<Trade<AssetId>, ConstU32<5>>;

pub type BoundedTrades<AssetId> = BoundedVec<TradeInstruction<AssetId>, ConstU32<MAX_INSTRUCTIONS>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent<AccountId, AssetId> {
	pub who: AccountId,
	pub swap: Swap<AssetId>,
	pub deadline: Moment,
	pub partial: bool,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
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
	pub intents: BoundedResolvedIntents,
	pub instructions: BoundedInstructions<AccountId, AssetId>,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum Instruction<AccountId, AssetId> {
	TransferIn {
		who: AccountId,
		asset_id: AssetId,
		amount: Balance,
	},
	TransferOut {
		who: AccountId,
		asset_id: AssetId,
		amount: Balance,
	},
	SwapExactIn {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedRoute<AssetId>,
	},
	SwapExactOut {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedRoute<AssetId>,
	},
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum TradeInstruction<AssetId> {
	SwapExactIn {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedRoute<AssetId>,
	},
	SwapExactOut {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		route: BoundedRoute<AssetId>,
	},
}

pub(crate) struct TradeInstructionTransform;
impl<AccountId, AssetId> Convert<BoundedTrades<AssetId>, BoundedInstructions<AccountId, AssetId>>
	for TradeInstructionTransform
{
	fn convert(trades: BoundedTrades<AssetId>) -> BoundedInstructions<AccountId, AssetId> {
		let t: Vec<Instruction<AccountId, AssetId>> =
			trades.into_iter().map(|trade| trade.transform::<AccountId>()).collect();
		BoundedInstructions::truncate_from(t)
	}
}

impl<AssetId> TradeInstruction<AssetId> {
	fn transform<A>(self) -> Instruction<A, AssetId> {
		match self {
			TradeInstruction::SwapExactIn {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				route,
			} => Instruction::SwapExactIn {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				route,
			},
			TradeInstruction::SwapExactOut {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				route,
			} => Instruction::SwapExactOut {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				route,
			},
		}
	}
}
