//! Reduce a block's normalized events into per-extrinsic flows.
//!
//! Pure: consumes the mapper's `(Phase, ChainEvent)` stream in event order (no runtime types, no
//! bytes) and produces `per_ext` + the block's trade list. Hook-phase events are kept — DCA
//! executions become attributed pseudo-extrinsics (the schedule owner is the economic actor),
//! everything else lands in a shared bucket at `HOOK_EXT_BASE`.

use crate::accounts::ProtocolAccounts;
use crate::model::{ChainEvent, ExtrinsicFlows, FlowHop, Phase, Trade, HOOK_EXT_BASE};
use crate::{AccountId, AssetId};
use std::collections::{BTreeMap, HashSet};

/// The reduced view of one block.
#[derive(Clone, Debug, Default)]
pub struct BlockData {
	pub per_ext: BTreeMap<u32, ExtrinsicFlows>,
	/// Every executed trade in the block — the edges of the executed-price graph.
	pub trades: Vec<Trade>,
}

type LegKey = (AssetId, Option<AccountId>, Option<AccountId>, u128);

pub fn reduce(events: &[(Phase, ChainEvent)], protocol: &ProtocolAccounts) -> BlockData {
	let mut per_ext: BTreeMap<u32, ExtrinsicFlows> = BTreeMap::new();
	let mut trades: Vec<Trade> = Vec::new();
	let mut hook_seq = HOOK_EXT_BASE;
	let mut open_dca: Option<(u32, u32)> = None; // (schedule id, pseudo-ext index)
											  // Per-ext dedup: an orml move via Currencies emits BOTH tokens.* and currencies.* with an
											  // identical (asset, from, to, amount); count it once. Keyed per ext so the same tuple in a
											  // different extrinsic still counts.
	let mut seen_legs: BTreeMap<u32, HashSet<LegKey>> = BTreeMap::new();

	for (phase, event) in events {
		if let ChainEvent::Trade(t) = event {
			trades.push(t.clone());
		}

		let ext_index = match phase {
			Phase::ApplyExtrinsic(i) => *i,
			Phase::Hook => {
				match event {
					ChainEvent::DcaStarted { schedule_id } => {
						hook_seq += 1;
						open_dca = Some((*schedule_id, hook_seq));
					}
					ChainEvent::DcaClosed { schedule_id, owner } => {
						if let Some((open_id, idx)) = &open_dca {
							if open_id == schedule_id {
								per_ext.entry(*idx).or_default().signer = Some(*owner);
								open_dca = None;
							}
						}
					}
					_ => {}
				}
				open_dca.map(|(_, idx)| idx).unwrap_or(HOOK_EXT_BASE)
			}
		};

		let flows = per_ext.entry(ext_index).or_default();

		match event {
			// A DCA segment's net swap is its ROUTER trade; capture it so the DCA is valued by
			// out−in (fair) rather than the owner's reserve-distorted balance delta.
			ChainEvent::Trade(t) => {
				if ext_index > HOOK_EXT_BASE && t.router {
					flows.dca_trade = Some(t.clone());
				}
				flows.trades.push(t.clone());
			}
			ChainEvent::FeePaid { who, amount } => {
				flows.signer = Some(*who);
				flows.fee_paid = *amount;
			}
			ChainEvent::Liquidity => flows.has_lp_event = true,
			ChainEvent::Notable(n) => flows.notable.push(n.clone()),
			ChainEvent::AavePosition(d) => flows.debt_legs.push(d.clone()),
			ChainEvent::RewardClaimed { who, asset, amount } => {
				*flows.rewards.entry((*who, *asset)).or_default() += *amount;
			}
			ChainEvent::Transfer(leg) => {
				let key: LegKey = (leg.asset, leg.from, leg.to, leg.amount);
				if !seen_legs.entry(ext_index).or_default().insert(key) {
					continue;
				}
				record_transfer(flows, protocol, leg.asset, leg.from, leg.to, leg.amount);
			}
			// Boundaries only steered ext_index above; the entry was already created by `or_default`.
			// Inert events exist solely to create that entry so the extrinsic is counted.
			ChainEvent::DcaStarted { .. } | ChainEvent::DcaClosed { .. } | ChainEvent::Inert => {}
		}
	}

	BlockData { per_ext, trades }
}

