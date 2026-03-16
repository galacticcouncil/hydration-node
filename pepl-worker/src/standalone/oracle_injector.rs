//! Oracle injection from CLI arguments or JSON files for scenario testing.

use crate::traits::*;
use sp_core::H160;
use std::collections::VecDeque;

/// Oracle update scenario loaded from JSON file or CLI.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct OracleScenario {
	pub block: String, // "latest" or block number
	pub oracle_updates: Vec<OracleScenarioEntry>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct OracleScenarioEntry {
	pub pair: String,       // e.g., "DOT/USD"
	pub price: f64,         // e.g., 3.50
	pub asset_address: Option<String>, // hex address of the asset in the money market
}

/// Injects oracle updates from a queue. Used for CLI-driven scenario testing.
pub struct OracleInjector {
	queue: VecDeque<Vec<OracleUpdate>>,
}

impl OracleInjector {
	pub fn new() -> Self {
		Self {
			queue: VecDeque::new(),
		}
	}

	/// Queue a batch of oracle updates to be delivered on the next poll.
	pub fn inject(&mut self, updates: Vec<OracleUpdate>) {
		self.queue.push_back(updates);
	}

	/// Load oracle updates from a scenario file.
	pub fn load_scenario(&mut self, scenario: &OracleScenario) {
		let updates: Vec<OracleUpdate> = scenario
			.oracle_updates
			.iter()
			.filter_map(|entry| {
				let addr_hex = entry.asset_address.as_ref()?;
				let addr_bytes = hex::decode(addr_hex.trim_start_matches("0x")).ok()?;
				if addr_bytes.len() != 20 {
					return None;
				}
				let asset_address = H160::from_slice(&addr_bytes);

				// Convert f64 price to U256 with 8 decimal precision (DIA oracle format).
				let price_u128 = (entry.price * 1e8) as u128;

				Some(OracleUpdate {
					asset_address,
					price: Some(ethabi::ethereum_types::U256::from(price_u128)),
				})
			})
			.collect();

		if !updates.is_empty() {
			self.queue.push_back(updates);
		}
	}
}

impl OracleSource for OracleInjector {
	fn poll_oracle_updates(&mut self) -> Vec<OracleUpdate> {
		self.queue.pop_front().unwrap_or_default()
	}
}

/// No-op oracle source for when no oracle injection is needed.
pub struct NoOpOracleSource;

impl OracleSource for NoOpOracleSource {
	fn poll_oracle_updates(&mut self) -> Vec<OracleUpdate> {
		Vec::new()
	}
}
