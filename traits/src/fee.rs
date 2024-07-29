use frame_support::sp_runtime::{DispatchError, DispatchResult};

pub trait InspectSufficiency<AssetId> {
	fn is_sufficient(asset: AssetId) -> bool;

	fn is_trade_supported(from: AssetId, into: AssetId) -> bool;
}

//TODO: give better name
pub trait InsufficientAssetTrader<AccountId, AssetId, Amount>: InspectSufficiency<AssetId> {
	fn buy(
		origin: &AccountId,
		dest: &AccountId,
		from: AssetId,
		into: AssetId,
		amount: Amount,
		max_limit: Amount,
	) -> DispatchResult;

	fn pool_trade_fee(swap_amount: Amount) -> Result<Amount, DispatchError>;

	fn get_amount_in_for_out(
		insuff_asset_id: AssetId,
		asset_out: AssetId,
		asset_out_amount: Amount,
	) -> Result<Amount, DispatchError>;
}
