// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! version-tolerant reader for a block's `System::Events`.
//!
//! synth logs are translated client-side from events decoded with the *node's*
//! compiled `RuntimeEvent`. that assumption breaks whenever the node is built
//! against a different polkadot-sdk than the on-chain runtime, and it breaks
//! silently: stable2603 inserted `MintedCredit`(11) and `BurnedDebt`(13) into
//! `pallet_balances::Event`, shifting every later variant, so a stable2603 node
//! (v49.2.0) reading a stable2506 runtime could not decode any block that paid
//! fees in native hdx — and dropped that block's synth logs entirely, including
//! the wormhole `LogMessagePublished` raised by substrate-dispatched `EVM.call`.
//!
//! `read_events` decodes with the compiled types first (zero cost when the
//! versions agree) and otherwise re-reads the balances events in an explicit
//! per-sdk layout, so a skew degrades into a slower path instead of silence.
//! the caller is expected to log which path was taken.

use codec::{Decode, Input};
use frame_support::traits::{tokens::BalanceStatus, PalletInfoAccess};
use frame_system::{EventRecord, Phase};
use hydradx_runtime::{Balances, Runtime, RuntimeEvent};
use pallet_balances::Event as BalancesEvent;
use primitives::AccountId;
use sp_core::H256;
use std::collections::BTreeSet;

type Records = Vec<EventRecord<RuntimeEvent, H256>>;
type LogKey = (sp_core::H160, Vec<H256>, Vec<u8>);
type Balance = u128;
type NodeBalancesEvent = BalancesEvent<Runtime>;

/// how a block's events were read.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
	/// the node's compiled `RuntimeEvent` matched the chain's encoding.
	Native,
	/// read through a compat balances layout — the node and the on-chain runtime
	/// are on different polkadot-sdk versions, but nothing was lost.
	Compat,
	/// resynchronised past records this node cannot decode at all. `skipped` events are
	/// missing, `recovered` were salvaged out of the skipped regions as `EVM.Log`, and
	/// `trailing` bytes were unparseable; everything else survived.
	Partial {
		skipped: usize,
		recovered: usize,
		trailing: usize,
	},
}

/// Decode `System::Events` without letting one undecodable record cost the whole
/// block. Three tiers, in order of fidelity:
///
/// 1. the node's compiled `RuntimeEvent` — free, and exact when node and runtime
///    agree;
/// 2. an explicit per-sdk `pallet_balances::Event` layout — exact for skews we
///    know about;
/// 3. resync scan — drops only the records that cannot be decoded.
///
/// `None` means not even one record was readable.
pub fn read_events(raw: &[u8]) -> Option<(Records, Source)> {
	if let Ok(records) = Records::decode(&mut &raw[..]) {
		return Some((records, Source::Native));
	}
	// add a layout here when a future sdk reorders `pallet_balances::Event`;
	// `node_balances_layout_is_known` fails until one is present.
	if let Some(records) = decode_records::<BalancesEvent2506>(raw) {
		return Some((records, Source::Compat));
	}
	// unknown skew: salvage per-record rather than returning an empty block. events
	// raised after the undecodable ones — `EVM.Log`, and with it wormhole's
	// `LogMessagePublished` — are what this tier exists to preserve.
	let (records, skipped, recovered, trailing) = read_resilient(raw)?;
	(!records.is_empty()).then_some((
		records,
		Source::Partial {
			skipped,
			recovered,
			trailing,
		},
	))
}

/// One record: `phase`, event, `topics`. Leaves the cursor untouched on failure so
/// the caller can resync from where the record began.
fn decode_record(input: &mut &[u8]) -> Option<EventRecord<RuntimeEvent, H256>> {
	let checkpoint = *input;
	let decode = |input: &mut &[u8]| {
		let phase = Phase::decode(input).ok()?;
		let event = RuntimeEvent::decode(input).ok()?;
		let topics = Vec::<H256>::decode(input).ok()?;
		Some(EventRecord { phase, event, topics })
	};
	let record = decode(input);
	if record.is_none() {
		*input = checkpoint;
	}
	record
}

