//! Crate with utility functions for `build.rs` scripts.

mod git;
mod version;

pub use git::*;
pub use version::*;
