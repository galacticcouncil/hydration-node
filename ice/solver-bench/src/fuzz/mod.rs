//! Standalone fuzzing / property-soak harness for the ICE solver and on-chain
//! solution submission, run against the same `mainnet_apr` snapshot the
//! integration tests use. Two tiers:
//!
//! - **Tier 1 (solver):** generate scenarios, solve in-memory against the live
//!   simulator state, assert solution invariants. Fast — thousands/sec.
//! - **Tier 2 (submit):** submit intents as extrinsics, run the solver, submit
//!   the solution to the pallet, assert it executes and balances move as
//!   claimed. Authoritative (the chain re-checks conservation + score).
//!
//! The oracle is invariant-based (an optimizer has no cheap ground truth):
//! every solution must respect each user's limit, conserve value per asset,
//! stay within bounds, and be deterministic. See [`oracle`].

pub mod diff;
pub mod gen;
pub mod harness;
pub mod oracle;
pub mod rng;

use amm_simulator::HydrationSimulator;
use primitives::{AssetId, Balance};

/// AMM interface whose `State` is the combined simulator state the solver runs
/// against — identical to the pallet's production wiring.
pub type Amm = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
pub type SolverV3 = ice_solver::v3::Solver<Amm>;
pub type SolverV4 = ice_solver::v4::Solver<Amm>;

pub use crate::CombinedSimulatorState as State;

/// Which solver(s) a run exercises.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SolverSel {
	V3,
	V4,
	/// Run both and compare (v4 ≥ v3 on score, ≤ on trades — soft, reported).
	Diff,
}

/// A tradeable asset on the snapshot plus a sane native-unit amount range for
/// generation. Restricted to all-Omnipool assets (reliable routing); USDT is
/// excluded from the core set because it routes through stableswap.
#[derive(Clone, Copy)]
pub struct AssetSpec {
	pub id: AssetId,
	pub decimals: u32,
	pub min_amount: Balance,
	pub max_amount: Balance,
}

/// Core universe: HDX(0,12), DOT(5,10), BNC(14,12), WETH(20,18), ETH(34,18).
pub const ASSETS: &[AssetSpec] = &[
	AssetSpec {
		id: 0,
		decimals: 12,
		min_amount: 1_000_000_000_000,
		max_amount: 10_000_000_000_000_000,
	},
	AssetSpec {
		id: 5,
		decimals: 10,
		min_amount: 10_000_000_000,
		max_amount: 100_000_000_000_000,
	},
	AssetSpec {
		id: 14,
		decimals: 12,
		min_amount: 1_000_000_000_000,
		max_amount: 10_000_000_000_000_000,
	},
	AssetSpec {
		id: 20,
		decimals: 18,
		min_amount: 1_000_000_000_000_000,
		max_amount: 10_000_000_000_000_000_000,
	},
	AssetSpec {
		id: 34,
		decimals: 18,
		min_amount: 1_000_000_000_000_000,
		max_amount: 10_000_000_000_000_000_000,
	},
];

pub fn asset_spec(id: AssetId) -> Option<&'static AssetSpec> {
	ASSETS.iter().find(|a| a.id == id)
}
