//! The reader-trait impls that let core read chain state through a live node. `SnapshotProvider`
//! and `BalanceReader` are implemented on `RemoteApi` directly (the chain handle); `PriceSource` is
//! implemented on a pre-fetched `Prices` snapshot (see `pricing`).

use crate::provider::RemoteApi;
use crate::transport::Transport;
use crate::{balance, snapshot};
use cerebro_core::reader::{BalanceReader, OmnipoolAssetSnapshot, SnapshotProvider, StableswapSnapshot, XykSnapshot};
use cerebro_core::{AccountId, AssetId, Balance, BlockHash};

impl SnapshotProvider for RemoteApi {
	fn stableswap(&self, pool: AssetId, at: BlockHash) -> Option<StableswapSnapshot> {
		snapshot::stableswap(self, pool, balance::h256(at))
	}
	fn omnipool_asset(&self, asset: AssetId, at: BlockHash) -> Option<OmnipoolAssetSnapshot> {
		snapshot::omnipool_asset(self, asset, balance::h256(at))
	}
	fn xyk(&self, pair_account: &AccountId, at: BlockHash) -> Option<XykSnapshot> {
		snapshot::xyk(self, pair_account, balance::h256(at))
	}
}

impl BalanceReader for RemoteApi {
	fn own_balance(&self, who: &AccountId, asset: AssetId, at: BlockHash) -> Option<Balance> {
		balance::own_balance(self, who, asset, balance::h256(at))
	}
	// Override the default (parent/at own_balance diff) to route aTokens through ERC20 balanceOf —
	// an aToken reads as absent in orml/native storage, so the default would miss its cost leg.
	fn real_delta(&self, who: &AccountId, asset: AssetId, parent: BlockHash, at: BlockHash) -> Option<i128> {
		balance::real_delta(self, who, asset, balance::h256(parent), balance::h256(at))
	}
}

/// The reader traits over any [`Transport`].
pub struct Chain<'a, T>(pub &'a T);

impl<T: Transport> SnapshotProvider for Chain<'_, T> {
	fn stableswap(&self, pool: AssetId, at: BlockHash) -> Option<StableswapSnapshot> {
		snapshot::stableswap(self.0, pool, balance::h256(at))
	}
	fn omnipool_asset(&self, asset: AssetId, at: BlockHash) -> Option<OmnipoolAssetSnapshot> {
		snapshot::omnipool_asset(self.0, asset, balance::h256(at))
	}
	fn xyk(&self, pair_account: &AccountId, at: BlockHash) -> Option<XykSnapshot> {
		snapshot::xyk(self.0, pair_account, balance::h256(at))
	}
}

impl<T: Transport> BalanceReader for Chain<'_, T> {
	fn own_balance(&self, who: &AccountId, asset: AssetId, at: BlockHash) -> Option<Balance> {
		balance::own_balance(self.0, who, asset, balance::h256(at))
	}
	fn real_delta(&self, who: &AccountId, asset: AssetId, parent: BlockHash, at: BlockHash) -> Option<i128> {
		balance::real_delta(self.0, who, asset, balance::h256(parent), balance::h256(at))
	}
}
