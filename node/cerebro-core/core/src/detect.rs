//! The detection rules, pure over the reduced per-extrinsic flows.
//!
//! Two per-block detectors plus the ledger-feeding valuation. All key on the SIGNER (the fee
//! payer) and exclude protocol accounts. A fair trade nets ~0 by construction at the block's own
//! executed rates, so only value taken *beyond* the executed rate surfaces.

use crate::accounts::ProtocolAccounts;
use crate::model::ExtrinsicFlows;
use crate::prices::ExecPrices;
use crate::value::Usd6;
use crate::{AccountId, AssetId, NATIVE_ASSET};
use std::collections::BTreeMap;

/// A suspicious-profit finding for one extrinsic, attributed to its signer.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
	pub ext_index: u32,
	pub account: AccountId,
	/// The positive per-asset deltas that constitute the unexplained gain.
	pub gains: Vec<(AssetId, i128)>,
	/// Signer's whole-extrinsic net value at executed rates. `None` if unpriced.
	pub net: Option<Usd6>,
	/// True if some non-native leg had no price (net is a lower bound).
	pub partial_pricing: bool,
	pub touched_pool: bool,
	pub has_lp_event: bool,
}

/// One signer's net value in one extrinsic, for the ledger (records every signer, not just findings).
#[derive(Clone, Debug, PartialEq)]
pub struct SignerAction {
	pub account: AccountId,
	pub ext_index: u32,
	pub net: Usd6,
	pub counterparty: Counterparty,
	pub priced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counterparty {
	Lp,
	Pool,
	User,
	/// An XCM transfer — the signer moving its own funds cross-chain (not a pool trade).
	Xcm,
}

impl Counterparty {
	pub fn as_str(self) -> &'static str {
		match self {
			Counterparty::Lp => "lp",
			Counterparty::Pool => "pool",
			Counterparty::User => "user",
			Counterparty::Xcm => "xcm",
		}
	}
}

/// Value the signer's whole-extrinsic net delta at executed rates. Returns `(net, priced)`:
/// an unpriced NATIVE leg is the tx fee (tolerated); any other unpriced leg makes the net a lower
/// bound and sets `priced = false`.
fn signer_net(deltas: &BTreeMap<AssetId, i128>, prices: &ExecPrices) -> (Usd6, bool) {
	let mut net = Usd6::ZERO;
	let mut priced = true;
	for (&asset, &d) in deltas {
		match prices.value(asset, d) {
			Some(v) => net = net.saturating_add(v),
			None if asset != NATIVE_ASSET => priced = false,
			None => {}
		}
	}
	(net, priced)
}

fn counterparty_of(flows: &ExtrinsicFlows) -> Counterparty {
	if flows.is_xcm {
		Counterparty::Xcm
	} else if flows.has_lp_event {
		Counterparty::Lp
	} else if flows.touched_pool {
		Counterparty::Pool
	} else {
		Counterparty::User
	}
}

/// Value each extrinsic's signer net delta at executed rates, tagging counterparty kind. Records
/// EVERY non-protocol signer (not just findings) so the cross-block detectors can accumulate.
pub fn signer_actions(
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	protocol: &ProtocolAccounts,
	prices: &ExecPrices,
) -> Vec<SignerAction> {
	let mut out = Vec::new();
	for (&ext_index, flows) in per_ext {
		let Some(signer) = &flows.signer else {
			continue;
		};
		if protocol.contains(signer) {
			continue;
		}
		let Some(deltas) = flows.deltas.get(signer) else {
			continue;
		};
		let (net, priced) = signer_net(deltas, prices);
		out.push(SignerAction {
			account: *signer,
			ext_index,
			net,
			counterparty: counterparty_of(flows),
			priced,
		});
	}
	out
}

