//! Route discovery, AMM simulation and existential-deposit memo shared by the
//! solver generations.
//!
//! Route discovery runs once per directed pair (a failure is cached as an empty
//! route set) and existential deposits are memoized because on chain they are a
//! registry read the resolution stages ask for over and over.

use hydradx_traits::amm::AMMInterface;
use hydradx_traits::router::Route;
use ice_support::{AssetId, Balance};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "solver::routes";

pub struct RouteCache<A: AMMInterface> {
	routes: BTreeMap<(AssetId, AssetId), Vec<Route<AssetId>>>,
	existential_deposits: BTreeMap<AssetId, Balance>,
	/// Directed pairs with no route at all — reported in the solve summary.
	unroutable: BTreeSet<(AssetId, AssetId)>,
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> Default for RouteCache<A> {
	fn default() -> Self {
		Self::new()
	}
}

impl<A: AMMInterface> RouteCache<A> {
	pub fn new() -> Self {
		Self {
			routes: BTreeMap::new(),
			existential_deposits: BTreeMap::new(),
			unroutable: BTreeSet::new(),
			_phantom: PhantomData,
		}
	}

	pub fn ed(&mut self, asset: AssetId) -> Balance {
		*self
			.existential_deposits
			.entry(asset)
			.or_insert_with(|| A::existential_deposit(asset))
	}

	/// Cached route set for a directed pair; empty when discovery failed.
	pub fn routes(&mut self, asset_in: AssetId, asset_out: AssetId, state: &A::State) -> &[Route<AssetId>] {
		let key = (asset_in, asset_out);
		if !self.routes.contains_key(&key) {
			let discovered = A::discover_routes(asset_in, asset_out, state).unwrap_or_default();
			if discovered.is_empty() {
				log::debug!(target: LOG_TARGET, "no route for {asset_in} -> {asset_out}");
				self.unroutable.insert(key);
			}
			self.routes.insert(key, discovered);
		}
		self.routes.get(&key).map(|v| v.as_slice()).unwrap_or_default()
	}

	/// Clone of the `i`-th already-discovered route for a pair. Callers that
	/// iterate route sets need an owned `Route` because the AMM interface takes
	/// it by value, but they must not re-run discovery per index.
	pub fn route_at(&self, asset_in: AssetId, asset_out: AssetId, i: usize) -> Option<Route<AssetId>> {
		self.routes.get(&(asset_in, asset_out))?.get(i).cloned()
	}

	/// Best `ExactIn` sell across every discovered route, simulated against
	/// `state`: the winning route, its output, and the state after that trade.
	///
	/// Never memoized — the result is only valid for the state it was simulated
	/// against, and callers thread the state between trades.
	pub fn best_sell(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Route<AssetId>, Balance, A::State)> {
		let mut best: Option<(Route<AssetId>, Balance, A::State)> = None;
		for i in 0..self.routes(asset_in, asset_out, state).len() {
			let Some(route) = self.route_at(asset_in, asset_out, i) else {
				break;
			};
			let Ok((new_state, exec)) = A::sell(asset_in, asset_out, amount_in, route.clone(), state) else {
				continue;
			};
			// `>=` keeps the last maximum on ties; the route list is
			// deterministic, so the choice is stable across collators.
			if best.as_ref().map(|(_, out, _)| exec.amount_out >= *out).unwrap_or(true) {
				best = Some((route, exec.amount_out, new_state));
			}
		}
		best
	}

	pub fn discovered_pairs(&self) -> usize {
		self.routes.len()
	}

	pub fn unroutable_pairs(&self) -> usize {
		self.unroutable.len()
	}
}
