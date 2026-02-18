//! AMM Simulation traits for off-chain trade simulation.
//!
//! This module provides traits for simulating AMM trades without modifying chain state.
//! The key abstractions are:
//!
//! - [`SimulatorConfig`] - Configuration bundling simulators and route provider
//! - [`AmmSimulator`] - Individual pool simulator (Omnipool, Stableswap, etc.)
//! - [`SimulatorSet`] - Composite of multiple simulators with automatic dispatch
//! - [`AMMInterface`] - High-level interface for the solver

use crate::router::{PoolType, Route};
use codec::{Decode, Encode};
use frame_support::traits::Get;
use hydra_dx_math::types::Ratio;
use primitives::{AssetId, Balance};
use scale_info::TypeInfo;

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum SimulatorError {
	/// Pool type not supported by this simulator
	NotSupported,
	/// Asset not found in the pool
	AssetNotFound,
	/// Insufficient liquidity for the trade
	InsufficientLiquidity,
	/// Trade amount too small
	TradeTooSmall,
	/// Trade amount too large
	TradeTooLarge,
	/// Limit not met (slippage)
	LimitNotMet,
	/// Math overflow/underflow
	MathError,
	/// Other error
	Other,
}

/// Result of a simulated trade
#[derive(Clone, Debug, Encode, Decode, TypeInfo, PartialEq, Eq)]
pub struct TradeResult {
	pub amount_in: Balance,
	pub amount_out: Balance,
}

impl TradeResult {
	pub fn new(amount_in: Balance, amount_out: Balance) -> Self {
		Self { amount_in, amount_out }
	}
}

/// Extended trade result including the route used
#[derive(Clone, Debug)]
pub struct TradeExecution {
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub route: Route<AssetId>,
}

/// Configuration trait for the simulator compositor.
///
/// Bundles together the simulators and route provider.
/// This is the main configuration type used by the ICE pallet.
///
/// # Example
/// ```ignore
/// pub struct HydrationSimulatorConfig;
///
/// impl SimulatorConfig for HydrationSimulatorConfig {
///     type Simulators = (Omnipool, Stableswap, Aave);
///     type RouteProvider = Router;
///     type PriceDenominator = LRNAAssetId;
/// }
/// ```
pub trait SimulatorConfig {
	/// Tuple of simulators implementing SimulatorSet
	type Simulators: SimulatorSet;
	/// Route provider for finding trade routes
	type RouteProvider: crate::router::RouteProvider<AssetId>;
	/// The reference asset all prices are denominated in (e.g., LRNA)
	type PriceDenominator: Get<AssetId>;
}

/// Individual pool simulator trait.
///
/// Each AMM type (Omnipool, Stableswap, etc.) implements this trait
/// to provide simulation capabilities without modifying chain state.
///
/// The simulator captures a snapshot of the pool state and can simulate
/// trades against that snapshot, returning updated state and trade results.
pub trait AmmSimulator {
	/// Snapshot of the pool state needed for simulation.
	/// Must be Clone for simulation state updates, Encode for offchain worker serialization.
	type Snapshot: Clone + Encode;

	/// Returns the pool type this simulator handles (representative value)
	fn pool_type() -> PoolType<AssetId>;

	/// Check if a given pool type is handled by this simulator.
	/// By default, uses exact equality, but can be overridden for pool types
	/// that have multiple instances (e.g., Stableswap pools with different IDs).
	fn matches_pool_type(pool_type: PoolType<AssetId>) -> bool {
		pool_type == Self::pool_type()
	}

	/// Create a snapshot from current chain state
	fn snapshot() -> Self::Snapshot;

	/// Simulate a sell trade
	fn simulate_sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError>;

	/// Simulate a buy trade
	fn simulate_buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError>;

	/// Get the spot price for a direct pair within this pool.
	/// Returns the price of asset_in in terms of asset_out as a Ratio.
	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		snapshot: &Self::Snapshot,
	) -> Result<Ratio, SimulatorError>;

	/// Check if this simulator can trade the given asset pair directly.
	/// Returns Some(PoolType) if the pair can be traded, None otherwise.
	///
	/// Each AMM knows its own trading capabilities:
	/// - Omnipool: Can trade if both assets are in the omnipool
	/// - Stableswap: Can trade if both assets are in the same pool
	/// - Aave: Can trade if it's a valid aToken/underlying pair
	fn can_trade(_asset_in: AssetId, _asset_out: AssetId, _snapshot: &Self::Snapshot) -> Option<PoolType<AssetId>> {
		// Default implementation: cannot determine trading capability
		None
	}
}

