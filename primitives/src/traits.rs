use frame_support::dispatch;
use sp_std::vec::Vec;

pub trait AMM<AccountId, AssetId, Amount> {
	fn exists(asset_a: AssetId, asset_b: AssetId) -> bool;

	fn get_pair_id(asset_a: &AssetId, asset_b: &AssetId) -> AccountId;

	fn get_pool_assets(pool_account_id: &AccountId) -> Option<(AssetId, AssetId)>;

	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: Amount) -> Amount;

	fn sell(
		origin: &AccountId,
		asset_sell: AssetId,
		asset_buy: AssetId,
		amount_sell: Amount,
		discount: bool,
	) -> dispatch::DispatchResult;

	fn buy(
		origin: &AccountId,
		asset_buy: AssetId,
		asset_sell: AssetId,
		amount_buy: Amount,
		discount: bool,
	) -> dispatch::DispatchResult;

	fn calculate_sell_price(
		sell_reserve: Amount,
		buy_reserve: Amount,
		sell_amount: Amount,
	) -> Result<Amount, dispatch::DispatchError>;

	fn calculate_buy_price(
		sell_reserve: Amount,
		buy_reserve: Amount,
		amount: Amount,
	) -> Result<Amount, dispatch::DispatchError>;

	fn calculate_spot_price(
		sell_reserve: Amount,
		buy_reserve: Amount,
		sell_amount: Amount,
	) -> Result<Amount, dispatch::DispatchError>;

	fn calculate_fees(amount: Amount, discount: bool, hdx_fee: &mut Amount) -> Result<Amount, dispatch::DispatchError>;
}

// Note: still not sure this is needed
pub trait DirectTrade<AccountId, AssetId, Amount> {
	fn transfer(from: &AccountId, to: &AccountId, asset: AssetId, amount: Amount) -> dispatch::DispatchResult;
}

pub trait Resolver<AccountId, Intention> {
	fn resolve_single_intention(intention: &Intention);
	fn resolve_intention(pair_account: &AccountId, intention: &Intention, matched: &Vec<Intention>) -> bool;
}

pub trait Matcher<AccountId, Intention> {
	fn group<'a>(
		pair_account: &AccountId,
		asset_a_sell: &'a Vec<Intention>,
		asset_b_sell: &'a Vec<Intention>,
	) -> Option<Vec<(Intention, Vec<Intention>)>>;
}
