use frame_support::sp_runtime::{DispatchError, DispatchResult};

///Checking if asset is an accepted transaction fee currency
pub trait InspectTransactionFeeCurrency<AssetId> {
	fn is_transaction_fee_currency(asset: AssetId) -> bool;
}

///Enabling trading of assets that are swappable but not part of AcceptedCurrencies of multi payment pallet
pub trait SwappablePaymentAssetTrader<AccountId, AssetId, Balance>: InspectTransactionFeeCurrency<AssetId> {
	fn is_trade_supported(from: AssetId, into: AssetId) -> bool;

	fn calculate_fee_amount(swap_amount: Balance) -> Result<Balance, DispatchError>;

	fn calculate_in_given_out(
		insuff_asset_id: AssetId,
		asset_out: AssetId,
		asset_out_amount: Balance,
	) -> Result<Balance, DispatchError>;

	fn calculate_out_given_in(
		asset_in: AssetId,
		asset_out: AssetId,
		asset_in_amount: Balance,
	) -> Result<Balance, DispatchError>;

	fn buy(
		origin: &AccountId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		max_limit: Balance,
		dest: &AccountId,
	) -> DispatchResult;
}

pub trait GetDynamicFee<K> {
	type Fee;
	// Return a fee for a given key
	fn get(key: K) -> Self::Fee;
	// Return a fee for a given key and store it
	fn get_and_store(key: K) -> Self::Fee;
}
