//! Node-mode mempool monitor: intercepts oracle update transactions from the tx pool.

use crate::traits::*;
use std::sync::mpsc;

/// Receives oracle updates parsed from mempool transactions via a channel.
pub struct NodeMempoolMonitor {
	receiver: mpsc::Receiver<Vec<OracleUpdate>>,
}

impl NodeMempoolMonitor {
	pub fn new(receiver: mpsc::Receiver<Vec<OracleUpdate>>) -> Self {
		Self { receiver }
	}
}

impl OracleSource for NodeMempoolMonitor {
	fn poll_oracle_updates(&mut self) -> Vec<OracleUpdate> {
		match self.receiver.try_recv() {
			Ok(updates) => updates,
			Err(_) => Vec::new(),
		}
	}
}
