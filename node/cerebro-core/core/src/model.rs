//! Normalized per-block event model — what the chain mapper hands to detection.
//! Populated module-by-module during the Phase-1 port (see docs/port-checklist.md).

use crate::{AccountId, AssetId, Balance, EvmAddress};
use std::collections::{BTreeMap, BTreeSet};

/// Hook-phase (on_initialize/on_finalize) events bucket into pseudo-extrinsics at or above this
/// index, so DCA and other non-ApplyExtrinsic activity is judged like a signed extrinsic.
pub const HOOK_EXT_BASE: u32 = 1_000_000;

pub fn is_hook_ext(ext_index: u32) -> bool {
	ext_index >= HOOK_EXT_BASE
}

/// Per-extrinsic reduced flows — the unit detection judges. One economic actor (the `signer`),
/// its per-asset net delta, and the context needed to explain or fold away benign profit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrinsicFlows {
	/// account -> asset -> signed net delta (inflow positive).
	pub deltas: BTreeMap<AccountId, BTreeMap<AssetId, i128>>,
	/// Fee-payer of this extrinsic — the economic actor whose in-vs-out we judge. Intermediaries
	/// (router, pools, EVM aToken fillers) never pay the fee, so keying on the signer excludes
	/// all plumbing.
	pub signer: Option<AccountId>,
	/// True if any leg touched a known protocol/pool account.
	pub touched_pool: bool,
	/// True if the extrinsic contained an add/remove-liquidity event (profit may be LP fees).
	pub has_lp_event: bool,
	/// Trades executed in this extrinsic (for the price graph and the confirmation sweep set).
	pub trades: Vec<Trade>,
	/// Per-asset conservation residual: Σ deltas over all accounts. ~0 ⇒ pure transfer; non-zero ⇒
	/// net mint/burn (or a decode double-count) — a write-only sanity meter, backstops leg dedup.
	pub residual: BTreeMap<AssetId, i128>,
	/// Account→account hops in EVENT ORDER — reconstructs a trade's path for the flow view.
	pub transfers: Vec<FlowHop>,
	/// Native fee this extrinsic paid.
	pub fee_paid: Balance,
	/// Rewards claimed (staking, omnipool/XYK liquidity mining), per (claimant, currency) — earned
	/// yield, folded out of the net.
	pub rewards: BTreeMap<(AccountId, AssetId), Balance>,
	/// For a DCA pseudo-ext: the net swap. The input is drawn from a reserve funded in a past block,
	/// so the owner's execution-block balance shows only the output — value the DCA by this trade.
	pub dca_trade: Option<Trade>,
	/// Aave position legs (from EVM logs — the debt/aToken side has no substrate event).
	pub debt_legs: Vec<DebtLeg>,
	/// Block-log-worthy actions in this extrinsic, for human labels (not used in detection).
	pub notable: Vec<Notable>,
	/// True if this extrinsic is an XCM transfer (xtokens / pallet_xcm) — the signer is moving its
	/// OWN funds across chains, not trading with a pool. Its net is neither profit nor loss, so it
	/// is never treated as pool extraction; a pool it happens to touch is still caught by the
	/// beneficiary-agnostic pool-leak detector.
	pub is_xcm: bool,
}

/// What one extrinsic changed in storage, decoded from its writes: the authority for balances,
/// issuance and which pools moved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateDelta {
	/// `(who, asset)` → free + reserved before and after.
	pub balances: BTreeMap<(AccountId, AssetId), (Balance, Balance)>,
	/// Total issuance before and after.
	pub issuance: BTreeMap<AssetId, (Balance, Balance)>,
	pub omnipool_assets: BTreeSet<AssetId>,
	pub stable_pools: BTreeSet<AssetId>,
	pub xyk_pools: BTreeSet<AccountId>,
	/// Venues that changed but have no leak model yet, as `Pallet.Item`.
	pub unmodelled: BTreeSet<String>,
	pub evm_slots: BTreeSet<(EvmAddress, [u8; 32])>,
	/// Writes outside the decode table, counted per `Pallet.Item`.
	pub other: BTreeMap<String, u32>,
	/// Writes to a decoded item whose value didn't decode.
	pub undecodable: BTreeSet<String>,
}