/// Could `bytes` begin a record? `Phase` is a tight anchor: `ApplyExtrinsic(u32)`,
/// `Finalization`, `Initialization` — and a block never holds 2^16 extrinsics.
fn is_phase_start(bytes: &[u8]) -> bool {
	match bytes.first() {
		Some(0) => bytes.len() >= 5 && bytes[3] == 0 && bytes[4] == 0,
		Some(1) | Some(2) => true,
		_ => false,
	}
}

/// One record, but only if it is an `EVM.Log`.
///
/// This is the salvage path, and it works where the generic one cannot: an `EVM.Log`
/// record is `phase ‖ pallet ‖ variant ‖ address ‖ topics ‖ data ‖ record_topics`, none
/// of which depends on a pallet whose layout an sdk bump moved. So wormhole's core-bridge
/// `LogMessagePublished` stays readable even when the record beside it does not decode.
///
/// Guards against parsing garbage: the pallet byte must be this runtime's EVM index, the
/// event must be the `Log` variant, at most 4 log topics (the evm cap), and the record's
/// own topic vec must be empty — `deposit_event` never sets topics for these.
fn decode_evm_log_record(input: &mut &[u8], evm_index: u8) -> Option<EventRecord<RuntimeEvent, H256>> {
	let checkpoint = *input;
	let decode = |input: &mut &[u8]| {
		let phase = Phase::decode(input).ok()?;
		if Input::read_byte(input).ok()? != evm_index {
			return None;
		}
		let event = pallet_evm::Event::<Runtime>::decode(input).ok()?;
		match &event {
			pallet_evm::Event::Log { log } if log.topics.len() <= 4 => {}
			_ => return None,
		}
		let topics = Vec::<H256>::decode(input).ok()?;
		if !topics.is_empty() {
			return None;
		}
		Some(EventRecord {
			phase,
			event: RuntimeEvent::EVM(event),
			topics,
		})
	};
	let record = decode(input);
	if record.is_none() {
		*input = checkpoint;
	}
	record
}

/// Walk a region we are about to skip and pull out every `EVM.Log` record in it, so a
/// resync never costs a wormhole event.
fn salvage_evm_logs(region: &[u8], evm_index: u8, out: &mut Records) -> usize {
	let mut off = 0usize;
	let mut found = 0usize;
	while off < region.len() {
		let mut probe = &region[off..];
		match decode_evm_log_record(&mut probe, evm_index) {
			Some(record) => {
				let consumed = region.len() - off - probe.len();
				out.push(record);
				found += 1;
				off += consumed.max(1);
			}
			None => off += 1,
		}
	}
	found
}

/// Decode as many records as possible, stepping over any this node's types cannot
/// parse, salvaging `EVM.Log` records out of the skipped regions. Returns
/// `(records, skipped, recovered, trailing_bytes)`.
///
/// Recovery relies on `Phase` being a tight anchor plus the next record decoding
/// cleanly from the candidate offset; a mis-landing costs extra skipped records,
/// never a wrong record.
fn read_resilient(raw: &[u8]) -> Option<(Records, usize, usize, usize)> {
	let evm_index = <hydradx_runtime::EVM as PalletInfoAccess>::index() as u8;
	let mut input = raw;
	let count = codec::Compact::<u32>::decode(&mut input).ok()?.0 as usize;
	let mut records = Records::new();
	let mut skipped = 0usize;
	let mut recovered = 0usize;

	while !input.is_empty() && records.len() + skipped < count {
		if let Some(record) = decode_record(&mut input) {
			records.push(record);
			continue;
		}
		// scan for the next offset that yields a decodable record.
		let bad = input;
		let resync = (1..bad.len()).find(|&off| {
			let candidate = &bad[off..];
			if !is_phase_start(candidate) {
				return false;
			}
			let mut probe = candidate;
			decode_record(&mut probe).is_some()
		});
		match resync {
			Some(off) => {
				input = &bad[off..];
				skipped += 1;
			}
			None => break,
		}
	}
	// A record whose layout this node gets wrong can still "decode" while consuming the
	// wrong byte count, swallowing whatever followed it — so the sequential walk above can
	// lose an EVM.Log without ever reporting a skip. Sweep the whole blob independently and
	// merge back anything missing. This is what makes "never lose a wormhole event" hold.
	let mut swept = Records::new();
	salvage_evm_logs(raw, evm_index, &mut swept);
	let have: BTreeSet<LogKey> = records.iter().filter_map(evm_log_key).collect();
	for record in swept {
		if evm_log_key(&record).map(|k| !have.contains(&k)).unwrap_or(false) {
			records.push(record);
			recovered += 1;
		}
	}

	Some((records, skipped, recovered, input.len()))
}

