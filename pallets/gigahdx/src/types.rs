use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::RuntimeDebug;
use frame_support::traits::LockIdentifier;
use primitives::Balance;
use scale_info::TypeInfo;

/// An unstake position after giga-unstake.
/// HDX is locked in the user's account until `unlock_at`.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct UnstakePosition<BlockNumber> {
	/// Lock identifier for this position (used to identify the HDX lock).
	pub lock_id: LockIdentifier,
	/// HDX amount locked.
	pub amount: Balance,
	/// Block number when cooldown expires and HDX can be unlocked.
	pub unlock_at: BlockNumber,
}
