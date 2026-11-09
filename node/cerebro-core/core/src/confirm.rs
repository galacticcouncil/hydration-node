//! Ground-truth confirmation: before trusting a finding, re-value the signer's COMPLETE net from
//! actual balance changes (state), not events. An event is the runtime's claim; the balance change
//! is what moved — their divergence is the exact signal (a pool sends without taking = free money).
//!
//! This is the false-positive killer. It reads balances (via [`BalanceReader`]) over a sweep set
//! wide enough to catch EVM-invisible cost legs, then folds out explained gains/costs that have no
//! readable balance counter-leg (Aave positions, staking/LM reward claims) or a reserve-distorted
//! one (DCA).
//! Returns `None` on ANY failed read — an incomplete picture may never suppress a finding.

use crate::accounts::{evm_address, ProtocolAccounts};
use crate::model::ExtrinsicFlows;
use crate::prices::ExecPrices;
use crate::reader::{BalanceReader, SnapshotProvider};
use crate::value::Usd6;
use crate::{AccountId, AssetId, BlockHash};
use std::collections::{BTreeMap, BTreeSet};

/// Actions with |event net| below this ($1) skip ground-truth confirmation (nothing material to check).
pub const CONFIRM_FLOOR_USDT: i128 = 1_000_000;

/// The signer's complete real net for one extrinsic, in USD. `None` = cannot fully account (a
/// failed read or missing parent) → the caller keeps the finding (never suppress on incomplete data).
///
/// `atoken_of` maps an underlying asset to its aToken's asset id: a supply/withdraw leg whose
/// aToken resolves is SWEPT (the aToken is a readable ERC20) instead of folded — folding a swept
/// counter-leg double-counts, which turned supplies into phantom profit.
#[allow(clippy::too_many_arguments)]
pub fn signer_real_net<B: BalanceReader, S: SnapshotProvider>(
	signer: &AccountId,
	ext_index: u32,
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	block_trade_assets: &BTreeSet<AssetId>,
	protocol: &ProtocolAccounts,
	atoken_of: &BTreeMap<AssetId, AssetId>,
	parent: BlockHash,
	at: BlockHash,
	exec: &ExecPrices,
	balances: &B,
	snapshots: &S,
) -> Option<Usd6> {
	let flows = per_ext.get(&ext_index);

	// A DCA execution's input is drawn from a reserve funded in a PAST block, so the owner's
	// execution-block balance shows only the output — a phantom gain. Value it by the executed
	// trade (out − in): fair by construction, an off-market DCA nets positive.
	if let Some(t) = flows.and_then(|f| f.dca_trade.as_ref()) {
		let out = exec.value(t.asset_out, t.amount_out as i128)?;
		let inn = exec.value(t.asset_in, t.amount_in as i128)?;
		return Some(out.saturating_sub(inn));
	}

	let mut assets = sweep_assets(signer, flows, block_trade_assets, protocol, at, snapshots);
	// A collateral leg's aToken counter-leg is readable — sweep it so the supply/withdraw nets out
	// of the balances naturally (its leg is then NOT folded below).
	let ev = evm_address(signer);
	for f in per_ext.values() {
		for d in f.debt_legs.iter().filter(|d| d.collateral && d.who == ev) {
			if let Some(atk) = d.asset.and_then(|u| atoken_of.get(&u)) {
				assets.insert(*atk);
			}
		}
	}

	let mut net = Usd6::ZERO;
	for asset in assets {
		// A failed read = we can't fully account for the signer → refuse to suppress.
		let r = balances.real_delta(signer, asset, parent, at)?;
		if let Some(v) = exec.value(asset, r) {
			net = net.saturating_add(v);
		}
	}

	net = fold_aave_legs(net, signer, per_ext, exec, atoken_of);
	net = fold_reward_claims(net, signer, per_ext, exec);
	net = fold_foreign_credits(net, signer, per_ext, protocol, exec);
	net = exclude_dca_footprint(net, signer, ext_index, per_ext, exec);
	Some(net)
}

