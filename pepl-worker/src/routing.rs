//! Off-chain route search for the collateral → debt sale, so a liquidation does not depend on the
//! router's stored routes. Same machinery as the ICE solver: candidate routes come from the pool
//! graph of a `LiquidityStateApi` snapshot and are priced by simulating the sale against it.

use crate::{InstanceKind, LiquidationDecision, LOG_TARGET};
use amm_simulator::HydrationSimulator;
use codec::Decode;
use hydradx_runtime::{HydrationSimulators, SimulatorPriceDenom, SmartRouteFinder};
use hydradx_traits::amm::{AMMInterface, SimulatorConfig, SimulatorSet};
use hydradx_traits::router::Route;
use pallet_ice_runtime_api::LiquidityStateApi;
use pepl_worker_support::types::{AssetId, Balance};
use sp_api::ApiExt;
use sp_runtime::traits::Block;
use std::time::Instant;

pub type LiquidityState = <HydrationSimulators as SimulatorSet>::State;

/// The runtime's simulators and route discovery, run natively against a decoded snapshot. The
/// simulators' `DataProvider`s are never invoked here, and route pricing never reads an ED.
pub struct PeplSimulatorConfig;
impl SimulatorConfig for PeplSimulatorConfig {
	type Simulators = HydrationSimulators;
	type RouteDiscovery = SmartRouteFinder<HydrationSimulators>;
	type PriceDenominator = SimulatorPriceDenom;
}

type Simulator = HydrationSimulator<PeplSimulatorConfig>;

/// `None` when the runtime predates `LiquidityStateApi` or the snapshot does not decode with this
/// binary's types — callers then fall back to the router's stored routes.
pub fn fetch_liquidity_state<B, Api>(api: &Api, at: B::Hash, log_prefix: &str) -> Option<LiquidityState>
where
	B: Block,
	Api: LiquidityStateApi<B> + ApiExt<B>,
{
	let timer = Instant::now();
	if !api.has_api::<dyn LiquidityStateApi<B>>(at).unwrap_or(false) {
		log::debug!(target: LOG_TARGET, "{log_prefix:?} fetch_liquidity_state(): runtime has no LiquidityStateApi, using stored routes");
		return None;
	}
	let raw = api
		.liquidity_state(at)
		.inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{log_prefix:?} fetch_liquidity_state(): runtime API call failed, using stored routes, err: {e:?}");
		})
		.ok()?;
	let fetched = timer.elapsed();
	let state = decode_liquidity_state(&raw.state, log_prefix)?;
	log::debug!(target: LOG_TARGET, "{log_prefix:?} fetch_liquidity_state(): {} bytes, fetch {:?}, decode {:?}", raw.state.len(), fetched, timer.elapsed().saturating_sub(fetched));
	Some(state)
}

pub fn decode_liquidity_state(raw: &[u8], log_prefix: &str) -> Option<LiquidityState> {
	LiquidityState::decode(&mut &raw[..])
		.inspect_err(|e| {
			// A runtime upgrade changed the snapshot layout ahead of this binary.
			log::error!(target: LOG_TARGET, "{log_prefix:?} decode_liquidity_state(): snapshot does not decode, using stored routes, err: {e:?}");
		})
		.ok()
}

/// Every candidate route `asset_in → asset_out` priced by selling `amount_in`; the highest
/// output wins. `None` when the assets are not connected or no candidate simulates.
pub fn best_sell_route(
	state: &LiquidityState,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
) -> Option<(Route<AssetId>, Balance)> {
	let routes = Simulator::discover_routes(asset_in, asset_out, state).ok()?;
	let mut best: Option<(Route<AssetId>, Balance)> = None;
	for route in routes {
		let Ok((_, exec)) = Simulator::sell(asset_in, asset_out, amount_in, route.clone(), state) else {
			continue;
		};
		// Strict `>` keeps the first maximum; the candidate list is deterministic.
		if best.as_ref().is_none_or(|(_, out)| exec.amount_out > *out) {
			best = Some((route, exec.amount_out));
		}
	}
	best
}

/// Attach the route the pallet will sell the seized collateral on. `None` drops the decision: no
/// route exists, or the best one returns less than `debt_to_cover` and the pallet would revert
/// with `NotProfitable`.
///
/// Without a snapshot the decision keeps the empty route (stored-route lookup). The gigahdx market
/// never sells collateral, so its decisions pass through untouched.
pub fn route_decision(
	kind: InstanceKind,
	liquidity: Option<&LiquidityState>,
	mut decision: LiquidationDecision,
	log_prefix: &str,
) -> Option<LiquidationDecision> {
	let Some(state) = liquidity else {
		return Some(decision);
	};
	if kind == InstanceKind::GigaHdx || decision.collateral_asset == decision.debt_asset {
		return Some(decision);
	}

	let timer = Instant::now();
	let Some((route, amount_out)) = best_sell_route(
		state,
		decision.collateral_asset,
		decision.debt_asset,
		decision.collateral_amount,
	) else {
		log::info!(target: LOG_TARGET, "{log_prefix:?} route_decision(): no route {} -> {} for borrower {:?}, elapsed: {:?}", decision.collateral_asset, decision.debt_asset, decision.user, timer.elapsed());
		return None;
	};
	if amount_out < decision.debt_to_cover {
		log::info!(target: LOG_TARGET, "{log_prefix:?} route_decision(): best route {} -> {} returns {amount_out} < debt_to_cover {} for borrower {:?}", decision.collateral_asset, decision.debt_asset, decision.debt_to_cover, decision.user);
		return None;
	}
	log::debug!(target: LOG_TARGET, "{log_prefix:?} route_decision(): {} -> {} via {} hop(s), out {amount_out} vs debt_to_cover {}, elapsed: {:?}", decision.collateral_asset, decision.debt_asset, route.len(), decision.debt_to_cover, timer.elapsed());
	decision.route = route;
	Some(decision)
}
