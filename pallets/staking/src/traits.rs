use crate::types::Balance;
use frame_support::dispatch::DispatchResult;
use sp_runtime::FixedU128;

pub trait PayablePercentage<Point> {
	/// Returns percentage to pay based of amount of points.
	fn get(points: Point) -> Option<FixedU128>;
}

pub trait GetReferendumState<Index> {
	fn is_referendum_finished(index: Index) -> bool;
}

pub(crate) trait ActionData {
	fn amount(&self) -> Balance;
	fn conviction(&self) -> FixedU128;
}

pub trait Freeze<AccountId, CollectionId> {
	/// Freezes given item so it is not transferable.
	fn freeze_collection(owner: AccountId, collection: CollectionId) -> DispatchResult;
}

pub trait VestingDetails<AccountId, Balance> {
	/// Returns vested amount for who.
	fn locked(who: AccountId) -> Balance;
}

/// Sum of HDX claimed by other pallets that must not back a legacy stake.
/// The runtime decides which lock ids count (e.g. `ghdxlock`) and which are
/// allowed to overlap (e.g. `pyconvot`). Returning > 0 reduces `stakeable`
/// by that amount, so the user cannot legacy-stake HDX that is already
/// pledged elsewhere.
pub trait ExternalClaims<AccountId> {
	fn on(who: &AccountId) -> Balance;
}

impl<AccountId> ExternalClaims<AccountId> for () {
	fn on(_who: &AccountId) -> Balance {
		0
	}
}
