//! ICE pass-through builder — the emergency drain mode.
//!
//! No matching and no optimization: every intent is quoted on its own against a
//! running state and admitted when the quote clears the limit the chain will
//! enforce. See [`solver`] for the algorithm and `ICE_EMERGENCY_SOLVER_DESIGN.md`
//! for why this mode exists.

mod solver;

pub use solver::Solver;
