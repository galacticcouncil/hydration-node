use frame_support::sp_runtime::{DispatchError, DispatchResult};

///Checking for asset sufficiency and tradeability for fee paying purposes
pub trait InspectSufficiency<AssetId> {
	fn is_sufficient(asset: AssetId) -> bool;

	fn is_trade_supported(from: AssetId, into: AssetId) -> bool;
}

///Insufficient asset trader for enabling insufficient fee support
pub trait InsufficientAssetTrader<AccountId, AssetId, Amount>: InspectSufficiency<AssetId> {
	fn buy(
		origin: &AccountId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Amount,
		max_limit: Amount,
		dest: &AccountId,
	) -> DispatchResult;

	fn calculate_fee_amount(swap_amount: Amount) -> Result<Amount, DispatchError>;

	fn calculate_in_given_out(
		insuff_asset_id: AssetId,
		asset_out: AssetId,
		asset_out_amount: Amount,
	) -> Result<Amount, DispatchError>;
}
