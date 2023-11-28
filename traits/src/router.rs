use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::sp_runtime::{DispatchError, DispatchResult};
use frame_support::weights::Weight;
use scale_info::TypeInfo;
use sp_std::vec;
use sp_std::vec::Vec;

#[derive(Debug, Encode, Decode, Copy, Clone, PartialOrd, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
pub struct AssetPair<AssetId> {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

impl<AssetId> AssetPair<AssetId> {
	pub fn new(asset_in: AssetId, asset_out: AssetId) -> Self {
		Self { asset_in, asset_out }
	}

	/// Return ordered asset tuple (A,B) where A < B
	/// Used in storage
	pub fn ordered_pair(&self) -> AssetPair<AssetId>
	where
		AssetId: PartialOrd + Copy,
	{
		match self.is_ordered() {
			true => AssetPair::new(self.asset_in, self.asset_out),
			false => AssetPair::new(self.asset_out, self.asset_in),
		}
	}

	pub fn is_ordered(&self) -> bool
	where
		AssetId: PartialOrd,
	{
		self.asset_in <= self.asset_out
	}

	pub fn to_ordered_vec(&self) -> Vec<AssetId>
	where
		AssetId: PartialOrd + Copy,
	{
		let pair = self.ordered_pair();
		vec![pair.asset_in, pair.asset_out]
	}
}

pub trait RouteProvider<AssetId> {
	fn get_route(asset_pair: AssetPair<AssetId>) -> Vec<Trade<AssetId>> {
		vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: asset_pair.asset_in,
			asset_out: asset_pair.asset_out,
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

pub fn inverse_route<AssetId>(trades: Vec<Trade<AssetId>>) -> Vec<Trade<AssetId>> {
	trades
		.into_iter()
		.map(|trade| Trade {
			pool: trade.pool,
			asset_in: trade.asset_out,
			asset_out: trade.asset_in,
		})
		.collect::<Vec<Trade<AssetId>>>()
		.into_iter()
		.rev()
		.collect()
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

	fn set_route(origin: Origin, asset_pair: AssetPair<AssetId>, route: Vec<Trade>) -> DispatchResultWithPostInfo;
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

	fn get_liquidity_depth(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>>;
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

	fn get_liquidity_depth(
		pool_type: PoolType<AssetId>,
		asset_a: AssetId,
		asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		for_tuples!(
			#(
				let value = match Tuple::get_liquidity_depth(pool_type,asset_a, asset_b){
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
	fn set_route_weight(route: &[Trade]) -> Weight;
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
	fn set_route_weight(_route: &[Trade]) -> Weight {
		Weight::zero()
	}
}
