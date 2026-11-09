//! Hydration chain adapter for cerebro. Decodes `RuntimeEvent`s into core's normalized model and
//! implements the reader traits (`SnapshotProvider`, `PriceSource`, `BalanceReader`) over a live
//! node's JSON-RPC. This crate holds every Hydration/Substrate-specific mechanism; core stays
//! runtime-agnostic.

pub mod accounts;
pub mod balance;
pub mod decode;
pub mod guard;
pub mod permit;
pub mod pricing;
pub mod provider;
pub mod readers;
pub mod snapshot;
pub mod spec;
pub mod state;
pub mod transport;
pub mod xcm;

pub use provider::{parse_h256, RemoteApi};
pub use transport::{Prefetch, Transport};

use cerebro_core::model::{ChainEvent, Phase};
use sp_core::H256;

/// Decode a block's events into core's `(Phase, ChainEvent)` stream, resolving each Aave leg's
/// reserve contract to an `AssetId` with chain access at `at`.
pub fn map_block(api: &RemoteApi, at: H256) -> Result<Vec<(Phase, ChainEvent)>, String> {
	let raw = api.events_raw(at)?;
	let decoded = decode::decode_events(&raw)?;
	let mut out = Vec::with_capacity(decoded.len());
	for (phase, event, raw_debt) in decoded {
		let event = match (event, raw_debt) {
			(ChainEvent::AavePosition(mut leg), Some(raw)) => {
				leg.asset = resolve_reserve(api, raw.reserve, at);
				ChainEvent::AavePosition(leg)
			}
			(ev, _) => ev,
		};
		out.push((phase, event));
	}
	Ok(out)
}

/// Resolve an Aave reserve EVM address to a Hydration `AssetId` (synthetic precompile or real Erc20
/// contract via the `AssetLocations` reverse map).
fn resolve_reserve(api: &RemoteApi, reserve: sp_core::H160, at: H256) -> Option<cerebro_core::AssetId> {
	balance::asset_of_reserve(api, reserve, at)
}