impl StateDelta {
	/// Signed balance change per account and asset, zero changes left out.
	pub fn balance_deltas(&self) -> BTreeMap<AccountId, BTreeMap<AssetId, i128>> {
		let mut out: BTreeMap<AccountId, BTreeMap<AssetId, i128>> = BTreeMap::new();
		for (&(who, asset), &(pre, post)) in &self.balances {
			let d = post as i128 - pre as i128;
			if d != 0 {
				out.entry(who).or_default().insert(asset, d);
			}
		}
		out
	}

	/// Assets whose balance changes don't add up to their issuance change: a balance write the
	/// decoder didn't see.
	pub fn unbalanced_assets(&self) -> BTreeSet<AssetId> {
		let mut sum: BTreeMap<AssetId, i128> = BTreeMap::new();
		for (&(_, asset), &(pre, post)) in &self.balances {
			*sum.entry(asset).or_default() += post as i128 - pre as i128;
		}
		for (&asset, &(pre, post)) in &self.issuance {
			*sum.entry(asset).or_default() -= post as i128 - pre as i128;
		}
		sum.into_iter().filter(|(_, d)| *d != 0).map(|(a, _)| a).collect()
	}
}

impl ExtrinsicFlows {
	/// Assets this extrinsic's own trades touched — part of the ground-truth confirmation sweep set.
	pub fn trade_assets(&self) -> BTreeSet<AssetId> {
		self.trades.iter().flat_map(|t| [t.asset_in, t.asset_out]).collect()
	}
}

/// Where in block execution an event occurred. `ApplyExtrinsic(i)` is a signed extrinsic; `Hook`
/// is on_initialize/on_finalize (DCA executions, buybacks, fee-processor distributions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
	ApplyExtrinsic(u32),
	Hook,
}

/// One decoded, chain-agnostic event. The mapper translates runtime events into these in event
/// order; core never sees `RuntimeEvent`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChainEvent {
	/// A balance movement between two accounts (tokens.*, balances.*, currencies.*).
	Transfer(TransferLeg),
	/// An executed trade (router or direct AMM sell/buy).
	Trade(Trade),
	/// An Aave position change parsed from an EVM log (borrow/repay/supply/withdraw).
	AavePosition(DebtLeg),
	/// Extrinsic fee paid — identifies the signer (the economic actor).
	FeePaid {
		who: AccountId,
		amount: Balance,
	},
	/// Rewards claimed — staking or liquidity-mining, in any currency (earned yield — folded out
	/// of profit).
	RewardClaimed {
		who: AccountId,
		asset: AssetId,
		amount: Balance,
	},
	/// A DCA execution segment boundary (schedule id; opens at `Started`, closes on the
	/// first who-bearing lifecycle event, which attributes the segment to the owner).
	DcaStarted {
		schedule_id: u32,
	},
	DcaClosed {
		schedule_id: u32,
		owner: AccountId,
	},
	/// A liquidity add/remove event (gates the R1 rule and labels).
	Liquidity,
	/// A block-log-worthy action, kept for human labels (the monitor renders it to a string).
	Notable(Notable),
	/// An event with no economic signal (ExtrinsicSuccess, timestamp, …). Carries only its phase so
	/// the extrinsic is still counted (`n_ext`) even when it emitted nothing else worth decoding.
	Inert,
}

