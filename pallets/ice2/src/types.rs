use codec::{Decode, Encode};
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::TypeInfo;
use sp_std::collections::btree_map::BTreeMap;

pub type AssetId = u32;
pub type Balance = u128;

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
	transfers_in: Vec<Instruction<AccountId>>,
	transfers_out: Vec<Instruction<AccountId>>,
	amounts_in: BTreeMap<AssetId, Balance>,
	amounts_out: BTreeMap<AssetId, Balance>,
}

impl<AccountId> Default for Solution<AccountId> {
	fn default() -> Self {
		Self {
			transfers_in: Vec::new(),
			transfers_out: Vec::new(),
			amounts_in: Default::default(),
			amounts_out: Default::default(),
		}
	}
}

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
