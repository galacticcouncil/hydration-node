use codec::{Decode, Encode};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
pub use pallet_intent::types::{AssetId, Balance};
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
	pub transfers_in: Vec<Instruction<AccountId>>,
	pub transfers_out: Vec<Instruction<AccountId>>,
	pub amounts: BTreeMap<AssetId, (Balance, Balance)>,
}

impl<AccountId> Default for Solution<AccountId> {
	fn default() -> Self {
		Self {
			transfers_in: Vec::new(),
			transfers_out: Vec::new(),
			amounts: Default::default(),
		}
	}
}

#[derive(Debug)]
pub enum Instruction<AccountId> {
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
}
