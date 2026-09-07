use codec::Decode;
use codec::DecodeWithMemTracking;
use codec::Encode;
use codec::MaxEncodedLen;
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use frame_support::traits::ConstU32;
use frame_support::BoundedVec;
use primitives::{AssetId, Balance, EvmAddress};

pub type Identificator = u128;

/// Upper bound on the opaque, contract-decoded `data` carried by a forward action.
pub const MAX_FORWARD_DATA: u32 = 512;

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, DecodeWithMemTracking, MaxEncodedLen)]
pub enum Source {
	ICE(Identificator),
}

/// A resolved intent's post-action: forward the resolved output to a user-named EVM contract.
///
/// Built by the intent pallet at resolution time (one per executed trade) from the user's
/// `OnResolved::Forward` plus the resolved amounts, and handed to the lazy-executor to run.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, DecodeWithMemTracking, MaxEncodedLen)]
pub struct ForwardAction {
	pub contract: EvmAddress,
	pub intent_id: u128,
	pub asset_in: AssetId,
	pub amount_in: Balance,
	pub asset_out: AssetId,
	pub amount_out: Balance,
	pub data: BoundedVec<u8, ConstU32<MAX_FORWARD_DATA>>,
}

pub trait Mutate<AccountId> {
	type Error;

	/// Queue `forward` to be executed lazily on behalf of `origin`.
	fn queue(src: Source, origin: AccountId, forward: ForwardAction) -> Result<(), Self::Error>;
}