/// An action recovered from an extrinsic's events, for readable labels. Venue/kind are economic
/// classification (which AMM, add vs remove); the monitor turns these into display strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notable {
	Trade {
		venue: &'static str,
		asset_in: AssetId,
		asset_out: AssetId,
	},
	Liquidity {
		venue: &'static str,
		added: bool,
	},
	Otc {
		kind: &'static str,
	},
	/// A reward claim (staking / LM) — labels the "gain with no cost leg" as earned yield.
	Reward {
		venue: &'static str,
	},
}

/// A balance movement as it comes off an event: either side may be `None` (a mint has no `from`,
/// a burn no `to`). This is the `ChainEvent::Transfer` payload the reducer accounts into deltas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransferLeg {
	pub asset: AssetId,
	pub from: Option<AccountId>,
	pub to: Option<AccountId>,
	pub amount: Balance,
}

/// A resolved account→account hop in event order — the flow-path view (both ends present; a
/// one-sided share mint/burn is resolved against its pool account before landing here).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowHop {
	pub from: AccountId,
	pub to: AccountId,
	pub asset: AssetId,
	pub amount: Balance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trade {
	pub asset_in: AssetId,
	pub amount_in: Balance,
	pub asset_out: AssetId,
	pub amount_out: Balance,
	/// True for a router-level trade (`Router.Executed` = the whole route), false for a single
	/// AMM hop (`Omnipool/Stableswap Sell/BuyExecuted`). A DCA segment values itself by its router
	/// trade, not by the individual hops.
	pub router: bool,
}

/// An Aave position leg. `amount` is signed from the account's wealth perspective:
/// borrow/withdraw increase spendable balance (+), repay/supply decrease it (−); the
/// confirmation fold subtracts the leg so the position change nets out of profit. `asset` is
/// resolved by the mapper (Erc20-type reserves need an `AssetLocations` reverse lookup only the
/// chain side can do); `None` when the reserve couldn't be resolved (that leg is not folded).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DebtLeg {
	pub who: EvmAddress,
	pub asset: Option<AssetId>,
	pub amount: i128,
	/// True for collateral-side ops (supply/withdraw): the counter-leg is a READABLE aToken, so
	/// when the underlying→aToken mapping resolves, the aToken is swept instead of folding this
	/// leg (folding would double-count). False for debt-side ops (borrow/repay), whose debt token
	/// has no readable balance — always folded.
	pub collateral: bool,
}

#[cfg(test)]
mod tests {
	use super::*;

	const A: AccountId = [1; 32];
	const B: AccountId = [2; 32];

	#[test]
	fn unbalanced_assets_should_be_empty_when_transfer_conserves_balance() {
		let delta = StateDelta {
			balances: BTreeMap::from([((A, 5), (100, 60)), ((B, 5), (0, 40))]),
			..Default::default()
		};

		assert_eq!(delta.unbalanced_assets(), BTreeSet::new());
	}

	#[test]
	fn unbalanced_assets_should_be_empty_when_mint_matches_issuance() {
		let delta = StateDelta {
			balances: BTreeMap::from([((A, 5), (0, 40))]),
			issuance: BTreeMap::from([(5, (1_000, 1_040))]),
			..Default::default()
		};

		assert_eq!(delta.unbalanced_assets(), BTreeSet::new());
	}

	#[test]
	fn unbalanced_assets_should_list_asset_when_balance_moves_without_issuance() {
		let delta = StateDelta {
			balances: BTreeMap::from([((A, 5), (0, 40)), ((B, 7), (10, 0)), ((A, 7), (0, 10))]),
			..Default::default()
		};

		assert_eq!(delta.unbalanced_assets(), BTreeSet::from([5]));
	}

	#[test]
	fn balance_deltas_should_skip_rows_when_balance_is_unchanged() {
		let delta = StateDelta {
			balances: BTreeMap::from([((A, 5), (100, 60)), ((B, 5), (7, 7))]),
			..Default::default()
		};

		assert_eq!(
			delta.balance_deltas(),
			BTreeMap::from([(A, BTreeMap::from([(5, -40)]))])
		);
	}
}
