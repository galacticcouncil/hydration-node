use frame_support::pallet_prelude::Weight;

pub trait WeightInfo {
	fn submit_intent() -> Weight;
	fn submit_solution() -> Weight;
}
