use sp_runtime::FixedU128;
use pallet_democracy::ReferendumIndex;

pub trait PayablePercentage<Point> {
	type Error;

	/// Returns percentage to pay based of amount of points.
	fn get(points: Point) -> Result<FixedU128, Self::Error>;
}

pub trait DemocracyReferendum{
	fn is_referendum_finished(index: ReferendumIndex) -> bool;
}