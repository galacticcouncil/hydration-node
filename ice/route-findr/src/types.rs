//! Core types for route suggestion.
//!
//! Pool routing types re-exported from `hydradx-traits` and `primitives`.

pub use hydradx_traits::router::{PoolType, Route, Trade, MAX_NUMBER_OF_TRADES};
pub use primitives::AssetId;

/// Concrete `PoolEdge` for this crate's `AssetId`.
pub type PoolEdge = hydradx_traits::router::PoolEdge<AssetId>;
