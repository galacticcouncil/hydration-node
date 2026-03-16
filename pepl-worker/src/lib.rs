//! PEPL Worker — Protocol Executed Partial Liquidation
//!
//! A standalone liquidation worker that can run either embedded in the Hydration node
//! (production mode) or as a standalone binary against an RPC endpoint (test/debug mode).
//!
//! # Architecture
//!
//! The core worker loop (`worker.rs`) is generic over environment traits:
//! - `BlockSource`: provides new block notifications
//! - `TxSubmitter`: submits liquidation transactions (or reports them in dry-run mode)
//! - `OracleSource`: provides oracle price updates
//! - `DryRunner`: validates transactions before submission
//!
//! # Feature Flags
//!
//! - `node`: enables node integration (real Substrate client, transaction pool)
//! - `standalone`: enables standalone binary mode (RPC-backed, dry-run reporting)

pub mod config;
pub mod oracle;
pub mod traits;
pub mod worker;

#[cfg(test)]
mod tests;

#[cfg(feature = "node")]
pub mod node;

#[cfg(feature = "standalone")]
pub mod standalone;

pub use config::WorkerConfig;
pub use traits::*;
pub use worker::run_worker;