/// Per-block extraction: a signer netting > `min_usd` at executed rates took value *beyond* the
/// executed rate (atomic round-trip / manipulation / real dislocation capture). An unpriceable
/// non-native leg (only the native fee is tolerated) makes the net untrusted → skipped.
pub fn detect_value_extraction(
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	protocol: &ProtocolAccounts,
	prices: &ExecPrices,
	min_usd: Usd6,
) -> Vec<Finding> {
	let mut out = Vec::new();
	for (&ext_index, flows) in per_ext {
		// An XCM transfer moves the signer's own funds cross-chain — never pool extraction.
		if flows.is_xcm {
			continue;
		}
		let Some(signer) = &flows.signer else {
			continue;
		};
		if protocol.contains(signer) {
			continue;
		}
		let Some(deltas) = flows.deltas.get(signer) else {
			continue;
		};
		let (net, priced) = signer_net(deltas, prices);
		if !priced || net <= min_usd {
			continue;
		}
		let gains: Vec<(AssetId, i128)> = deltas.iter().filter(|(_, &d)| d > 0).map(|(&a, &d)| (a, d)).collect();
		out.push(Finding {
			ext_index,
			account: *signer,
			gains,
			net: Some(net),
			partial_pricing: false,
			touched_pool: flows.touched_pool,
			has_lp_event: counterparty_of(flows) == Counterparty::Lp,
		});
	}
	out
}

/// R1 — atomic unexplained extraction (pricing-independent, the lead rule). Flags a signer that
/// ends the extrinsic **net-non-negative in every non-native asset and strictly positive in at
/// least one**, with a pool counterparty and no liquidity event. It received value from the pools
/// and rendered nothing. Native (HDX) is ignored to sidestep the fee leg. A normal swap has a
/// negative leg (it paid), so it never trips R1; only atomic round-trips do.
pub fn detect_signer_gain(per_ext: &BTreeMap<u32, ExtrinsicFlows>, protocol: &ProtocolAccounts) -> Vec<Finding> {
	let mut out = Vec::new();
	for (&ext_index, flows) in per_ext {
		let Some(signer) = &flows.signer else {
			continue;
		};
		if protocol.contains(signer) {
			continue;
		}
		// An XCM transfer moves the signer's own funds cross-chain — never pool extraction.
		if flows.is_xcm || !flows.touched_pool || flows.has_lp_event {
			continue;
		}
		let Some(deltas) = flows.deltas.get(signer) else {
			continue;
		};
		let mut any_gain = false;
		let mut all_non_negative = true;
		for (&asset, &d) in deltas {
			if asset == NATIVE_ASSET {
				continue;
			}
			if d < 0 {
				all_non_negative = false;
				break;
			}
			if d > 0 {
				any_gain = true;
			}
		}
		if all_non_negative && any_gain {
			let gains = deltas
				.iter()
				.filter(|(&a, &d)| a != NATIVE_ASSET && d > 0)
				.map(|(&a, &d)| (a, d))
				.collect();
			out.push(Finding {
				ext_index,
				account: *signer,
				gains,
				net: None,
				partial_pricing: false,
				touched_pool: true,
				has_lp_event: false,
			});
		}
	}
	out
}

/// Observe-mode annotations for a finding — hints, not suppression. `lp`: gain may be returned
/// principal + fees; `user-funded`: no pool counterparty ⇒ likely a payment in, not extraction;
/// `pool-extraction`: the R1 target (value out of pools, nothing rendered).
pub fn annotate(f: &Finding) -> Vec<&'static str> {
	let mut a = Vec::new();
	if f.has_lp_event {
		a.push("lp");
	}
	if !f.touched_pool {
		a.push("user-funded");
	}
	if f.touched_pool && !f.has_lp_event {
		a.push("pool-extraction");
	}
	a
}

/// The default velocity floor: a cumulative alert needs at least this many pool-sourced actions in
/// the window (the "series of actions" target — a single large action is the per-block detector's job).
pub const DEFAULT_MIN_VELOCITY: u32 = 3;

