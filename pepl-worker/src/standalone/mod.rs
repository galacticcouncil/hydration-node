//! Standalone-mode implementations.
//!
//! These provide trait implementations backed by RPC calls and CLI/file-based
//! oracle injection, enabling the worker to run as a standalone binary
//! for testing, debugging, and scenario simulation.

pub mod oracle_injector;
pub mod report_submitter;
pub mod rpc_block_source;
pub mod rpc_provider;
pub mod types;

pub use oracle_injector::OracleInjector;
pub use report_submitter::ReportSubmitter;
pub use rpc_block_source::RpcBlockSource;
pub use rpc_provider::RpcState;
