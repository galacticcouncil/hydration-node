use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::RuntimeDebug;
use primitives::Balance;
use scale_info::TypeInfo;

/// An unstake position after giga-unstake.
/// HDX is locked in the user's account until `unlock_at`. All positions for a
/// given account share a single aggregate lock identifier — the pallet sums
/// active position amounts and updates the lock on every stake/unlock.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct UnstakePosition<BlockNumber> {
	/// HDX amount locked.
	pub amount: Balance,
	/// Block number when cooldown expires and HDX can be unlocked.
	pub unlock_at: BlockNumber,
}
