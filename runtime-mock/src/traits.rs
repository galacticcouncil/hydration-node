pub type Balance = u128;

pub trait FuzzedPallet<Call, AssetId, AccountId> {
	fn initial_calls(&self) -> Vec<Call>;
	fn native_endowed_accounts(&self) -> Vec<(AccountId, Balance)>;
	fn foreign_endowed_accounts(&self) -> Vec<(AccountId, Vec<(AssetId, Balance)>)>;
}

pub trait Loader {
	fn load_setup(filename: &str) -> Self;
}
