//! Core types for route suggestion.
//!
//! Pool routing types re-exported from `hydradx-traits` and `primitives`.

pub use hydradx_traits::router::{PoolType, Route, Trade, MAX_NUMBER_OF_TRADES};
pub use primitives::AssetId;

/// Concrete `PoolEdge` for this crate's `AssetId`.
pub type PoolEdge = hydradx_traits::router::PoolEdge<AssetId>;

/// Provides the set of all active pools to the route suggester.
///
/// Implement this in the runtime by querying each AMM pallet
/// (Omnipool, XYK, Stableswap, LBP, Aave, HSM).
///
/// The `State` parameter mirrors the `AMMInterface::State` /
/// `SimulatorSet::State` snapshot so that pool discovery can use
/// the same on-chain state as the solver.
pub trait PoolProvider {
    type State: Clone;

    fn get_all_pools(state: &Self::State) -> Vec<PoolEdge>;
}
