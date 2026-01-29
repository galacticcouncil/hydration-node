#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::Get;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, SimulatorConfig, SimulatorError, SimulatorSet, TradeExecution};
use hydradx_traits::router::{AssetPair, Route, RouteProvider};
use sp_std::marker::PhantomData;

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

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		route: Option<Route<u32>>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let route = route.unwrap_or_else(|| C::RouteProvider::get_route(AssetPair::new(asset_in, asset_out)));

		if route.is_empty() {
			return Err(SimulatorError::Other);
		}

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
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		route: Option<Route<u32>>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let route = route.unwrap_or_else(|| C::RouteProvider::get_route(AssetPair::new(asset_in, asset_out)));

		if route.is_empty() {
			return Err(SimulatorError::Other);
		}

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

	fn get_spot_price(asset_in: u32, asset_out: u32, state: &Self::State) -> Result<Ratio, Self::Error> {
		let route = C::RouteProvider::get_route(AssetPair::new(asset_in, asset_out));

		if route.is_empty() {
			return Err(SimulatorError::AssetNotFound);
		}

		let mut numerator = 1u128;
		let mut denominator = 1u128;

		for trade in route.iter() {
			let hop_price = C::Simulators::get_spot_price(trade.pool, trade.asset_in, trade.asset_out, state)?;

			// Multiply: (n1/d1) * (n2/d2) = (n1*n2)/(d1*d2)
			//TODO: u256?!
			numerator = numerator.checked_mul(hop_price.n).ok_or(SimulatorError::MathError)?;
			denominator = denominator.checked_mul(hop_price.d).ok_or(SimulatorError::MathError)?;
		}

		Ok(Ratio::new(numerator, denominator))
	}

	fn price_denominator() -> u32 {
		C::PriceDenominator::get()
	}
}
