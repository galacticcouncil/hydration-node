#![allow(clippy::upper_case_acronyms)]

use frame_support::dispatch;
use frame_support::dispatch::DispatchResult;
use sp_std::vec::Vec;

/// Hold information to perform amm transfer
/// Contains also exact amount which will be sold/bought
pub struct AMMTransfer<AccountId, AssetPair, Balance> {
	pub origin: AccountId,
	pub assets: AssetPair,
	pub amount: Balance,
	pub amount_out: Balance,
	pub discount: bool,
	pub discount_amount: Balance,
}

/// Traits for handling AMM Pool trades.
pub trait AMM<AccountId, AssetId, AssetPair, Amount> {
	/// Check if a pool for asset_a and asset_b exists.
	fn exists(assets: AssetPair) -> bool;

	/// Return pair account.
	fn get_pair_id(assets: AssetPair) -> AccountId;

	/// Return list of active assets in a given pool.
	fn get_pool_assets(pool_account_id: &AccountId) -> Option<Vec<AssetId>>;

	/// Calculate spot price for asset a and b.
	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Amount) -> Amount;

	/// SELL
	/// Perform all necessary checks to validate an intended sale.
	fn validate_sell(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		min_bought: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetPair, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_sell(transfer: &AMMTransfer<AccountId, AssetPair, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	/// Call execute following the validation.
	fn sell(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		min_bought: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_sell(&Self::validate_sell(origin, assets, amount, min_bought, discount)?)?;
		Ok(())
	}

	/// BUY
	/// Perform all necessary checks to validate an intended buy.
	fn validate_buy(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		max_limit: Amount,
		discount: bool,
	) -> Result<AMMTransfer<AccountId, AssetPair, Amount>, frame_support::sp_runtime::DispatchError>;

	/// Execute buy for given validated transfer.
	fn execute_buy(transfer: &AMMTransfer<AccountId, AssetPair, Amount>) -> dispatch::DispatchResult;

	/// Perform asset swap.
	fn buy(
		origin: &AccountId,
		assets: AssetPair,
		amount: Amount,
		max_limit: Amount,
		discount: bool,
	) -> dispatch::DispatchResult {
		Self::execute_buy(&Self::validate_buy(origin, assets, amount, max_limit, discount)?)?;
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

pub trait CurrencySwap<AccountId, Balance> {
	fn swap_currency(who: &AccountId, fee: Balance) -> DispatchResult;
}
