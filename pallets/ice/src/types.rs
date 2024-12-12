use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::{RuntimeDebug, TypeInfo};
use frame_support::traits::ConstU32;
use hydradx_traits::router::Trade;
use sp_runtime::traits::Convert;
use sp_runtime::BoundedVec;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub const MAX_RESOLVED_INTENTS: u32 = 20_000;
pub const MAX_INSTRUCTIONS: u32 = 50_000;

pub type NamedReserveIdentifier = [u8; 8];
pub type AssetId = u32;
pub type Balance = u128;
pub type Moment = u64;
pub type IncrementalIntentId = u64;
pub type IntentId = u128;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;
pub type BoundedResolvedIntents = BoundedVec<ResolvedIntent, ConstU32<MAX_RESOLVED_INTENTS>>;
pub type BoundedInstructions<AccountId, AssetId> =
	BoundedVec<Instruction<AccountId, AssetId>, ConstU32<MAX_INSTRUCTIONS>>;

// Unfortunately, we need simple representations of the types to be able to use across the FFI
// dev: perhaps, it could be possible to implement IntoFFIValue to simplify.
pub type IntentRepr = (IntentId, AssetId, AssetId, Balance, Balance);
pub type DataRepr = (u8, AssetId, Balance, Balance, u8, (u32, u32), (u32, u32));

pub type BoundedRoute<AssetId> = BoundedVec<Trade<AssetId>, ConstU32<5>>;

pub type BoundedTrades<AssetId> = BoundedVec<TradeInstruction<AssetId>, ConstU32<MAX_INSTRUCTIONS>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Intent<AccountId> {
	pub who: AccountId,
	pub swap: Swap,
	pub deadline: Moment,
	pub partial: bool,
	pub on_success: Option<CallData>,
	pub on_failure: Option<CallData>,
}

impl<AccountId> Into<IntentRepr> for Intent<AccountId> {
	fn into(self) -> IntentRepr {
		(
			0,
			self.swap.asset_in,
			self.swap.asset_out,
			self.swap.amount_in,
			self.swap.amount_out,
		)
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Swap {
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

pub(crate) struct SolutionAmounts<AssetId> {
	pub(crate) amounts_in: BTreeMap<AssetId, Balance>,
	pub(crate) amounts_out: BTreeMap<AssetId, Balance>,
}