/// The confirmation sweep set: the signer's event-delta assets ∪ this ext's trade assets ∪ every
/// asset traded ANYWHERE in the block ∪ (for any swept stableswap SHARE) that pool's underlyings.
///
/// Block-wide, not per-ext: an uncaptured ERC20 cost leg (an aDOT unwound to DOT) emits no
/// substrate event for its own ext but appears in the block's trade graph. And a share received
/// from add-liquidity was paid for with the pool's Erc20 underlyings (HOLLAR/aUSDC), which emit no
/// substrate event — sweep them or the minted shares read as a gain. Both were live false positives.
pub fn sweep_assets<S: SnapshotProvider>(
	signer: &AccountId,
	flows: Option<&ExtrinsicFlows>,
	block_trade_assets: &BTreeSet<AssetId>,
	protocol: &ProtocolAccounts,
	at: BlockHash,
	snapshots: &S,
) -> BTreeSet<AssetId> {
	let empty = BTreeMap::new();
	let event_deltas = flows.and_then(|f| f.deltas.get(signer)).unwrap_or(&empty);
	let mut assets: BTreeSet<AssetId> = event_deltas
		.keys()
		.copied()
		.chain(flows.map(|f| f.trade_assets()).unwrap_or_default())
		.chain(block_trade_assets.iter().copied())
		.collect();

	let share_underlyings: Vec<AssetId> = assets
		.iter()
		.copied()
		.filter(|&a| protocol.stable_pool_account_of(a).is_some())
		.filter_map(|pool_id| snapshots.stableswap(pool_id, at).map(|s| s.assets))
		.flatten()
		.collect();
	assets.extend(share_underlyings);
	assets
}

/// Fold Aave positions across the WHOLE block (balance reads are block-level parent→at, so an
/// account that borrows in one ext and swaps in another must count the borrow offset too). A
/// borrow received an asset AND took on a debt token with no readable balance — subtract the debt
/// (+borrow) / add back cleared debt (−repay), uniformly `net -= value(asset, amount)`.
///
/// Collateral legs (supply/withdraw) whose aToken resolved in `atoken_of` are NOT folded — their
/// readable aToken counter-leg was swept instead. Unresolved collateral legs keep the fold (the
/// counter-leg really is invisible then).
fn fold_aave_legs(
	mut net: Usd6,
	signer: &AccountId,
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	exec: &ExecPrices,
	atoken_of: &BTreeMap<AssetId, AssetId>,
) -> Usd6 {
	let ev = evm_address(signer);
	for f in per_ext.values() {
		for d in f.debt_legs.iter().filter(|d| d.who == ev) {
			if let Some(asset) = d.asset {
				if d.collateral && atoken_of.contains_key(&asset) {
					continue;
				}
				if let Some(v) = exec.value(asset, d.amount) {
					net = net.saturating_sub(v);
				}
			}
		}
	}
	net
}

/// Fold credits the signer received in OTHER accounts' extrinsics of this block. Block-level
/// balance reads net an incoming same-block user transfer against this extrinsic's spend — which
/// turned a helper-funded Aave repay into phantom profit (the cost leg vanished, the repay fold
/// stayed). Only non-protocol sources fold: a pool paying the signer in someone else's extrinsic
/// is still unexplained gain.
fn fold_foreign_credits(
	mut net: Usd6,
	signer: &AccountId,
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	protocol: &ProtocolAccounts,
	exec: &ExecPrices,
) -> Usd6 {
	for f in per_ext.values().filter(|f| f.signer.as_ref() != Some(signer)) {
		for hop in f
			.transfers
			.iter()
			.filter(|h| h.to == *signer && !protocol.contains(&h.from))
		{
			if let Some(v) = exec.value(hop.asset, hop.amount as i128) {
				net = net.saturating_sub(v);
			}
		}
	}
	net
}