fn record_transfer(
	flows: &mut ExtrinsicFlows,
	protocol: &ProtocolAccounts,
	asset: AssetId,
	from: Option<AccountId>,
	to: Option<AccountId>,
	amount: u128,
) {
	match (from, to) {
		// account→account: an ordered hop for the flow path.
		(Some(from), Some(to)) => flows.transfers.push(FlowHop {
			from,
			to,
			asset,
			amount,
		}),
		// One-sided mint/burn of a stableswap SHARE (add/remove liquidity): the pool is the
		// invisible counterparty, synthesized for the flow path ONLY. Delta/residual accounting
		// below keeps the honest one-sided view (residual still flags the mint).
		_ => {
			if let Some(pool) = protocol.stable_pool_account_of(asset) {
				let hop = match (from, to) {
					(None, Some(to)) => Some((*pool, to)),
					(Some(from), None) => Some((from, *pool)),
					_ => None,
				};
				if let Some((f, t)) = hop.filter(|(f, t)| f != t) {
					flows.transfers.push(FlowHop {
						from: f,
						to: t,
						asset,
						amount,
					});
				}
			}
		}
	}

	if let Some(from) = from {
		*flows.deltas.entry(from).or_default().entry(asset).or_default() -= amount as i128;
		*flows.residual.entry(asset).or_default() -= amount as i128;
		if protocol.contains(&from) {
			flows.touched_pool = true;
		}
	}
	if let Some(to) = to {
		*flows.deltas.entry(to).or_default().entry(asset).or_default() += amount as i128;
		*flows.residual.entry(asset).or_default() += amount as i128;
		if protocol.contains(&to) {
			flows.touched_pool = true;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::model::TransferLeg as Leg;

	const USER: AccountId = [1u8; 32];
	const POOL: AccountId = [9u8; 32]; // stableswap pool account (registered below)

	fn transfer(asset: AssetId, from: Option<AccountId>, to: Option<AccountId>, amount: u128) -> ChainEvent {
		ChainEvent::Transfer(Leg {
			asset,
			from,
			to,
			amount,
		})
	}

	fn ext(i: u32) -> Phase {
		Phase::ApplyExtrinsic(i)
	}

	#[test]
	fn reduce_should_dedup_identical_tokens_and_currencies_legs_in_one_ext() {
		// The same orml move emitted as tokens.* and currencies.* — must count once.
		let leg = transfer(222, Some(USER), Some(POOL), 100);
		let events = vec![(ext(2), leg.clone()), (ext(2), leg)];
		let bd = reduce(&events, &ProtocolAccounts::default());
		let d = bd.per_ext[&2].deltas[&USER][&222];
		assert_eq!(d, -100, "duplicate leg must not double-count");
	}

	#[test]
	fn reduce_should_count_same_leg_separately_in_different_exts() {
		let leg = transfer(222, Some(USER), Some(POOL), 100);
		let events = vec![(ext(2), leg.clone()), (ext(3), leg)];
		let bd = reduce(&events, &ProtocolAccounts::default());
		assert_eq!(bd.per_ext[&2].deltas[&USER][&222], -100);
		assert_eq!(bd.per_ext[&3].deltas[&USER][&222], -100);
	}

	#[test]
	fn reduce_should_set_touched_pool_when_counterparty_is_protocol() {
		let mut modl = [0u8; 32];
		modl[..4].copy_from_slice(b"modl");
		let events = vec![(ext(2), transfer(10, Some(USER), Some(modl), 500))];
		let bd = reduce(&events, &ProtocolAccounts::default());
		assert!(bd.per_ext[&2].touched_pool);
	}

	#[test]
	fn reduce_should_attribute_dca_segment_to_owner_and_capture_router_trade() {
		let sched = 33604u32;
		let owner = [7u8; 32];
		let router_trade = Trade {
			asset_in: 1,
			amount_in: 100,
			asset_out: 10,
			amount_out: 99,
			router: true,
		};
		let hop = Trade {
			asset_in: 1,
			amount_in: 100,
			asset_out: 10,
			amount_out: 99,
			router: false,
		};
		let events = vec![
			(Phase::Hook, ChainEvent::DcaStarted { schedule_id: sched }),
			(Phase::Hook, ChainEvent::Trade(hop)),
			(Phase::Hook, ChainEvent::Trade(router_trade.clone())),
			(
				Phase::Hook,
				ChainEvent::DcaClosed {
					schedule_id: sched,
					owner,
				},
			),
		];
		let bd = reduce(&events, &ProtocolAccounts::default());
		// The segment is the first pseudo-ext above HOOK_EXT_BASE.
		let seg = bd.per_ext.get(&(HOOK_EXT_BASE + 1)).expect("dca segment");
		assert_eq!(seg.signer, Some(owner));
		assert_eq!(
			seg.dca_trade,
			Some(router_trade),
			"dca valued by router trade, not the hop"
		);
		assert_eq!(bd.trades.len(), 2, "both trades still feed the price graph");
	}

	#[test]
	fn reduce_should_bucket_unattributed_hook_events_at_hook_base() {
		let events = vec![(Phase::Hook, transfer(10, Some([2u8; 32]), Some([3u8; 32]), 5))];
		let bd = reduce(&events, &ProtocolAccounts::default());
		assert!(bd.per_ext.contains_key(&HOOK_EXT_BASE));
	}

	#[test]
	fn reduce_should_synthesize_pool_hop_for_one_sided_share_mint() {
		// Register pool 222 → POOL account; a one-sided mint of share asset 222 to USER
		// synthesizes POOL→USER for the flow path, but USER's delta stays the honest +100.
		let pa = ProtocolAccounts {
			stable_pools: vec![(222, POOL)],
			..Default::default()
		};
		let events = vec![(ext(2), transfer(222, None, Some(USER), 100))];
		let bd = reduce(&events, &pa);
		let f = &bd.per_ext[&2];
		assert_eq!(f.transfers.len(), 1);
		assert_eq!(f.transfers[0].from, POOL);
		assert_eq!(f.transfers[0].to, USER);
		assert_eq!(f.deltas[&USER][&222], 100);
	}
}
