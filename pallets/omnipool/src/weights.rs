use frame_support::weights::Weight;

/// Weight functions needed for claims.
pub trait WeightInfo {
	fn add_token() -> Weight;
}
