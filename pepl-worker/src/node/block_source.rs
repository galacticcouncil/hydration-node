//! Node-mode BlockSource: wraps `client.import_notification_stream()` and event parsing.

use crate::traits::*;
use std::sync::mpsc;

/// Receives block events from the node's event stream via a channel.
/// The actual stream processing (import_notification_stream + event filtering) happens
/// in the async task that feeds this channel.
pub struct NodeBlockSource {
	receiver: mpsc::Receiver<BlockEvent>,
}

impl NodeBlockSource {
	pub fn new(receiver: mpsc::Receiver<BlockEvent>) -> Self {
		Self { receiver }
	}
}

impl BlockSource for NodeBlockSource {
	fn next_block(&mut self) -> Option<BlockEvent> {
		// Blocking wait for the next block event from the async subscription.
		self.receiver.recv().ok()
	}

	fn try_next_block(&mut self) -> Option<BlockEvent> {
		// Non-blocking: return immediately if no block is queued.
		self.receiver.try_recv().ok()
	}
}
