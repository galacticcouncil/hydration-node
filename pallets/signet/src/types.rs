use frame_support::weights::Weight;

pub trait WeightInfo {
	fn initialize() -> Weight;
	fn update_deposit() -> Weight;
	fn withdraw_funds() -> Weight;
	fn sign() -> Weight;
	fn sign_respond() -> Weight;
	fn respond() -> Weight;
	fn respond_error() -> Weight;
	fn read_respond() -> Weight;
}