/// A set of simulators that can be dispatched to based on pool type.
///
/// Implemented for individual `AmmSimulator` types (via blanket impl) and
/// tuples of simulators (via macro), allowing composition of multiple
/// simulators with automatic state management.
///
/// When using tuples, the state type is automatically derived as a tuple
/// of individual snapshot types.
///
/// # Example
/// ```ignore
/// // Single simulator - state is OmnipoolSnapshot
/// type Simulators = Omnipool;
///
/// // Multiple simulators - state is (OmnipoolSnapshot, StableswapSnapshot)
/// type Simulators = (Omnipool, Stableswap);
/// ```
pub trait SimulatorSet {
	/// Composite state type - typically a tuple of individual snapshots.
	/// Must be Clone for simulation state updates, Encode for offchain worker serialization.
	type State: Clone + Encode;

	/// Create initial state by calling snapshot() on each simulator
	fn initial_state() -> Self::State;

	/// Simulate a sell trade, dispatching to the appropriate simulator
	fn simulate_sell(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError>;

	/// Simulate a buy trade, dispatching to the appropriate simulator
	fn simulate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError>;

	/// Get spot price, dispatching to the appropriate simulator
	fn get_spot_price(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		state: &Self::State,
	) -> Result<Ratio, SimulatorError>;

	/// Find a simulator that can trade the given asset pair.
	/// Returns Some(PoolType) from the first simulator that can handle it.
	fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>>;
}

/// High-level AMM interface for the solver.
///
/// This is the interface the solver uses - it handles routing
/// and delegates to individual simulators via SimulatorSet.
pub trait AMMInterface {
	type Error;
	type State: Clone;

	fn sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		route: Option<Route<AssetId>>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error>;

	fn buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		route: Option<Route<AssetId>>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error>;

	/// Get spot price for an asset pair (uses routing internally).
	/// Returns the price of asset_in in terms of asset_out.
	fn get_spot_price(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Result<Ratio, Self::Error>;

	/// The reference asset all prices can be denominated in (e.g., LRNA)
	fn price_denominator() -> AssetId;
}

/// Blanket implementation for single simulator.
/// Allows using a single `AmmSimulator` where a `SimulatorSet` is expected.
impl<S: AmmSimulator> SimulatorSet for S {
	type State = S::Snapshot;

	fn initial_state() -> Self::State {
		S::snapshot()
	}

