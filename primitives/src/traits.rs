use frame_support::dispatch;
use sp_std::vec::Vec;

/// Hold information to perform amm transfer
/// Contains also exact amount which will be sold/bought
pub struct AMMTransfer<AccountId, AssetId, Balance> {
	pub origin: AccountId,
	pub asset_sell: AssetId,
	pub asset_buy: AssetId,
	pub amount: Balance,
	pub amount_out: Balance,
	pub discount: bool,
	pub discount_amount: Balance,
}

/// Traits for handling AMM Pool trades.
pub trait AMM<AccountId, AssetId, Amount> {
	/// Check if a pool for asset_a and asset_b exists.
	fn exists(asset_a: AssetId, asset_b: AssetId) -> bool;

	/// Return pair account.
	fn get_pair_id(asset_a: &AssetId, asset_b: &AssetId) -> AccountId;

	/// Return list of active assets in a given pool.
	fn get_pool_assets(pool_account_id: &AccountId) -> Option<Vec<AssetId>>;

	/// Calculate spot price for asset a and b.
	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Amount) -> Amount;

	fn calculate_spot_price(
		sell_reserve: Amount,
		buy_reserve: Amount,
		sell_amount: Amount,
	) -> Result<Amount, dispatch::DispatchError>;

	/// SELL
	/// Perform all necessary checks to validate an intended sale.
	fn validate_sell(
		origin: &AccountId,
		asset_sell: AssetId,
		asset_buy: AssetId,
		amount_buy: Amount,
		min_bought: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetId, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_sell(transfer: &AMMTransfer<AccountId, AssetId, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	/// Call execute following the validation.
	fn sell(
		origin: &AccountId,
		asset_sell: AssetId,
		asset_buy: AssetId,
		amount_sell: Amount,
		min_bought: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_sell(&Self::validate_sell(
			origin,
			asset_sell,
			asset_buy,
			amount_sell,
			min_bought,
			discount,
		)?)?;
		Ok(())
	}

	/// BUY
	/// Perform all necessary checks to validate an intended buy.
	fn validate_buy(
		origin: &AccountId,
		asset_buy: AssetId,
		asset_sell: AssetId,
		amount_buy: Amount,
		max_limit: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetId, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_buy(transfer: &AMMTransfer<AccountId, AssetId, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	fn buy(
		origin: &AccountId,
		asset_buy: AssetId,
		asset_sell: AssetId,
		amount_buy: Amount,
		max_limit: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_buy(&Self::validate_buy(
			origin, asset_buy, asset_sell, amount_buy, max_limit, discount,
		)?)?;
		Ok(())
	}
}

pub trait Resolver<AccountId, Intention, E> {
	/// Resolve an intention directl via AMM pool.
	fn resolve_single_intention(intention: &Intention);

	/// Resolve intentions by either directly trading with each other or via AMM pool.
	/// Intention ```intention``` must be validated prior to call this function.
	fn resolve_matched_intentions(pair_account: &AccountId, intention: &Intention, matched: &[Intention]);
}
