use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type AssetId = u32;
pub type Balance = u128;

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum AssetType<NFTId> {
	Fungible(AssetId),
	NFT(NFTId),
}

pub type OtcOrderId = u32;

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum Filler {
	Omnipool,
	Stableswap(AssetId), // pool id
	XYK(AssetId),        // share token
	LBP,
	OTC(OtcOrderId),
	XcmExchange, //TODO: do we need some info?
	Xcm(Option<[u8; 32]>), //TODO: VERIFY
	             // ICE(solution_id/block id),      swapper: alice, filler: solver
}


#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Fee<AccountId> {
	pub asset: AssetId,
	pub amount: Balance,
	pub recipient: AccountId,
}
impl<AccountId> Fee<AccountId> {
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
