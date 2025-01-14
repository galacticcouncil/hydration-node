use codec::{Decode, Encode};
use frame_support::pallet_prelude::{ConstU32, TypeInfo};
use frame_support::BoundedVec;

#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq, TypeInfo, Default)]
pub struct AssetAmount<AssetId> {
	pub asset_id: AssetId,
	pub amount: u128,
}

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

pub trait SubmitIntent<AccountId, AssetId> {
	type Error;
	fn submit_intent(
		who: &AccountId,
		asset_in: AssetAmount<AssetId>,
		asset_out: AssetAmount<AssetId>,
		deadline: u64,
		partial: bool,
		on_success: Option<CallData>,
		on_failure: Option<CallData>,
	) -> Result<u128, Self::Error>;
}
