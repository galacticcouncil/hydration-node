use sp_runtime::FixedU128;

pub trait PayablePercentage<Point> {
	type Error;

	/// Returns percentage to pay based of amount of points.
	fn get(points: Point) -> Result<FixedU128, Self::Error>;
}
