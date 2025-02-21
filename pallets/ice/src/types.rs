use codec::{Decode, Encode};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
pub use pallet_intent::types::{AssetId, Balance, Intent, IntentId, ResolvedIntent};
use sp_std::collections::btree_map::BTreeMap;

/// The reason for invalid solution.
#[derive(Encode, Decode, Eq, PartialEq, TypeInfo, frame_support::PalletError, RuntimeDebug)]
pub enum Reason {
	Empty,
	Score,
	IntentNotFound,
	IntentAmount,
	IntentPartialAmount,
	IntentPrice,
}

pub(crate) struct Solution<AccountId> {
	pub transfers_in: Vec<(AccountId, AssetId, Balance)>,
	pub transfers_out: Vec<(AccountId, AssetId, Balance)>,
	pub amounts: BTreeMap<AssetId, (Balance, Balance)>,
}
