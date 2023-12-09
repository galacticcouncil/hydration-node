pub trait SpotPriceProvider<AssetId> {
	type Price;

	fn pair_exists(asset_a: AssetId, asset_b: AssetId) -> bool;

	/// Return spot price for given asset pair.
	///
	/// Returns price of the `asset_b` denominated in `asset_a` ( `asset_a / asset_b` ).
	/// Example: `spot_price(DAI, LRNA) == 25` (you get 25 DAI for each LRNA).
	///
	/// Returns `None` if such pair does not exist.
	fn spot_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price>;
}

/// Manage list of non-dustable accounts
pub trait DustRemovalAccountWhitelist<AccountId> {
	type Error;

	/// Add account to the list.
	fn add_account(account: &AccountId) -> Result<(), Self::Error>;

	/// Remove an account from the list.
	fn remove_account(account: &AccountId) -> Result<(), Self::Error>;
}
