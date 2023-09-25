use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

///A single trade for buy/sell, describing the asset pair and the pool type in which the trade is executed
#[derive(Encode, Decode, Debug, Eq, PartialEq, Copy, Clone, TypeInfo, MaxEncodedLen)]
pub struct Trade<AssetId> {
	pub pool: PoolType<AssetId>,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
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
