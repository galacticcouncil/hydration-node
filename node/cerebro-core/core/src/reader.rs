//! The seam between detection and the chain: three small traits, implemented by the
//! chain mapper (RPC today, `AmmSnapshotApi` / node client / recorded fixtures later).
//! Each trait has an independent replacement lifecycle — that is why they are separate.

use crate::value::Usd6;
use crate::{AccountId, AssetId, Balance, BlockHash};

/// One asset's reserve in a stableswap pool, decimals baked in at snapshot build time
/// (mirrors the runtime's `AssetReserve` so the future `AmmSnapshotApi` backend maps 1:1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetReserve {
	pub amount: Balance,
	pub decimals: u8,
}

/// Stableswap pool state sufficient to value the pool at its D-invariant.
/// Amplification arrives already interpolated, pegs already updated — the provider
/// does chain-exact preparation; core only computes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StableswapSnapshot {
	pub assets: Vec<AssetId>,
	pub reserves: Vec<AssetReserve>,
	pub amplification: u128,
	pub pegs: Vec<(Balance, Balance)>,
	pub share_issuance: Balance,
}

/// One omnipool asset's state — each asset↔LRNA leg is a constant-product pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OmnipoolAssetSnapshot {
	pub reserve: Balance,
	pub hub_reserve: Balance,
	pub shares: Balance,
}

/// XYK pair state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XykSnapshot {
	pub assets: (AssetId, AssetId),
	pub reserves: (Balance, Balance),
	pub share_token: AssetId,
	pub share_issuance: Balance,
}

/// How core obtains pool state. v1 = hand-rolled storage reads over RPC;
/// v2 = one `AmmSnapshotApi` state_call per block. Core never knows which answered.
pub trait SnapshotProvider {
	fn stableswap(&self, pool: AssetId, at: BlockHash) -> Option<StableswapSnapshot>;
	fn omnipool_asset(&self, asset: AssetId, at: BlockHash) -> Option<OmnipoolAssetSnapshot>;
	fn xyk(&self, pair_account: &AccountId, at: BlockHash) -> Option<XykSnapshot>;
}

/// Fallback pricing, consulted ONLY where an in-block executed rate is absent
/// (pool-leak USD scaling, flow display). Never used for signer-net valuation:
/// the executed-rate graph always wins. v1 = chainlink/EMA precompile read;
/// v2 = client-side exact-math spot over the AMM snapshot.
pub trait PriceSource {
	/// USD value of `amount` raw units of `asset`.
	fn value_usd(&self, asset: AssetId, amount: i128, at: BlockHash) -> Option<Usd6>;
}

/// Ground-truth account reads for the confirmation fold. The impl resolves the right
/// ledger per asset (orml Tokens / native System.Account / ERC20-aToken balanceOf) —
/// core only sees "the account's OWN balance", which includes RESERVED funds (a DCA's
/// unreserve→trade→re-reserve must not read as a gain on free balance alone).
pub trait BalanceReader {
	fn own_balance(&self, who: &AccountId, asset: AssetId, at: BlockHash) -> Option<Balance>;

	/// Signed balance change parent→at. `None` when either read fails — the caller
	/// must treat an incomplete picture as "cannot confirm", never as zero.
	fn real_delta(&self, who: &AccountId, asset: AssetId, parent: BlockHash, at: BlockHash) -> Option<i128> {
		let before = self.own_balance(who, asset, parent)?;
		let after = self.own_balance(who, asset, at)?;
		Some(after as i128 - before as i128)
	}
}