	fn simulate_sell(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError> {
		if !S::matches_pool_type(pool_type) {
			return Err(SimulatorError::NotSupported);
		}
		S::simulate_sell(asset_in, asset_out, amount_in, min_amount_out, state)
	}

	fn simulate_buy(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError> {
		if !S::matches_pool_type(pool_type) {
			return Err(SimulatorError::NotSupported);
		}
		S::simulate_buy(asset_in, asset_out, amount_out, max_amount_in, state)
	}

	fn get_spot_price(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		state: &Self::State,
	) -> Result<Ratio, SimulatorError> {
		if !S::matches_pool_type(pool_type) {
			return Err(SimulatorError::NotSupported);
		}
		S::get_spot_price(asset_in, asset_out, state)
	}

	fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
		S::can_trade(asset_in, asset_out, state)
	}
}

/// Macro to implement SimulatorSet for tuples.
///
/// This generates implementations for tuples of 2 to N simulators,
/// handling the sequential dispatch and positional state updates.
macro_rules! impl_simulator_set_for_tuple {
	// 2-tuple
	(($A:ident, $B:ident), ($a:tt, $b:tt)) => {
		impl<$A, $B> SimulatorSet for ($A, $B)
		where
			$A: SimulatorSet,
			$B: SimulatorSet,
		{
			type State = ($A::State, $B::State);

			fn initial_state() -> Self::State {
				($A::initial_state(), $B::initial_state())
			}

			fn simulate_sell(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				min_amount_out: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_sell(
					pool_type,
					asset_in,
					asset_out,
					amount_in,
					min_amount_out,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok(((new_state, state.$b.clone()), result)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_sell(
							pool_type,
							asset_in,
							asset_out,
							amount_in,
							min_amount_out,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok(((state.$a.clone(), new_state), result)),
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn simulate_buy(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				max_amount_in: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_buy(
					pool_type,
					asset_in,
					asset_out,
					amount_out,
					max_amount_in,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok(((new_state, state.$b.clone()), result)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_buy(
							pool_type,
							asset_in,
							asset_out,
							amount_out,
							max_amount_in,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok(((state.$a.clone(), new_state), result)),
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn get_spot_price(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				state: &Self::State,
			) -> Result<Ratio, SimulatorError> {
				match $A::get_spot_price(pool_type, asset_in, asset_out, &state.$a) {
					Ok(price) => Ok(price),
					Err(SimulatorError::NotSupported) => $B::get_spot_price(pool_type, asset_in, asset_out, &state.$b),
					Err(e) => Err(e),
				}
			}

			fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
				if let Some(pool_type) = $A::can_trade(asset_in, asset_out, &state.$a) {
					return Some(pool_type);
				}
				$B::can_trade(asset_in, asset_out, &state.$b)
			}
		}
	};

	// 3-tuple
	(($A:ident, $B:ident, $C:ident), ($a:tt, $b:tt, $c:tt)) => {
		impl<$A, $B, $C> SimulatorSet for ($A, $B, $C)
		where
			$A: SimulatorSet,
			$B: SimulatorSet,
			$C: SimulatorSet,
		{
			type State = ($A::State, $B::State, $C::State);

			fn initial_state() -> Self::State {
				($A::initial_state(), $B::initial_state(), $C::initial_state())
			}

			fn simulate_sell(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				min_amount_out: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_sell(
					pool_type,
					asset_in,
					asset_out,
					amount_in,
					min_amount_out,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok(((new_state, state.$b.clone(), state.$c.clone()), result)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_sell(
							pool_type,
							asset_in,
							asset_out,
							amount_in,
							min_amount_out,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok(((state.$a.clone(), new_state, state.$c.clone()), result)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_sell(
									pool_type,
									asset_in,
									asset_out,
									amount_in,
									min_amount_out,
									&state.$c,
								) {
									Ok((new_state, result)) => {
										Ok(((state.$a.clone(), state.$b.clone(), new_state), result))
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn simulate_buy(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				max_amount_in: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_buy(
					pool_type,
					asset_in,
					asset_out,
					amount_out,
					max_amount_in,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok(((new_state, state.$b.clone(), state.$c.clone()), result)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_buy(
							pool_type,
							asset_in,
							asset_out,
							amount_out,
							max_amount_in,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok(((state.$a.clone(), new_state, state.$c.clone()), result)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_buy(
									pool_type,
									asset_in,
									asset_out,
									amount_out,
									max_amount_in,
									&state.$c,
								) {
									Ok((new_state, result)) => {
										Ok(((state.$a.clone(), state.$b.clone(), new_state), result))
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn get_spot_price(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				state: &Self::State,
			) -> Result<Ratio, SimulatorError> {
				match $A::get_spot_price(pool_type, asset_in, asset_out, &state.$a) {
					Ok(price) => Ok(price),
					Err(SimulatorError::NotSupported) => {
						match $B::get_spot_price(pool_type, asset_in, asset_out, &state.$b) {
							Ok(price) => Ok(price),
							Err(SimulatorError::NotSupported) => {
								$C::get_spot_price(pool_type, asset_in, asset_out, &state.$c)
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
				if let Some(pool_type) = $A::can_trade(asset_in, asset_out, &state.$a) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $B::can_trade(asset_in, asset_out, &state.$b) {
					return Some(pool_type);
				}
				$C::can_trade(asset_in, asset_out, &state.$c)
			}
		}
	};

	// 4-tuple
	(($A:ident, $B:ident, $C:ident, $D:ident), ($a:tt, $b:tt, $c:tt, $d:tt)) => {
		impl<$A, $B, $C, $D> SimulatorSet for ($A, $B, $C, $D)
		where
			$A: SimulatorSet,
			$B: SimulatorSet,
			$C: SimulatorSet,
			$D: SimulatorSet,
		{
			type State = ($A::State, $B::State, $C::State, $D::State);

			fn initial_state() -> Self::State {
				(
					$A::initial_state(),
					$B::initial_state(),
					$C::initial_state(),
					$D::initial_state(),
				)
			}

			fn simulate_sell(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				min_amount_out: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_sell(
					pool_type,
					asset_in,
					asset_out,
					amount_in,
					min_amount_out,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(new_state, state.$b.clone(), state.$c.clone(), state.$d.clone()),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_sell(
							pool_type,
							asset_in,
							asset_out,
							amount_in,
							min_amount_out,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(state.$a.clone(), new_state, state.$c.clone(), state.$d.clone()),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_sell(
									pool_type,
									asset_in,
									asset_out,
									amount_in,
									min_amount_out,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(state.$a.clone(), state.$b.clone(), new_state, state.$d.clone()),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_sell(
											pool_type,
											asset_in,
											asset_out,
											amount_in,
											min_amount_out,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(state.$a.clone(), state.$b.clone(), state.$c.clone(), new_state),
												result,
											)),
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn simulate_buy(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				max_amount_in: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_buy(
					pool_type,
					asset_in,
					asset_out,
					amount_out,
					max_amount_in,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(new_state, state.$b.clone(), state.$c.clone(), state.$d.clone()),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_buy(
							pool_type,
							asset_in,
							asset_out,
							amount_out,
							max_amount_in,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(state.$a.clone(), new_state, state.$c.clone(), state.$d.clone()),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_buy(
									pool_type,
									asset_in,
									asset_out,
									amount_out,
									max_amount_in,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(state.$a.clone(), state.$b.clone(), new_state, state.$d.clone()),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_buy(
											pool_type,
											asset_in,
											asset_out,
											amount_out,
											max_amount_in,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(state.$a.clone(), state.$b.clone(), state.$c.clone(), new_state),
												result,
											)),
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn get_spot_price(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				state: &Self::State,
			) -> Result<Ratio, SimulatorError> {
				match $A::get_spot_price(pool_type, asset_in, asset_out, &state.$a) {
					Ok(price) => Ok(price),
					Err(SimulatorError::NotSupported) => {
						match $B::get_spot_price(pool_type, asset_in, asset_out, &state.$b) {
							Ok(price) => Ok(price),
							Err(SimulatorError::NotSupported) => {
								match $C::get_spot_price(pool_type, asset_in, asset_out, &state.$c) {
									Ok(price) => Ok(price),
									Err(SimulatorError::NotSupported) => {
										$D::get_spot_price(pool_type, asset_in, asset_out, &state.$d)
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
				if let Some(pool_type) = $A::can_trade(asset_in, asset_out, &state.$a) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $B::can_trade(asset_in, asset_out, &state.$b) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $C::can_trade(asset_in, asset_out, &state.$c) {
					return Some(pool_type);
				}
				$D::can_trade(asset_in, asset_out, &state.$d)
			}
		}
	};

	// 5-tuple
	(($A:ident, $B:ident, $C:ident, $D:ident, $E:ident), ($a:tt, $b:tt, $c:tt, $d:tt, $e:tt)) => {
		impl<$A, $B, $C, $D, $E> SimulatorSet for ($A, $B, $C, $D, $E)
		where
			$A: SimulatorSet,
			$B: SimulatorSet,
			$C: SimulatorSet,
			$D: SimulatorSet,
			$E: SimulatorSet,
		{
			type State = ($A::State, $B::State, $C::State, $D::State, $E::State);

			fn initial_state() -> Self::State {
				(
					$A::initial_state(),
					$B::initial_state(),
					$C::initial_state(),
					$D::initial_state(),
					$E::initial_state(),
				)
			}

			fn simulate_sell(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				min_amount_out: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_sell(
					pool_type,
					asset_in,
					asset_out,
					amount_in,
					min_amount_out,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(
							new_state,
							state.$b.clone(),
							state.$c.clone(),
							state.$d.clone(),
							state.$e.clone(),
						),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_sell(
							pool_type,
							asset_in,
							asset_out,
							amount_in,
							min_amount_out,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(
									state.$a.clone(),
									new_state,
									state.$c.clone(),
									state.$d.clone(),
									state.$e.clone(),
								),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_sell(
									pool_type,
									asset_in,
									asset_out,
									amount_in,
									min_amount_out,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(
											state.$a.clone(),
											state.$b.clone(),
											new_state,
											state.$d.clone(),
											state.$e.clone(),
										),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_sell(
											pool_type,
											asset_in,
											asset_out,
											amount_in,
											min_amount_out,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(
													state.$a.clone(),
													state.$b.clone(),
													state.$c.clone(),
													new_state,
													state.$e.clone(),
												),
												result,
											)),
											Err(SimulatorError::NotSupported) => {
												match $E::simulate_sell(
													pool_type,
													asset_in,
													asset_out,
													amount_in,
													min_amount_out,
													&state.$e,
												) {
													Ok((new_state, result)) => Ok((
														(
															state.$a.clone(),
															state.$b.clone(),
															state.$c.clone(),
															state.$d.clone(),
															new_state,
														),
														result,
													)),
													Err(e) => Err(e),
												}
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn simulate_buy(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				max_amount_in: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_buy(
					pool_type,
					asset_in,
					asset_out,
					amount_out,
					max_amount_in,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(
							new_state,
							state.$b.clone(),
							state.$c.clone(),
							state.$d.clone(),
							state.$e.clone(),
						),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_buy(
							pool_type,
							asset_in,
							asset_out,
							amount_out,
							max_amount_in,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(
									state.$a.clone(),
									new_state,
									state.$c.clone(),
									state.$d.clone(),
									state.$e.clone(),
								),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_buy(
									pool_type,
									asset_in,
									asset_out,
									amount_out,
									max_amount_in,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(
											state.$a.clone(),
											state.$b.clone(),
											new_state,
											state.$d.clone(),
											state.$e.clone(),
										),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_buy(
											pool_type,
											asset_in,
											asset_out,
											amount_out,
											max_amount_in,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(
													state.$a.clone(),
													state.$b.clone(),
													state.$c.clone(),
													new_state,
													state.$e.clone(),
												),
												result,
											)),
											Err(SimulatorError::NotSupported) => {
												match $E::simulate_buy(
													pool_type,
													asset_in,
													asset_out,
													amount_out,
													max_amount_in,
													&state.$e,
												) {
													Ok((new_state, result)) => Ok((
														(
															state.$a.clone(),
															state.$b.clone(),
															state.$c.clone(),
															state.$d.clone(),
															new_state,
														),
														result,
													)),
													Err(e) => Err(e),
												}
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn get_spot_price(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				state: &Self::State,
			) -> Result<Ratio, SimulatorError> {
				match $A::get_spot_price(pool_type, asset_in, asset_out, &state.$a) {
					Ok(price) => Ok(price),
					Err(SimulatorError::NotSupported) => {
						match $B::get_spot_price(pool_type, asset_in, asset_out, &state.$b) {
							Ok(price) => Ok(price),
							Err(SimulatorError::NotSupported) => {
								match $C::get_spot_price(pool_type, asset_in, asset_out, &state.$c) {
									Ok(price) => Ok(price),
									Err(SimulatorError::NotSupported) => {
										match $D::get_spot_price(pool_type, asset_in, asset_out, &state.$d) {
											Ok(price) => Ok(price),
											Err(SimulatorError::NotSupported) => {
												$E::get_spot_price(pool_type, asset_in, asset_out, &state.$e)
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
				if let Some(pool_type) = $A::can_trade(asset_in, asset_out, &state.$a) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $B::can_trade(asset_in, asset_out, &state.$b) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $C::can_trade(asset_in, asset_out, &state.$c) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $D::can_trade(asset_in, asset_out, &state.$d) {
					return Some(pool_type);
				}
				$E::can_trade(asset_in, asset_out, &state.$e)
			}
		}
	};

	// 6-tuple
	(($A:ident, $B:ident, $C:ident, $D:ident, $E:ident, $F:ident), ($a:tt, $b:tt, $c:tt, $d:tt, $e:tt, $f:tt)) => {
		impl<$A, $B, $C, $D, $E, $F> SimulatorSet for ($A, $B, $C, $D, $E, $F)
		where
			$A: SimulatorSet,
			$B: SimulatorSet,
			$C: SimulatorSet,
			$D: SimulatorSet,
			$E: SimulatorSet,
			$F: SimulatorSet,
		{
			type State = ($A::State, $B::State, $C::State, $D::State, $E::State, $F::State);

			fn initial_state() -> Self::State {
				(
					$A::initial_state(),
					$B::initial_state(),
					$C::initial_state(),
					$D::initial_state(),
					$E::initial_state(),
					$F::initial_state(),
				)
			}

			fn simulate_sell(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				min_amount_out: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_sell(
					pool_type,
					asset_in,
					asset_out,
					amount_in,
					min_amount_out,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(
							new_state,
							state.$b.clone(),
							state.$c.clone(),
							state.$d.clone(),
							state.$e.clone(),
							state.$f.clone(),
						),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_sell(
							pool_type,
							asset_in,
							asset_out,
							amount_in,
							min_amount_out,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(
									state.$a.clone(),
									new_state,
									state.$c.clone(),
									state.$d.clone(),
									state.$e.clone(),
									state.$f.clone(),
								),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_sell(
									pool_type,
									asset_in,
									asset_out,
									amount_in,
									min_amount_out,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(
											state.$a.clone(),
											state.$b.clone(),
											new_state,
											state.$d.clone(),
											state.$e.clone(),
											state.$f.clone(),
										),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_sell(
											pool_type,
											asset_in,
											asset_out,
											amount_in,
											min_amount_out,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(
													state.$a.clone(),
													state.$b.clone(),
													state.$c.clone(),
													new_state,
													state.$e.clone(),
													state.$f.clone(),
												),
												result,
											)),
											Err(SimulatorError::NotSupported) => {
												match $E::simulate_sell(
													pool_type,
													asset_in,
													asset_out,
													amount_in,
													min_amount_out,
													&state.$e,
												) {
													Ok((new_state, result)) => Ok((
														(
															state.$a.clone(),
															state.$b.clone(),
															state.$c.clone(),
															state.$d.clone(),
															new_state,
															state.$f.clone(),
														),
														result,
													)),
													Err(SimulatorError::NotSupported) => {
														match $F::simulate_sell(
															pool_type,
															asset_in,
															asset_out,
															amount_in,
															min_amount_out,
															&state.$f,
														) {
															Ok((new_state, result)) => Ok((
																(
																	state.$a.clone(),
																	state.$b.clone(),
																	state.$c.clone(),
																	state.$d.clone(),
																	state.$e.clone(),
																	new_state,
																),
																result,
															)),
															Err(e) => Err(e),
														}
													}
													Err(e) => Err(e),
												}
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn simulate_buy(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				max_amount_in: Balance,
				state: &Self::State,
			) -> Result<(Self::State, TradeResult), SimulatorError> {
				match $A::simulate_buy(
					pool_type,
					asset_in,
					asset_out,
					amount_out,
					max_amount_in,
					&state.$a,
				) {
					Ok((new_state, result)) => Ok((
						(
							new_state,
							state.$b.clone(),
							state.$c.clone(),
							state.$d.clone(),
							state.$e.clone(),
							state.$f.clone(),
						),
						result,
					)),
					Err(SimulatorError::NotSupported) => {
						match $B::simulate_buy(
							pool_type,
							asset_in,
							asset_out,
							amount_out,
							max_amount_in,
							&state.$b,
						) {
							Ok((new_state, result)) => Ok((
								(
									state.$a.clone(),
									new_state,
									state.$c.clone(),
									state.$d.clone(),
									state.$e.clone(),
									state.$f.clone(),
								),
								result,
							)),
							Err(SimulatorError::NotSupported) => {
								match $C::simulate_buy(
									pool_type,
									asset_in,
									asset_out,
									amount_out,
									max_amount_in,
									&state.$c,
								) {
									Ok((new_state, result)) => Ok((
										(
											state.$a.clone(),
											state.$b.clone(),
											new_state,
											state.$d.clone(),
											state.$e.clone(),
											state.$f.clone(),
										),
										result,
									)),
									Err(SimulatorError::NotSupported) => {
										match $D::simulate_buy(
											pool_type,
											asset_in,
											asset_out,
											amount_out,
											max_amount_in,
											&state.$d,
										) {
											Ok((new_state, result)) => Ok((
												(
													state.$a.clone(),
													state.$b.clone(),
													state.$c.clone(),
													new_state,
													state.$e.clone(),
													state.$f.clone(),
												),
												result,
											)),
											Err(SimulatorError::NotSupported) => {
												match $E::simulate_buy(
													pool_type,
													asset_in,
													asset_out,
													amount_out,
													max_amount_in,
													&state.$e,
												) {
													Ok((new_state, result)) => Ok((
														(
															state.$a.clone(),
															state.$b.clone(),
															state.$c.clone(),
															state.$d.clone(),
															new_state,
															state.$f.clone(),
														),
														result,
													)),
													Err(SimulatorError::NotSupported) => {
														match $F::simulate_buy(
															pool_type,
															asset_in,
															asset_out,
															amount_out,
															max_amount_in,
															&state.$f,
														) {
															Ok((new_state, result)) => Ok((
																(
																	state.$a.clone(),
																	state.$b.clone(),
																	state.$c.clone(),
																	state.$d.clone(),
																	state.$e.clone(),
																	new_state,
																),
																result,
															)),
															Err(e) => Err(e),
														}
													}
													Err(e) => Err(e),
												}
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn get_spot_price(
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				state: &Self::State,
			) -> Result<Ratio, SimulatorError> {
				match $A::get_spot_price(pool_type, asset_in, asset_out, &state.$a) {
					Ok(price) => Ok(price),
					Err(SimulatorError::NotSupported) => {
						match $B::get_spot_price(pool_type, asset_in, asset_out, &state.$b) {
							Ok(price) => Ok(price),
							Err(SimulatorError::NotSupported) => {
								match $C::get_spot_price(pool_type, asset_in, asset_out, &state.$c) {
									Ok(price) => Ok(price),
									Err(SimulatorError::NotSupported) => {
										match $D::get_spot_price(pool_type, asset_in, asset_out, &state.$d) {
											Ok(price) => Ok(price),
											Err(SimulatorError::NotSupported) => {
												match $E::get_spot_price(pool_type, asset_in, asset_out, &state.$e) {
													Ok(price) => Ok(price),
													Err(SimulatorError::NotSupported) => {
														$F::get_spot_price(pool_type, asset_in, asset_out, &state.$f)
													}
													Err(e) => Err(e),
												}
											}
											Err(e) => Err(e),
										}
									}
									Err(e) => Err(e),
								}
							}
							Err(e) => Err(e),
						}
					}
					Err(e) => Err(e),
				}
			}

			fn can_trade(asset_in: AssetId, asset_out: AssetId, state: &Self::State) -> Option<PoolType<AssetId>> {
				if let Some(pool_type) = $A::can_trade(asset_in, asset_out, &state.$a) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $B::can_trade(asset_in, asset_out, &state.$b) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $C::can_trade(asset_in, asset_out, &state.$c) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $D::can_trade(asset_in, asset_out, &state.$d) {
					return Some(pool_type);
				}
				if let Some(pool_type) = $E::can_trade(asset_in, asset_out, &state.$e) {
					return Some(pool_type);
				}
				$F::can_trade(asset_in, asset_out, &state.$f)
			}
		}
	};
}

// Generate implementations for tuples of 2 to 6 elements
impl_simulator_set_for_tuple!((A, B), (0, 1));
impl_simulator_set_for_tuple!((A, B, C), (0, 1, 2));
impl_simulator_set_for_tuple!((A, B, C, D), (0, 1, 2, 3));
impl_simulator_set_for_tuple!((A, B, C, D, E), (0, 1, 2, 3, 4));
impl_simulator_set_for_tuple!((A, B, C, D, E, F), (0, 1, 2, 3, 4, 5));
