//! ICE Solver v4 — global netting.
//!
//! The production solver. Flows are netted at the *asset* level across the whole
//! batch, so chains, cycles of any length and partial cross-pair coincidences
//! internalize and only each asset's true residual imbalance reaches the AMM.
//! See [`solver`] for the pipeline and `SOLVER.md` for the narrative version.

mod solver;

pub use solver::Solver;
