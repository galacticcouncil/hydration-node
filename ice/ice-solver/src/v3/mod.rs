//! ICE Solver v3 — price-crossing batch clearing.
//!
//! Drop-in alternative to [`crate::v2`]: identical `solve` signature, inputs
//! and `Solution` output, so the runtime can switch between solver versions by
//! changing a single type path.

mod solver;

pub use solver::Solver;
