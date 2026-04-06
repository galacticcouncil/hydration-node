#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Get;
use frame_support::BoundedVec;
use hydra_dx_math::support::rational::{round_u512_to_rational, Rounding};
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{
	AMMInterface, RouteDiscovery, SimulatorConfig, SimulatorError, SimulatorSet, TradeExecution,
};
use hydradx_traits::router::{AssetPair, PoolEdge, Route, RouteProvider, Trade};
use primitive_types::U512;
use sp_std::marker::PhantomData;
use sp_std::vec;
use sp_std::vec::Vec;

pub mod aave;
pub mod omnipool;
pub mod stableswap;

/// Route discovery using on-chain routes, simulator `can_trade`, and RouteProvider fallback.
///
/// This is the default strategy. It can be replaced in `SimulatorConfig` with a custom
/// implementation (e.g., one that simulates sells across candidate routes).
pub struct OnChainRouteDiscovery<RP, Sims>(PhantomData<(RP, Sims)>);

impl<RP, Sims> RouteDiscovery<Sims::State> for OnChainRouteDiscovery<RP, Sims>
where
	RP: RouteProvider<u32>,
	Sims: SimulatorSet,
{
	fn discover_routes(asset_in: u32, asset_out: u32, state: &Sims::State) -> Result<Vec<Route<u32>>, SimulatorError> {
		let asset_pair = AssetPair::new(asset_in, asset_out);

		// Priority 1: Check for explicitly configured on-chain route
		if let Some(explicit_route) = RP::get_onchain_route(asset_pair) {
			return Ok(vec![explicit_route]);
		}

		// Priority 2: Ask simulators if they can trade this pair directly
		if let Some(pool_type) = Sims::can_trade(asset_in, asset_out, state) {
			return Ok(vec![BoundedVec::truncate_from(vec![Trade {
				pool: pool_type,
				asset_in,
				asset_out,
			}])]);
		}

		// Priority 3: Fall back to the route provider's default
		let route = RP::get_route(asset_pair);
		if route.is_empty() {
			return Err(SimulatorError::AssetNotFound);
		}
		Ok(vec![route])
	}
}

/// The Hydration simulator compositor.
///
/// Implements AMMInterface by composing multiple individual AMM simulators
/// and handling multi-hop routing between them.
pub struct HydrationSimulator<C: SimulatorConfig>(PhantomData<C>);

impl<C: SimulatorConfig> HydrationSimulator<C> {
	/// Get the initial state from all simulators
	pub fn initial_state() -> <C::Simulators as SimulatorSet>::State {
		C::Simulators::initial_state()
	}
}

impl<C: SimulatorConfig> AMMInterface for HydrationSimulator<C> {
	type Error = SimulatorError;
	type State = <C::Simulators as SimulatorSet>::State;

	fn discover_routes(asset_in: u32, asset_out: u32, state: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		C::RouteDiscovery::discover_routes(asset_in, asset_out, state)
	}

	fn sell(
		_asset_in: u32,
		_asset_out: u32,
		amount_in: u128,
		route: Route<u32>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let mut current_state = state.clone();
		let mut current_amount = amount_in;
		let original_amount_in = amount_in;

		for trade in route.iter() {
			let (new_state, result) = C::Simulators::simulate_sell(
				trade.pool,
				trade.asset_in,
				trade.asset_out,
				current_amount,
				0, // No limit check on intermediate hops
				&current_state,
			)?;

			current_state = new_state;
			current_amount = result.amount_out;
		}

		Ok((
			current_state,
			TradeExecution {
				amount_in: original_amount_in,
				amount_out: current_amount,
				route,
			},
		))
	}

	fn buy(
		_asset_in: u32,
		_asset_out: u32,
		amount_out: u128,
		route: Route<u32>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let mut current_required = amount_out;

		let mut current_state = state.clone();
		let mut current_amount = 0u128;

		for trade in route.iter().rev() {
			let (new_state, result) = C::Simulators::simulate_buy(
				trade.pool,
				trade.asset_in,
				trade.asset_out,
				current_required,
				u128::MAX, // No limit on intermediate hops
				&current_state,
			)?;

			current_state = new_state;
			current_amount = result.amount_in;
			current_required = result.amount_in;
		}

		Ok((
			current_state,
			TradeExecution {
				amount_in: current_amount,
				amount_out,
				route,
			},
		))
	}

	fn get_spot_price(
		_asset_in: u32,
		_asset_out: u32,
		route: Route<u32>,
		state: &Self::State,
	) -> Result<Ratio, Self::Error> {
		let mut numerator = U512::from(1u128);
		let mut denominator = U512::from(1u128);

		for chunk in route.chunks(4) {
			let mut chunk_numerator = U512::from(1u128);
			let mut chunk_denominator = U512::from(1u128);

			for trade in chunk.iter() {
				let hop_price = C::Simulators::get_spot_price(trade.pool, trade.asset_in, trade.asset_out, state)?;

				chunk_numerator = chunk_numerator
					.checked_mul(U512::from(hop_price.n))
					.ok_or(SimulatorError::MathError)?;
				chunk_denominator = chunk_denominator
					.checked_mul(U512::from(hop_price.d))
					.ok_or(SimulatorError::MathError)?;
			}

			numerator = numerator
				.checked_mul(chunk_numerator)
				.ok_or(SimulatorError::MathError)?;
			denominator = denominator
				.checked_mul(chunk_denominator)
				.ok_or(SimulatorError::MathError)?;
		}

		let (n, d) = round_u512_to_rational((numerator, denominator), Rounding::Nearest);
		Ok(Ratio::new(n, d))
	}

	fn price_denominator() -> u32 {
		C::PriceDenominator::get()
	}

	fn pool_edges(state: &Self::State) -> Vec<PoolEdge<u32>> {
		C::Simulators::pool_edges(state)
	}
}
