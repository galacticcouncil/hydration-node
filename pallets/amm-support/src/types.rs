use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::sp_runtime::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_arithmetic::FixedU128;
use sp_std::vec;
use sp_std::vec::Vec;



#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum AssetType<AssetId, NFTId> {
	Fungible(AssetId),
	NFT(NFTId),
}

pub type OtcOrderId = u32;

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum Filler<AssetId, OtcOrderId> {
	Omnipool,
	Stableswap(AssetId), // pool id
	XYK(AssetId),        // share token
	LBP,
	OTC(OtcOrderId),
	XcmExchange, //TODO: do we need some info?
	Xcm(Option<[u8; 32]>), //TODO: VERIFY
	             // ICE(solution_id/block id),      swapper: alice, filler: solver
}

pub trait ExecutionTypeStack<IncrementalId> {
	fn push(execution_type: ExecutionType<IncrementalId>) -> DispatchResult;
	fn pop() -> Result<ExecutionType<IncrementalId>, DispatchError>;
	fn get() -> Vec<ExecutionType<IncrementalId>>;
	fn clear();
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Fee<AssetId, Balance, AccountId> {
	pub asset: AssetId,
	pub amount: Balance,
	pub recipient: AccountId,
}
impl<AssetId, Balance, AccountId> Fee<AssetId, Balance, AccountId> {
	pub fn new(asset: AssetId, amount: Balance, recipient: AccountId) -> Self {
		Self {
			asset,
			amount,
			recipient,
		}
	}
}

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum ExecutionType<IncrementalId> {
	Router(IncrementalId),
	DCA(IncrementalId), //We might need schedule id. How about otc?
	ICE(IncrementalId),
	Batch(IncrementalId),
	Omnipool(IncrementalId),
	XcmExchange(IncrementalId),
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum TradeOperation {
	ExactIn,
	ExactOut,
	Limit,
	LiquidityAdd,
	LiquidityRemove,
}
