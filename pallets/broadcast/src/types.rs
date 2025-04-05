use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type AssetId = u32;
pub type Balance = u128;
pub type IncrementalIdType = u32;
pub type OtcOrderId = u32;
pub type ScheduleId = u32;

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum Filler {
	Omnipool,
	Stableswap(AssetId), // pool id
	XYK(AssetId),        // share token
	LBP,
	OTC(OtcOrderId),
	AAVE, // ICE(solution_id/block id),      swapper: alice, filler: solver
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum Destination<AccountId> {
	Account(AccountId),
	Burned,
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Fee<AccountId> {
	pub asset: AssetId,
	pub amount: Balance,
	pub destination: Destination<AccountId>,
}
impl<AccountId> Fee<AccountId> {
	pub fn new(asset: AssetId, amount: Balance, destination: Destination<AccountId>) -> Self {
		Self {
			asset,
			amount,
			destination,
		}
	}
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Asset {
	pub asset: AssetId,
	pub amount: Balance,
}
impl Asset {
	pub fn new(asset: AssetId, amount: Balance) -> Self {
		Self { asset, amount }
	}
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum ExecutionType {
	Router(IncrementalIdType),
	DCA(ScheduleId, IncrementalIdType),
	Batch(IncrementalIdType),
	Omnipool(IncrementalIdType),
	XcmExchange(IncrementalIdType),
	Xcm([u8; 32], IncrementalIdType),
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum TradeOperation {
	ExactIn,
	ExactOut,
	Limit,
	LiquidityAdd,
	LiquidityRemove,
}
