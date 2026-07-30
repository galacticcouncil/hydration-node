//! ICE Solver v4 — global netting.
//!
//! Builds on [`crate::v3`] (price-crossing per-pair clearing) by netting at the
//! *asset* level across the whole batch: chains, cycles of any length, and
//! partial cross-pair coincidences internalize, so only each asset's true net
//! imbalance reaches the AMM. Identical `solve` signature, inputs and `Solution`
//! output as v2/v3, so the runtime switches versions by changing one type path.
//!
//! Currently mirrors v3 verbatim; the global-netting stage is being implemented
//! incrementally and validated against the `ice::netting` regression suite.

mod solver;

pub use solver::Solver;