/// Identity of an `EVM.Log` record, for merging a sweep without duplicating.
fn evm_log_key(record: &EventRecord<RuntimeEvent, H256>) -> Option<LogKey> {
	match &record.event {
		RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => Some((log.address, log.topics.clone(), log.data.clone())),
		_ => None,
	}
}

/// Read records one at a time, decoding the balances pallet with `B`'s layout and
/// delegating every other pallet to the node's compiled `RuntimeEvent`.
///
/// pallet indices come from hydration's own `construct_runtime`, so they are
/// stable across sdk bumps; only the inner variant order moves.
fn decode_records<B>(raw: &[u8]) -> Option<Records>
where
	B: Decode + Into<NodeBalancesEvent>,
{
	let balances = <Balances as PalletInfoAccess>::index() as u8;
	let mut input = raw;
	let count = codec::Compact::<u32>::decode(&mut input).ok()?.0;
	// no `with_capacity`: `count` is attacker-controlled if state is corrupt.
	let mut records = Records::new();
	for _ in 0..count {
		let phase = Phase::decode(&mut input).ok()?;
		// `&[u8]` is `Copy`, so keep a cursor to rewind after peeking the pallet byte.
		let rewind = input;
		let event = if Input::read_byte(&mut input).ok()? == balances {
			RuntimeEvent::Balances(B::decode(&mut input).ok()?.into())
		} else {
			input = rewind;
			RuntimeEvent::decode(&mut input).ok()?
		};
		let topics = Vec::<H256>::decode(&mut input).ok()?;
		records.push(EventRecord { phase, event, topics });
	}
	// a wrong layout can still decode `count` records by chance; trailing bytes
	// prove it was the wrong one.
	input.is_empty().then_some(records)
}

/// `pallet_balances::Event` as encoded by polkadot-sdk stable2506 — the layout
/// the on-chain runtime uses. Variants synth ignores are still listed: they must
/// consume their bytes exactly or the cursor desynchronises.
#[derive(Decode)]
enum BalancesEvent2506 {
	#[codec(index = 0)]
	Endowed { account: AccountId, free_balance: Balance },
	#[codec(index = 1)]
	DustLost { account: AccountId, amount: Balance },
	#[codec(index = 2)]
	Transfer {
		from: AccountId,
		to: AccountId,
		amount: Balance,
	},
	#[codec(index = 3)]
	BalanceSet { who: AccountId, free: Balance },
	#[codec(index = 4)]
	Reserved { who: AccountId, amount: Balance },
	#[codec(index = 5)]
	Unreserved { who: AccountId, amount: Balance },
	#[codec(index = 6)]
	ReserveRepatriated {
		from: AccountId,
		to: AccountId,
		amount: Balance,
		destination_status: BalanceStatus,
	},
	#[codec(index = 7)]
	Deposit { who: AccountId, amount: Balance },
	#[codec(index = 8)]
	Withdraw { who: AccountId, amount: Balance },
	#[codec(index = 9)]
	Slashed { who: AccountId, amount: Balance },
	#[codec(index = 10)]
	Minted { who: AccountId, amount: Balance },
	#[codec(index = 11)]
	Burned { who: AccountId, amount: Balance },
	#[codec(index = 12)]
	Suspended { who: AccountId, amount: Balance },
	#[codec(index = 13)]
	Restored { who: AccountId, amount: Balance },
	#[codec(index = 14)]
	Upgraded { who: AccountId },
	#[codec(index = 15)]
	Issued { amount: Balance },
	#[codec(index = 16)]
	Rescinded { amount: Balance },
	#[codec(index = 17)]
	Locked { who: AccountId, amount: Balance },
	#[codec(index = 18)]
	Unlocked { who: AccountId, amount: Balance },
	#[codec(index = 19)]
	Frozen { who: AccountId, amount: Balance },
	#[codec(index = 20)]
	Thawed { who: AccountId, amount: Balance },
	#[codec(index = 21)]
	TotalIssuanceForced { old: Balance, new: Balance },
}