/// A sibling DCA segment's input is drawn from a past-block reserve, so only its OUTPUT lands in
/// the signer's block-level balance — a phantom gain from the perspective of any OTHER extrinsic
/// being confirmed. Subtract each sibling segment's output value (the segment itself is valued by
/// its router trade, never by balances). Over-corrects when the output was immediately converted
/// (e.g. into add-liq shares) — direction-safe: it can only suppress, never inflate.
fn exclude_dca_footprint(
	mut net: Usd6,
	signer: &AccountId,
	ext_index: u32,
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	exec: &ExecPrices,
) -> Usd6 {
	for (&idx, f) in per_ext {
		if idx == ext_index || f.signer.as_ref() != Some(signer) {
			continue;
		}
		if let Some(t) = &f.dca_trade {
			if let Some(v) = exec.value(t.asset_out, t.amount_out as i128) {
				net = net.saturating_sub(v);
			}
		}
	}
	net
}

/// Fold reward claims (earned yield, not extraction): staking pays native HDX, the LM pallets pay
/// their farm's reward currency (`RewardClaimed`). Subtract each claimed (currency, amount) so a
/// pure claim reads ~0 while any extraction on top still surfaces. Block-wide, like the balances.
fn fold_reward_claims(
	mut net: Usd6,
	signer: &AccountId,
	per_ext: &BTreeMap<u32, ExtrinsicFlows>,
	exec: &ExecPrices,
) -> Usd6 {
	let mut by_asset: BTreeMap<AssetId, u128> = BTreeMap::new();
	for f in per_ext.values() {
		for ((who, asset), amount) in &f.rewards {
			if who == signer {
				*by_asset.entry(*asset).or_default() += *amount;
			}
		}
	}
	for (asset, reward) in by_asset {
		if let Some(v) = exec.value(asset, reward as i128) {
			net = net.saturating_sub(v);
		}
	}
	net
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::{DebtLeg, Trade};
	use crate::reader::{OmnipoolAssetSnapshot, StableswapSnapshot, XykSnapshot};
	use crate::NATIVE_ASSET;

	const SIGNER: AccountId = [1u8; 32];
	const PARENT: BlockHash = [0xa; 32];
	const AT: BlockHash = [0xb; 32];

	/// A BalanceReader backed by explicit parent/at balances per (account, asset).
	#[derive(Default)]
	struct MockBalances {
		at: BTreeMap<(AccountId, AssetId), u128>,
		parent: BTreeMap<(AccountId, AssetId), u128>,
		fail: BTreeSet<AssetId>,
	}
	impl BalanceReader for MockBalances {
		fn own_balance(&self, who: &AccountId, asset: AssetId, at: BlockHash) -> Option<u128> {
			if self.fail.contains(&asset) {
				return None;
			}
			let m = if at == AT { &self.at } else { &self.parent };
			Some(*m.get(&(*who, asset)).unwrap_or(&0))
		}
	}

	struct NoSnapshots;
	impl SnapshotProvider for NoSnapshots {
		fn stableswap(&self, _: AssetId, _: BlockHash) -> Option<StableswapSnapshot> {
			None
		}
		fn omnipool_asset(&self, _: AssetId, _: BlockHash) -> Option<OmnipoolAssetSnapshot> {
			None
		}
		fn xyk(&self, _: &AccountId, _: BlockHash) -> Option<XykSnapshot> {
			None
		}
	}

	fn one_ext(flows: ExtrinsicFlows) -> BTreeMap<u32, ExtrinsicFlows> {
		BTreeMap::from([(2, flows)])
	}

	fn base(signer: AccountId) -> ExtrinsicFlows {
		ExtrinsicFlows {
			signer: Some(signer),
			..Default::default()
		}
	}

	// USDT=10. avDOT-pass-through class: events showed only DOT(5) +$4264, but the real avDOT(1005)
	// cost leg dropped equally → real net ~0. With balances reading both, the fold must net 0.
	#[test]
	fn confirm_should_net_zero_when_erc20_cost_leg_offsets_the_gain() {
		let mut flows = base(SIGNER);
		flows
			.deltas
			.insert(SIGNER, BTreeMap::from([(5, 49_579_168_469_756i128)]));
		// The block traded avDOT(1005)->DOT(5); block_trade_assets brings 1005 into the sweep.
		let block_assets = BTreeSet::from([5u32, 1005u32]);
		// Executed rates: price DOT and avDOT so out==in. Use a trade edge USDT->DOT and avDOT->DOT.
		let trades = vec![
			Trade {
				asset_in: 10,
				amount_in: 1_000_000,
				asset_out: 5,
				amount_out: 1_181_000_000,
				router: true,
			},
			Trade {
				asset_in: 1005,
				amount_in: 3_000_000_000_000,
				asset_out: 5,
				amount_out: 49_579_168_469_756,
				router: true,
			},
		];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		// Real: DOT +4957.9e9, avDOT -30000e8 (both parent 0 -> at the delta). Represent via at/parent.
		bal.at.insert((SIGNER, 5), 49_579_168_469_756);
		bal.at.insert((SIGNER, 1005), 0);
		bal.parent.insert((SIGNER, 1005), 3_000_000_000_000);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&block_assets,
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		// out(DOT) - in(avDOT) at exec rates ≈ 0.
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"avDOT unwind should net ~0, got {net}"
		);
	}

	#[test]
	fn confirm_should_return_none_when_a_balance_read_fails() {
		let mut flows = base(SIGNER);
		flows.deltas.insert(SIGNER, BTreeMap::from([(5, 100)]));
		let exec = ExecPrices::build(&[], &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		bal.fail.insert(5);
		let out = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		);
		assert_eq!(out, None, "a failed read must never suppress");
	}

	// A pure staking-reward claim: HDX balance rose by the reward, folded out → net ~0.
	#[test]
	fn confirm_should_fold_out_staking_reward_when_pure_claim() {
		let reward = 80_051_000_000_000u128; // ~80,051 HDX (12 dec)
		let mut flows = base(SIGNER);
		flows.rewards.insert((SIGNER, NATIVE_ASSET), reward);
		flows
			.deltas
			.insert(SIGNER, BTreeMap::from([(NATIVE_ASSET, reward as i128)]));
		// Price HDX via a USDT->HDX trade edge at $0.00597/HDX.
		let trades = vec![Trade {
			asset_in: 10,
			amount_in: 5_970,
			asset_out: NATIVE_ASSET,
			amount_out: 1_000_000_000_000,
			router: true,
		}];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		bal.at.insert((SIGNER, NATIVE_ASSET), reward);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"reward claim should net ~0, got {net}"
		);
	}

	// A pure LM reward claim paid in a NON-native currency (the 13342002 case: 266.75 HOLLAR
	// from omnipool liquidity mining): balance rose by the claim, folded out → net ~0.
	#[test]
	fn confirm_should_fold_out_lm_reward_claim_in_reward_currency() {
		const HOLLAR: AssetId = 222;
		let reward = 266_752_368_139_364_746_884u128; // 266.75 HOLLAR (18 dec)
		let mut flows = base(SIGNER);
		flows.rewards.insert((SIGNER, HOLLAR), reward);
		flows.deltas.insert(SIGNER, BTreeMap::from([(HOLLAR, reward as i128)]));
		// HOLLAR priced via a $1 confirmed anchor (as on mainnet).
		let anchors = std::collections::HashMap::from([(HOLLAR, 1e-12)]);
		let exec = ExecPrices::build(&[], &anchors, &Default::default());
		let mut bal = MockBalances::default();
		bal.at.insert((SIGNER, HOLLAR), reward);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"LM reward claim should net ~0, got {net}"
		);
	}

	// An Aave borrow: received DOT, took on DOT debt (no balance). Fold the debt leg → net ~0.
	#[test]
	fn confirm_should_fold_out_aave_borrow_debt() {
		let mut flows = base(SIGNER);
		flows
			.deltas
			.insert(SIGNER, BTreeMap::from([(5, 2_650_000_000_000i128)]));
		flows.debt_legs.push(DebtLeg {
			who: evm_address(&SIGNER),
			asset: Some(5),
			amount: 2_650_000_000_000,
			collateral: false,
		});
		let trades = vec![Trade {
			asset_in: 10,
			amount_in: 1_000_000,
			asset_out: 5,
			amount_out: 1_181_000_000,
			router: true,
		}];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		bal.at.insert((SIGNER, 5), 2_650_000_000_000);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"borrow should net ~0 after debt fold, got {net}"
		);
	}

	// An Aave SUPPLY whose aToken resolves (the 13358736 case: 440 DOT → 440 aDOT): balances
	// read both legs (−DOT +aDOT ≈ 0); the collateral leg must NOT be folded on top.
	#[test]
	fn confirm_should_not_fold_supply_when_atoken_is_swept() {
		const DOT: AssetId = 5;
		const ADOT: AssetId = 1001;
		let supplied = 4_399_992_792_862i128; // 440 DOT (10 dec)
		let mut flows = base(SIGNER);
		flows.deltas.insert(SIGNER, BTreeMap::from([(DOT, -supplied)]));
		flows.debt_legs.push(DebtLeg {
			who: evm_address(&SIGNER),
			asset: Some(DOT),
			amount: -supplied,
			collateral: true,
		});
		// Price DOT via USDT edge; aDOT via an aDOT->DOT edge (1:1).
		let trades = vec![
			Trade {
				asset_in: 10,
				amount_in: 755_000,
				asset_out: DOT,
				amount_out: 10_000_000_000,
				router: true,
			},
			Trade {
				asset_in: ADOT,
				amount_in: 1_000_000_000,
				asset_out: DOT,
				amount_out: 1_000_000_000,
				router: true,
			},
		];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		bal.parent.insert((SIGNER, DOT), supplied as u128);
		bal.at.insert((SIGNER, DOT), 0);
		bal.at.insert((SIGNER, ADOT), supplied as u128); // received aTokens 1:1
		let atoken_of = BTreeMap::from([(DOT, ADOT)]);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&atoken_of,
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"supply should net ~0 (sweep covers the aToken, no fold), got {net}"
		);
	}

	// The same supply when the aToken CANNOT be resolved: the counter-leg is invisible, so the
	// fold must still apply (pre-existing behavior preserved).
	#[test]
	fn confirm_should_fold_supply_when_atoken_unresolved() {
		const DOT: AssetId = 5;
		let supplied = 4_399_992_792_862i128;
		let mut flows = base(SIGNER);
		flows.deltas.insert(SIGNER, BTreeMap::from([(DOT, -supplied)]));
		flows.debt_legs.push(DebtLeg {
			who: evm_address(&SIGNER),
			asset: Some(DOT),
			amount: -supplied,
			collateral: true,
		});
		let trades = vec![Trade {
			asset_in: 10,
			amount_in: 755_000,
			asset_out: DOT,
			amount_out: 10_000_000_000,
			router: true,
		}];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		bal.parent.insert((SIGNER, DOT), supplied as u128);
		bal.at.insert((SIGNER, DOT), 0);
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"unresolved-aToken supply should net ~0 via the fold, got {net}"
		);
	}

	// A flash-funded Aave repay (the 13348493 case): the repay money arrived from a helper in
	// ANOTHER extrinsic of the same block, so the signer's block-level balance delta is 0; the
	// repay fold alone would read +$383. The foreign-credit fold must cancel it.
	#[test]
	fn confirm_should_fold_foreign_credit_when_repay_is_flash_funded() {
		const USDT: AssetId = 10;
		let amount = 383_372_009u128;
		let helper: AccountId = [7u8; 32];
		let mut repay_flows = base(SIGNER);
		repay_flows
			.deltas
			.insert(SIGNER, BTreeMap::from([(USDT, -(amount as i128))]));
		repay_flows.debt_legs.push(DebtLeg {
			who: evm_address(&SIGNER),
			asset: Some(USDT),
			amount: -(amount as i128),
			collateral: false,
		});
		let mut fund_flows = base(helper);
		fund_flows.transfers.push(crate::model::FlowHop {
			from: helper,
			to: SIGNER,
			asset: USDT,
			amount,
		});
		let per_ext = BTreeMap::from([(3, fund_flows), (4, repay_flows)]);
		let exec = ExecPrices::build(&[], &Default::default(), &Default::default());
		let bal = MockBalances::default(); // flash-funded: parent 0, at 0 → delta 0
		let net = signer_real_net(
			&SIGNER,
			4,
			&per_ext,
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		assert!(
			net.abs() <= Usd6::from_dollars(1.0),
			"flash-funded repay should net ~0, got {net}"
		);
	}

	// A sibling DCA segment's reserve-funded output (the 13364672 case: +$299 HEURC) must not
	// leak into ANOTHER extrinsic's confirmation.
	#[test]
	fn confirm_should_exclude_sibling_dca_output_from_other_ext() {
		const USDT: AssetId = 10;
		const HEURC: AssetId = 4444;
		let out = 298_940_972u128; // 298.94 HEURC (6 dec for test simplicity)
		let mut trade_flows = base(SIGNER);
		trade_flows
			.deltas
			.insert(SIGNER, BTreeMap::from([(USDT, -8_230_000i128)]));
		let mut dca_flows = base(SIGNER);
		dca_flows.dca_trade = Some(Trade {
			asset_in: USDT,
			amount_in: 298_940_972,
			asset_out: HEURC,
			amount_out: out,
			router: true,
		});
		let per_ext = BTreeMap::from([(2, trade_flows), (1_000_001, dca_flows)]);
		let trades = vec![Trade {
			asset_in: USDT,
			amount_in: 1_000_000,
			asset_out: HEURC,
			amount_out: 1_000_000,
			router: true,
		}];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let mut bal = MockBalances::default();
		// Balance shows the DCA's output (reserve-funded input invisible) and the trade's spend.
		bal.at.insert((SIGNER, HEURC), out);
		bal.parent.insert((SIGNER, USDT), 8_230_000);
		bal.at.insert((SIGNER, USDT), 0);
		// HEURC is swept because the DCA's router trade puts it in the block's trade assets.
		let net = signer_real_net(
			&SIGNER,
			2,
			&per_ext,
			&BTreeSet::from([USDT, HEURC]),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		// The DCA's +$298.94 is excluded; ext 2's own −$8.23 spend remains.
		assert!(
			net <= Usd6::ZERO && net >= Usd6::from_dollars(-10.0),
			"sibling DCA output must not inflate ext 2, got {net}"
		);
	}

	// A DCA execution: valued by its router trade (out − in), never the reserve-distorted balance.
	#[test]
	fn confirm_should_value_dca_by_router_trade_not_balance() {
		let mut flows = base(SIGNER);
		flows.dca_trade = Some(Trade {
			asset_in: 63,
			amount_in: 1_000_000,
			asset_out: 10,
			amount_out: 999_800,
			router: true,
		});
		let trades = vec![Trade {
			asset_in: 63,
			amount_in: 1_000_000,
			asset_out: 10,
			amount_out: 999_800,
			router: true,
		}];
		let exec = ExecPrices::build(&trades, &Default::default(), &Default::default());
		let bal = MockBalances::default(); // never consulted for a DCA
		let net = signer_real_net(
			&SIGNER,
			2,
			&one_ext(flows),
			&BTreeSet::new(),
			&ProtocolAccounts::default(),
			&BTreeMap::new(),
			PARENT,
			AT,
			&exec,
			&bal,
			&NoSnapshots,
		)
		.unwrap();
		// 999_800 out - 1_000_000 in (priced via the edge) = -200 raw-USDT, a small fee-like loss.
		assert!(
			net <= Usd6::ZERO && net >= Usd6(-1_000),
			"DCA nets slightly negative (fee), got {net}"
		);
	}
}
