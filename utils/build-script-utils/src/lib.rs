//! Crate with utility functions for `build.rs` scripts.

mod version;
mod git;

pub use git::*;
pub use version::*;
