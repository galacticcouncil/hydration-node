use crate::types::Balance;
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