impl From<BalancesEvent2506> for NodeBalancesEvent {
	fn from(e: BalancesEvent2506) -> Self {
		use BalancesEvent2506 as E;
		match e {
			E::Endowed { account, free_balance } => Self::Endowed { account, free_balance },
			E::DustLost { account, amount } => Self::DustLost { account, amount },
			E::Transfer { from, to, amount } => Self::Transfer { from, to, amount },
			E::BalanceSet { who, free } => Self::BalanceSet { who, free },
			E::Reserved { who, amount } => Self::Reserved { who, amount },
			E::Unreserved { who, amount } => Self::Unreserved { who, amount },
			E::ReserveRepatriated {
				from,
				to,
				amount,
				destination_status,
			} => Self::ReserveRepatriated {
				from,
				to,
				amount,
				destination_status,
			},
			E::Deposit { who, amount } => Self::Deposit { who, amount },
			E::Withdraw { who, amount } => Self::Withdraw { who, amount },
			E::Slashed { who, amount } => Self::Slashed { who, amount },
			E::Minted { who, amount } => Self::Minted { who, amount },
			E::Burned { who, amount } => Self::Burned { who, amount },
			E::Suspended { who, amount } => Self::Suspended { who, amount },
			E::Restored { who, amount } => Self::Restored { who, amount },
			E::Upgraded { who } => Self::Upgraded { who },
			E::Issued { amount } => Self::Issued { amount },
			E::Rescinded { amount } => Self::Rescinded { amount },
			E::Locked { who, amount } => Self::Locked { who, amount },
			E::Unlocked { who, amount } => Self::Unlocked { who, amount },
			E::Frozen { who, amount } => Self::Frozen { who, amount },
			E::Thawed { who, amount } => Self::Thawed { who, amount },
			E::TotalIssuanceForced { old, new } => Self::TotalIssuanceForced { old, new },
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use codec::Encode;

	/// `System::Events` as encoded by mainnet at block 13386492 (spec 433,
	/// stable2506): 84 events over `Timestamp.set`, `ParachainSystem`,
	/// `Omnipool.sell` and a `Utility.batch_all([EVM.call, EVM.call])` that burns
	/// DAI through the ntt spoke manager and publishes wormhole message seq 5.
	/// Native hdx fees make it carry `Balances::{Withdraw, Rescinded, Burned,
	/// Minted, Deposit, Issued}` — the variants a skewed layout chokes on.
	const MAINNET_EVENTS: &[u8] = include_bytes!("test_data/events_13386492.scale");

	/// `System::Events` from mainnet block 13384676 (50 records) — the DAI transfer that
	/// published wormhole sequence 2. This block also carries `ConvictionVoting.Voted` and
	/// `Scheduler.{Scheduled,Canceled}`, whose layouts moved between stable2506 and
	/// stable2603 too, so a skewed node cannot decode it even through the balances compat
	/// layout. It is the regression case for "never lose a wormhole event".
	const MAINNET_EVENTS_SEQ2: &[u8] = include_bytes!("test_data/events_13384676.scale");

	/// keccak256("LogMessagePublished(address,uint64,uint32,bytes,uint8)")
	const LOG_MESSAGE_PUBLISHED: [u8; 32] =
		hex_literal::hex!("6eb224fb001ed210e379b335e35efe88672a8ce935d981a6896b27ffdf52a3b2");
	/// hydration wormhole core bridge
	const CORE_BRIDGE: [u8; 20] = hex_literal::hex!("3792a6d63c31941B2805181771795D9176fA82A1");

	fn variant_index(event: NodeBalancesEvent) -> u8 {
		event.encode()[0]
	}

	/// The guard that fires on the next sdk bump. Synth reads `System::Events`
	/// with the node's compiled `RuntimeEvent`; if an sdk reorders
	/// `pallet_balances::Event`, a node built on it can no longer decode a runtime
	/// built on another, and every affected block silently loses its synth logs.
	/// When this fails: either align the node's sdk with the runtime's, or add the
	/// new layout to `read_events`.
	#[test]
	fn node_balances_layout_is_known() {
		let who = || AccountId::new([0u8; 32]);
		// one index per variant synth reads or has to step over.
		let transfer = NodeBalancesEvent::Transfer {
			from: who(),
			to: who(),
			amount: 0,
		};
		let fingerprint = [
			("Transfer", variant_index(transfer)),
			(
				"Deposit",
				variant_index(NodeBalancesEvent::Deposit { who: who(), amount: 0 }),
			),
			(
				"Withdraw",
				variant_index(NodeBalancesEvent::Withdraw { who: who(), amount: 0 }),
			),
			(
				"Slashed",
				variant_index(NodeBalancesEvent::Slashed { who: who(), amount: 0 }),
			),
			(
				"Minted",
				variant_index(NodeBalancesEvent::Minted { who: who(), amount: 0 }),
			),
			(
				"Burned",
				variant_index(NodeBalancesEvent::Burned { who: who(), amount: 0 }),
			),
			("Issued", variant_index(NodeBalancesEvent::Issued { amount: 0 })),
			("Rescinded", variant_index(NodeBalancesEvent::Rescinded { amount: 0 })),
			(
				"Locked",
				variant_index(NodeBalancesEvent::Locked { who: who(), amount: 0 }),
			),
			(
				"Frozen",
				variant_index(NodeBalancesEvent::Frozen { who: who(), amount: 0 }),
			),
		];

		// layouts `read_events` can decode. add an entry (and a mirror enum) when a
		// new sdk shows up here.
		const STABLE2506: [u8; 10] = [2, 7, 8, 9, 10, 11, 15, 16, 17, 19];
		const STABLE2603: [u8; 10] = [2, 7, 8, 9, 10, 12, 17, 18, 19, 21];

		let indices: Vec<u8> = fingerprint.iter().map(|(_, i)| *i).collect();
		assert!(
			indices == STABLE2506 || indices == STABLE2603,
			"pallet_balances::Event layout is not one this node can read: {fingerprint:?}.\n\
			 synth logs decode System::Events with the node's compiled RuntimeEvent; an unknown \
			 layout means every block with native-hdx balance events silently loses ALL its synth \
			 logs (this is what broke wormhole LogMessagePublished in v49.2.0).\n\
			 fix: align the node's polkadot-sdk with the on-chain runtime's, or add this layout to \
			 compat_events::read_events."
		);
	}

	/// The reader must recover mainnet-encoded events whatever sdk the node was
	/// built on — natively when the layouts agree, through the compat path when
	/// they don't.
	#[test]
	fn reads_mainnet_events_on_any_node_sdk() {
		let (records, _source) = read_events(MAINNET_EVENTS).expect("mainnet events must be readable");
		assert_eq!(records.len(), 84, "block 13386492 has 84 events");

		let evm_logs: Vec<_> = records
			.iter()
			.filter_map(|r| match &r.event {
				RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => Some(log),
				_ => None,
			})
			.collect();
		// the batch_all alone raises 6 (approve, core bridge, 2x transceiver, 2x manager).
		assert!(
			evm_logs.len() >= 6,
			"expected the EVM.call logs, got {}",
			evm_logs.len()
		);

		// the whole point: the wormhole message must survive the read.
		let published = evm_logs
			.iter()
			.find(|log| log.address.0 == CORE_BRIDGE && log.topics.first().map(|t| t.0) == Some(LOG_MESSAGE_PUBLISHED))
			.expect("core-bridge LogMessagePublished must be readable from a substrate EVM.call");
		// LogMessagePublished(sender indexed, sequence, nonce, payload, consistency)
		let sequence = u64::from_be_bytes(published.data[24..32].try_into().unwrap());
		assert_eq!(sequence, 5, "wormhole sequence 5");
	}

	/// Pins which tier does the work, so the diagnosis itself is under test: the
	/// fixture is stable2506-encoded, so a node on that sdk reads it natively and a
	/// skewed one (v49.2.0 / stable2603) must not manage it natively. Either way
	/// recovery has to be exact — `Partial` here would mean events were lost.
	#[test]
	fn skewed_node_recovers_exactly_not_partially() {
		let native_ok = Records::decode(&mut &MAINNET_EVENTS[..]).is_ok();
		let (records, source) = read_events(MAINNET_EVENTS).expect("must be readable");
		println!("mainnet fixture read via {source:?} (native decode ok: {native_ok})");
		assert_eq!(records.len(), 84);
		if native_ok {
			assert_eq!(source, Source::Native, "node sdk matches the chain's");
		} else {
			assert_eq!(
				source,
				Source::Compat,
				"a known layout skew must be recovered exactly; Partial means events were dropped"
			);
		}
	}

	/// The last-resort tier, exercised directly: whatever this node cannot decode,
	/// the records around it must still come through. On a node whose sdk matches
	/// the chain this decodes cleanly; on a skewed one it steps over the balances
	/// events and still recovers the wormhole message that follows them.
	#[test]
	fn resync_preserves_logs_around_undecodable_records() {
		let (records, skipped, _recovered, trailing) = read_resilient(MAINNET_EVENTS).expect("must salvage records");
		assert!(!records.is_empty(), "a skew must never cost the whole block");

		let published = records.iter().any(|r| match &r.event {
			RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => {
				log.address.0 == CORE_BRIDGE && log.topics.first().map(|t| t.0) == Some(LOG_MESSAGE_PUBLISHED)
			}
			_ => false,
		});
		assert!(
			published,
			"LogMessagePublished must survive resync (skipped {skipped}, trailing {trailing})"
		);
	}

	/// The hard guarantee: whatever this node cannot decode, a wormhole core-bridge
	/// `LogMessagePublished` must still come out. Block 13384676 defeats both the typed
	/// path and the balances compat layout on a skewed node, so this exercises the salvage
	/// scan on real data.
	#[test]
	fn wormhole_event_survives_undecodable_neighbours() {
		let (records, source) = read_events(MAINNET_EVENTS_SEQ2).expect("must be readable");
		println!("seq2 fixture read via {source:?}, {} records", records.len());

		let published = records
			.iter()
			.filter_map(|r| match &r.event {
				RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => Some(log),
				_ => None,
			})
			.find(|log| log.address.0 == CORE_BRIDGE && log.topics.first().map(|t| t.0) == Some(LOG_MESSAGE_PUBLISHED))
			.expect("core-bridge LogMessagePublished must survive whatever else fails");

		let sequence = u64::from_be_bytes(published.data[24..32].try_into().unwrap());
		assert_eq!(sequence, 2, "wormhole sequence 2");
	}

	/// A trailing-byte guard: the compat path must reject a layout that happens to
	/// consume the right number of records but not the whole blob.
	#[test]
	fn compat_rejects_layout_that_leaves_trailing_bytes() {
		let mut padded = MAINNET_EVENTS.to_vec();
		padded.push(0xff);
		assert!(
			decode_records::<BalancesEvent2506>(&padded).is_none(),
			"trailing bytes must invalidate the decode"
		);
	}
}