/// Cross-block cumulative rule, pure over the numbers the ledger supplies: fire when the windowed
/// sum of pool-sourced, priced, positive net exceeds `min_cum` AND at least `min_velocity` actions
/// occurred. Fragmented campaigns (fresh signer per round) keep velocity low and evade this by
/// design — the beneficiary-agnostic pool-leak detector is the backstop for those.
pub fn is_cumulative_alert(cum: Usd6, velocity: u32, min_cum: Usd6, min_velocity: u32) -> bool {
	cum > min_cum && velocity >= min_velocity
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::Trade;

	fn flows_with(signer: AccountId, deltas: &[(AssetId, i128)], touched_pool: bool, lp: bool) -> ExtrinsicFlows {
		let mut d = BTreeMap::new();
		d.insert(signer, deltas.iter().copied().collect());
		ExtrinsicFlows {
			deltas: d,
			signer: Some(signer),
			touched_pool,
			has_lp_event: lp,
			..Default::default()
		}
	}

	// An XCM transfer whose signer nets positive on-chain (e.g. an inbound bridge deposit absorbed
	// into a signed ext) must NOT be flagged — it's the user's own cross-chain funds, not extraction.
	#[test]
	fn xcm_ext_should_not_be_flagged_even_when_signer_nets_positive() {
		let mut f = flows_with([1u8; 32], &[(10, 490_000_000)], true, false);
		f.is_xcm = true;
		let per_ext = BTreeMap::from([(3, f)]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		assert!(detect_value_extraction(
			&per_ext,
			&ProtocolAccounts::default(),
			&prices,
			Usd6::from_dollars(50.0)
		)
		.is_empty());
		assert!(detect_signer_gain(&per_ext, &ProtocolAccounts::default()).is_empty());
	}

	#[test]
	fn signer_actions_should_tag_xcm_counterparty_for_xcm_ext() {
		let mut f = flows_with([1u8; 32], &[(9, -96_000_000_000)], true, false);
		f.is_xcm = true;
		let per_ext = BTreeMap::from([(3, f)]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		let acts = signer_actions(&per_ext, &ProtocolAccounts::default(), &prices);
		assert_eq!(acts.len(), 1);
		assert_eq!(acts[0].counterparty, Counterparty::Xcm);
	}

	// USDT=10, DOT=5. A fair swap pays one asset and gains the other → NOT R1.
	#[test]
	fn r1_should_not_fire_when_swap_has_a_negative_leg() {
		let per_ext = BTreeMap::from([(
			2,
			flows_with([1u8; 32], &[(10, -86_000_000), (5, 101_000_000_000)], true, false),
		)]);
		assert!(detect_signer_gain(&per_ext, &ProtocolAccounts::default()).is_empty());
	}

	// Atomic round-trip: net-flat in the spent asset, up in another, pool counterparty → R1 fires.
	#[test]
	fn r1_should_fire_when_net_nonnegative_and_gains_from_pool() {
		let per_ext = BTreeMap::from([(
			2,
			flows_with([1u8; 32], &[(10, 4_327_000_000), (0, -900_000)], true, false),
		)]);
		let f = detect_signer_gain(&per_ext, &ProtocolAccounts::default());
		assert_eq!(f.len(), 1);
		assert_eq!(f[0].gains, vec![(10, 4_327_000_000)]);
	}

	#[test]
	fn r1_should_not_fire_when_liquidity_event_present() {
		let per_ext = BTreeMap::from([(2, flows_with([1u8; 32], &[(10, 5_000_000)], true, true))]);
		assert!(detect_signer_gain(&per_ext, &ProtocolAccounts::default()).is_empty());
	}

	#[test]
	fn r1_should_not_fire_when_no_pool_counterparty() {
		let per_ext = BTreeMap::from([(2, flows_with([1u8; 32], &[(10, 5_000_000)], false, false))]);
		assert!(detect_signer_gain(&per_ext, &ProtocolAccounts::default()).is_empty());
	}

	#[test]
	fn value_extraction_should_skip_when_leg_unpriceable() {
		// DOT (5) has no trade path → unpriced non-native leg → net untrusted → skipped.
		let per_ext = BTreeMap::from([(2, flows_with([1u8; 32], &[(5, 1_000_000_000)], true, false))]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		assert!(detect_value_extraction(
			&per_ext,
			&ProtocolAccounts::default(),
			&prices,
			Usd6::from_dollars(50.0)
		)
		.is_empty());
	}

	#[test]
	fn value_extraction_should_fire_when_signer_nets_above_threshold() {
		// A pure USDT gain (asset 10 anchored at $1) of $4327 with only a native fee leg.
		let per_ext = BTreeMap::from([(
			2,
			flows_with([1u8; 32], &[(10, 4_327_820_297), (0, -900_000)], true, false),
		)]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		let f = detect_value_extraction(
			&per_ext,
			&ProtocolAccounts::default(),
			&prices,
			Usd6::from_dollars(50.0),
		);
		assert_eq!(f.len(), 1);
		assert_eq!(f[0].net, Some(Usd6(4_327_820_297)));
	}

	#[test]
	fn signer_actions_should_record_every_nonprotocol_signer() {
		let per_ext = BTreeMap::from([
			(1, flows_with([1u8; 32], &[(10, -280_000)], true, false)),
			(2, flows_with([2u8; 32], &[(10, 1_000_000)], false, true)),
		]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		let acts = signer_actions(&per_ext, &ProtocolAccounts::default(), &prices);
		assert_eq!(acts.len(), 2);
		assert_eq!(acts[0].counterparty, Counterparty::Pool);
		assert_eq!(acts[1].counterparty, Counterparty::Lp);
	}

	#[test]
	fn annotate_should_tag_pool_extraction_when_pool_touched_without_lp() {
		let f = Finding {
			ext_index: 2,
			account: [1u8; 32],
			gains: vec![],
			net: None,
			partial_pricing: false,
			touched_pool: true,
			has_lp_event: false,
		};
		assert_eq!(annotate(&f), vec!["pool-extraction"]);
	}

	#[test]
	fn annotate_should_tag_lp_and_user_funded_appropriately() {
		let lp = Finding {
			ext_index: 2,
			account: [1u8; 32],
			gains: vec![],
			net: None,
			partial_pricing: false,
			touched_pool: true,
			has_lp_event: true,
		};
		assert_eq!(annotate(&lp), vec!["lp"]);
		let uf = Finding {
			ext_index: 2,
			account: [1u8; 32],
			gains: vec![],
			net: None,
			partial_pricing: false,
			touched_pool: false,
			has_lp_event: false,
		};
		assert_eq!(annotate(&uf), vec!["user-funded"]);
	}

	#[test]
	fn cumulative_alert_should_require_both_threshold_and_velocity() {
		let min = Usd6::from_dollars(200.0);
		assert!(is_cumulative_alert(
			Usd6::from_dollars(250.0),
			3,
			min,
			DEFAULT_MIN_VELOCITY
		));
		// enough value but too few actions (fragmented campaign) → no per-account alert
		assert!(!is_cumulative_alert(
			Usd6::from_dollars(250.0),
			2,
			min,
			DEFAULT_MIN_VELOCITY
		));
		// enough actions but under the value floor
		assert!(!is_cumulative_alert(
			Usd6::from_dollars(150.0),
			5,
			min,
			DEFAULT_MIN_VELOCITY
		));
	}

	#[test]
	fn signer_actions_should_skip_protocol_signers() {
		let mut modl = [0u8; 32];
		modl[..4].copy_from_slice(b"modl");
		let per_ext = BTreeMap::from([(1, flows_with(modl, &[(10, 5_000_000)], true, false))]);
		let prices = ExecPrices::build(&[], &Default::default(), &Default::default());
		let _ = Trade {
			asset_in: 0,
			amount_in: 0,
			asset_out: 0,
			amount_out: 0,
			router: false,
		};
		assert!(signer_actions(&per_ext, &ProtocolAccounts::default(), &prices).is_empty());
	}
}
