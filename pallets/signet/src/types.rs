use frame_support::weights::Weight;

pub trait WeightInfo {
	fn set_config() -> Weight;
	fn withdraw_funds() -> Weight;
	fn sign() -> Weight;
	fn sign_bidirectional() -> Weight;
	fn respond() -> Weight;
	fn respond_error() -> Weight;
	fn respond_bidirectional() -> Weight;
	fn pause() -> Weight;
	fn unpause() -> Weight;
}
