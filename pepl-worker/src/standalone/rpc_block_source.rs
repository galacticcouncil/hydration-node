//! RPC-backed BlockSource for standalone mode.
//!
//! Subscribes to new block headers via `chain_subscribeNewHeads` and parses
//! EVM events (Borrow, LiquidationCall) via `eth_getLogs`.

use crate::traits::*;
use std::sync::mpsc;

/// Receives BlockEvents from an async subscription task via a sync channel.
pub struct RpcBlockSource {
	receiver: mpsc::Receiver<BlockEvent>,
}

impl RpcBlockSource {
	pub fn new(receiver: mpsc::Receiver<BlockEvent>) -> Self {
		Self { receiver }
	}
}

impl BlockSource for RpcBlockSource {
	fn next_block(&mut self) -> Option<BlockEvent> {
		// Blocking wait for the next block event from the async subscription.
		self.receiver.recv().ok()
	}

	fn try_next_block(&mut self) -> Option<BlockEvent> {
		// Non-blocking: return immediately if no block is queued.
		self.receiver.try_recv().ok()
	}
}

/// Header as returned by `chain_subscribeNewHeads`.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubstrateHeader {
	pub number: String,
	#[serde(default)]
	pub parent_hash: String,
}

/// EVM log entry from `eth_getLogs`.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvmLog {
	pub address: String,
	pub topics: Vec<String>,
	#[serde(default)]
	pub data: String,
	#[serde(default)]
	pub block_number: String,
}

/// Configuration for the block subscription event parser.
#[derive(Clone)]
pub struct EventParserConfig {
	/// Address of the Aave Pool contract (emits Borrow events).
	pub borrow_call_address: sp_core::H160,
	/// Borrow event topic hash.
	pub borrow_topic: [u8; 32],
	/// LiquidationCall event topic hash (from the Pool contract).
	pub liquidation_topic: [u8; 32],
	/// CollateralConfigurationChanged event topic hash.
	pub collateral_config_topic: [u8; 32],
	/// Pool configurator address.
	pub pool_configurator_address: sp_core::H160,
}

impl Default for EventParserConfig {
	fn default() -> Self {
		use hex_literal::hex;
		Self {
			borrow_call_address: sp_core::H160(hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38")),
			// Borrow(address reserve, address user, address onBehalfOf, uint256 amount, ...)
			borrow_topic: hex!("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0"),
			// LiquidationCall(address collateralAsset, address debtAsset, address user, ...)
			liquidation_topic: hex!("e413a321e8681d831f4dbccbca790d2952b56f977908e45be37335533e005286"),
			// CollateralConfigurationChanged(address asset, ...)
			collateral_config_topic: hex!("f5e3c2b15c8a16774baa3fab14da8e4c89a5961afb14f57610e8e89444a8c5af"),
			pool_configurator_address: sp_core::H160(hex!("e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4")),
		}
	}
}

/// Parse EVM logs into block event data.
pub fn parse_evm_logs(
	logs: &[EvmLog],
	config: &EventParserConfig,
) -> (Vec<UserAddress>, Vec<UserAddress>, Vec<AssetAddress>) {
	let mut new_borrowers = Vec::new();
	let mut liquidated_users = Vec::new();
	let mut new_assets = Vec::new();

	let borrow_addr = format!("0x{}", hex::encode(config.borrow_call_address.as_bytes()));
	let configurator_addr = format!(
		"0x{}",
		hex::encode(config.pool_configurator_address.as_bytes())
	);
	let borrow_topic = format!("0x{}", hex::encode(config.borrow_topic));
	let liquidation_topic = format!("0x{}", hex::encode(config.liquidation_topic));
	let collateral_topic = format!("0x{}", hex::encode(config.collateral_config_topic));

	for log in logs {
		let addr_lower = log.address.to_lowercase();
		let topic0 = log.topics.first().map(|t| t.to_lowercase());

		if addr_lower == borrow_addr.to_lowercase() {
			if topic0.as_deref() == Some(&borrow_topic) {
				// Borrow event: topic[2] is the onBehalfOf address (the borrower)
				if let Some(topic2) = log.topics.get(2) {
					if let Some(addr) = parse_topic_address(topic2) {
						if !new_borrowers.contains(&addr) {
							new_borrowers.push(addr);
						}
					}
				}
			} else if topic0.as_deref() == Some(&liquidation_topic) {
				// LiquidationCall event: topic[3] is the user being liquidated
				if let Some(topic3) = log.topics.get(3) {
					if let Some(addr) = parse_topic_address(topic3) {
						if !liquidated_users.contains(&addr) {
							liquidated_users.push(addr);
						}
					}
				}
			}
		}

		if addr_lower == configurator_addr.to_lowercase() {
			if topic0.as_deref() == Some(&collateral_topic) {
				// CollateralConfigurationChanged: topic[1] is the asset address
				if let Some(topic1) = log.topics.get(1) {
					if let Some(addr) = parse_topic_address(topic1) {
						if !new_assets.contains(&addr) {
							new_assets.push(addr);
						}
					}
				}
			}
		}
	}

	(new_borrowers, liquidated_users, new_assets)
}

/// Extract an H160 address from a 32-byte hex-encoded topic (last 20 bytes).
fn parse_topic_address(topic: &str) -> Option<sp_core::H160> {
	let bytes = hex::decode(topic.trim_start_matches("0x")).ok()?;
	if bytes.len() < 20 {
		return None;
	}
	// Address is in the last 20 bytes of the 32-byte topic
	let start = bytes.len() - 20;
	Some(sp_core::H160::from_slice(&bytes[start..]))
}
