use crate::types::Balance;
use frame_support::dispatch::DispatchResult;
use pallet_democracy::ReferendumIndex;
use sp_runtime::FixedU128;

pub trait PayablePercentage<Point> {
	type Error;

	/// Returns percentage to pay based of amount of points.
	fn get(points: Point) -> Result<FixedU128, Self::Error>;
}

pub trait DemocracyReferendum {
	fn is_referendum_finished(index: ReferendumIndex) -> bool;
}

pub(crate) trait ActionData {
	fn amount(&self) -> Balance;
	fn conviction(&self) -> u32;
}

pub trait Freeze<AccountId, CollectionId> {
	/// Freezes given item so it is not transferable.
	fn freeze_collection(owner: AccountId, collection: CollectionId) -> DispatchResult;
}

pub trait VestingDetails<AccountId, Balance> {
	/// Returns vested amount for who.
	fn locked(who: AccountId) -> Balance;
}
