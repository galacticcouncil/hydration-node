use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use scale_info::TypeInfo;
use sp_std::vec;
use sp_std::vec::Vec;
pub trait RouteProvider<AssetId> {
	fn get(asset_in: AssetId, asset_out: AssetId) -> Vec<Trade<AssetId>> {
		vec![Trade {
			pool: PoolType::Omnipool,
			asset_in,
			asset_out,
		}]
	}
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum PoolType<AssetId> {
	XYK,
	LBP,
	Stableswap(AssetId),
	Omnipool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExecutorError<E> {
	NotSupported,
	Error(E),
}

///A single trade for buy/sell, describing the asset pair and the pool type in which the trade is executed
#[derive(Encode, Decode, Debug, Eq, PartialEq, Copy, Clone, TypeInfo, MaxEncodedLen)]
pub struct Trade<AssetId> {
	pub pool: PoolType<AssetId>,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

#[derive(Debug, PartialEq)]
pub struct AmountInAndOut<Balance> {
	pub amount_in: Balance,
	pub amount_out: Balance,
}

pub trait RouterT<Origin, AssetId, Balance, Trade, AmountInAndOut> {
	fn sell(
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		route: Vec<Trade>,
	) -> DispatchResult;

	fn buy(
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		route: Vec<Trade>,
	) -> DispatchResult;

	fn calculate_sell_trade_amounts(route: &[Trade], amount_in: Balance) -> Result<Vec<AmountInAndOut>, DispatchError>;

	fn calculate_buy_trade_amounts(route: &[Trade], amount_out: Balance) -> Result<Vec<AmountInAndOut>, DispatchError>;
}

/// All AMMs used in the router are required to implement this trait.
pub trait TradeExecution<Origin, AccountId, AssetId, Balance> {
	type Error;

	fn calculate_sell(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>>;

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>>;

	fn execute_sell(
		who: Origin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>>;

	fn execute_buy(
		who: Origin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>>;
}

#[allow(clippy::redundant_clone)] //Needed as it complains about redundant clone, but clone is needed as Origin is moved and it is not copy type.
#[impl_trait_for_tuples::impl_for_tuples(1, 5)]
impl<E: PartialEq, Origin: Clone, AccountId, AssetId: Copy, Balance: Copy>
	TradeExecution<Origin, AccountId, AssetId, Balance> for Tuple
{
	for_tuples!( where #(Tuple: TradeExecution<Origin,AccountId, AssetId, Balance, Error=E>)*);
	type Error = E;

	fn calculate_sell(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		for_tuples!(
			#(
				let value = match Tuple::calculate_sell(pool_type, asset_in,asset_out,amount_in) {
					Ok(result) => return Ok(result),
					Err(v) if v == ExecutorError::NotSupported => v,
					Err(v) => return Err(v),
				};
			)*
		);
		Err(value)
	}

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		for_tuples!(
			#(
				let value = match Tuple::calculate_buy(pool_type, asset_in,asset_out,amount_out) {
					Ok(result) => return Ok(result),
					Err(v) if v == ExecutorError::NotSupported => v,
					Err(v) => return Err(v),
				};
			)*
		);
		Err(value)
	}

	fn execute_sell(
		who: Origin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		for_tuples!(
			#(
				let value = match Tuple::execute_sell(who.clone(),pool_type, asset_in, asset_out, amount_in, min_limit) {
					Ok(result) => return Ok(result),
					Err(v) if v == ExecutorError::NotSupported => v,
					Err(v) => return Err(v),
				};
			)*
		);
		Err(value)
	}

	fn execute_buy(
		who: Origin,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		for_tuples!(
			#(
				let value = match Tuple::execute_buy(who.clone(), pool_type,asset_in, asset_out, amount_out, max_limit) {
					Ok(result) => return Ok(result),
					Err(v) if v == ExecutorError::NotSupported => v,
					Err(v) => return Err(v),
				};
			)*
		);
		Err(value)
	}
}

/// Provides weight info for the router. Calculates the weight of a route based on the AMMs.
pub trait AmmTradeWeights<Trade> {
	fn sell_weight(route: &[Trade]) -> Weight;
	fn buy_weight(route: &[Trade]) -> Weight;
	fn calculate_buy_trade_amounts_weight(route: &[Trade]) -> Weight;
	fn sell_and_calculate_sell_trade_amounts_weight(route: &[Trade]) -> Weight;
	fn buy_and_calculate_buy_trade_amounts_weight(route: &[Trade]) -> Weight;
}

impl<Trade> AmmTradeWeights<Trade> for () {
	fn sell_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
	fn buy_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
	fn calculate_buy_trade_amounts_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
	fn sell_and_calculate_sell_trade_amounts_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
	fn buy_and_calculate_buy_trade_amounts_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
}
