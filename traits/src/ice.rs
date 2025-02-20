use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{ConstU32, TypeInfo};
use frame_support::BoundedVec;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type CallData = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

pub trait CallExecutor<AccountId> {
	fn execute(who: AccountId, ident: u128, call: CallData) -> DispatchResult;
}
