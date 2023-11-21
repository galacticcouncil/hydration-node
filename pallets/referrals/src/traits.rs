pub trait Convert<AccountId, AssetId, Balance> {
	type Error;

	fn convert(who: AccountId, asset_from: AssetId, asset_to: AssetId, amount: Balance)
		-> Result<Balance, Self::Error>;
}
