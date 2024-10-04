use frame_support::sp_runtime::{DispatchError, DispatchResult};

///Checking if asset is an accepted transaction fee currency
pub trait InspectTransactionFeeCurrency<AssetId> {
	fn is_transaction_fee_currency(asset: AssetId) -> bool;
}

///Enabling trading of assets that are swappable but not part of AcceptedCurrencies of multi payment pallet
pub trait SwappablePaymentAssetTrader<AccountId, AssetId, Amount>: InspectTransactionFeeCurrency<AssetId> {
	fn is_trade_supported(from: AssetId, into: AssetId) -> bool;

	fn calculate_fee_amount(swap_amount: Amount) -> Result<Amount, DispatchError>;

	fn calculate_in_given_out(
		insuff_asset_id: AssetId,
		asset_out: AssetId,
		asset_out_amount: Amount,
	) -> Result<Amount, DispatchError>;

	fn calculate_out_given_in(
		asset_in: AssetId,
		asset_in_amount: Amount,
		asset_out: AssetId,
	) -> Result<Amount, DispatchError>;

	fn buy(
		origin: &AccountId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Amount,
		max_limit: Amount,
		dest: &AccountId,
	) -> DispatchResult;
}
