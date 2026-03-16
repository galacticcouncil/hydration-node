//! Liquidation worker — thin adapter delegating to `pepl-worker` crate.
//!
//! All PEPL logic lives in the `pepl-worker` crate. This module re-exports
//! the types needed by the node's CLI, service, and RPC layers.

pub use pepl_worker::node::{
	LiquidationTask, LiquidationTaskData, LiquidationWorkerConfig,
	rpc,
};
