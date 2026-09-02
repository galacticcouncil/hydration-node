// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Reorg stability of the synth tx hash, end to end.
//!
//! The property under test is the one an offchain indexer depends on: a transaction keeps
//! its hash when a reorg re-includes it somewhere else. The runtime crate proves this for
//! `assemble_synth_txs` in isolation; here it is driven through the whole translation —
//! `synthetic_txs_from_records`, which derives the buckets, pulls each extrinsic's origin out
//! of `TransactionPayment.TransactionFeePaid`, and assembles the envelope — over a real
//! mainnet events blob.
//!
//! These tests live in the node crate for two reasons: the event fixtures are here, and the
//! runtime crate's lib-test target does not compile on this branch (`runtime/hydradx/src/
//! tests.rs` calls `Runtime::query_xcm_weight`, removed by the `XcmPaymentApi` v1→v2 change),
//! so a test placed there could not be run.

use super::metadata_events::{EventLayout, TEST_CHAIN_METADATA};
use frame_system::{EventRecord, Phase};
use hydradx_runtime::evm::event_logs::synthetic_txs_from_records;
use hydradx_runtime::evm::synthetic_logs::{KIND_EXTRINSIC, SYNTH_SELECTOR, SYNTH_V2_FROM, SYNTH_V2_SELECTOR};
use hydradx_runtime::RuntimeEvent;
use pallet_ethereum::Transaction;
use sp_core::{H256, U256};
use std::collections::BTreeMap;

type Record = EventRecord<RuntimeEvent, H256>;

/// `System::Events` from mainnet block 13386492: 84 records over `Timestamp.set`,
/// `ParachainSystem`, an `Omnipool.sell` and a `Utility.batch_all([EVM.call, EVM.call])`.
const MAINNET_EVENTS: &[u8] = include_bytes!("test_data/events_13386492.scale");

/// Decoded against the chain's own metadata, so this works whatever sdk the node was built
/// against.
fn mainnet_records() -> Vec<Record> {
	let layout = EventLayout::new(TEST_CHAIN_METADATA).expect("fixture metadata builds a layout");
	let decoded = layout.decode(MAINNET_EVENTS);
	assert!(!decoded.records.is_empty(), "fixture must decode");
	decoded.records
}

/// Highest `ApplyExtrinsic` index in the blob, so the extrinsic-hash table can be sized.
fn extrinsic_count(records: &[Record]) -> usize {
	records
		.iter()
		.filter_map(|r| match r.phase {
			Phase::ApplyExtrinsic(i) => Some(i as usize + 1),
			_ => None,
		})
		.max()
		.unwrap_or(0)
}

/// The competing block: one extra extrinsic landed ahead of everything else, so every
/// `ApplyExtrinsic(i)` becomes `ApplyExtrinsic(i + 1)`. This is what actually moves under a
/// reorg and what v1's identity was (wrongly) sensitive to — it shifts both the extrinsic
/// index folded into v1's `bucket`, and the `group_index` behind v1's `nonce`.
fn shift_extrinsics_by_one(records: &[Record]) -> Vec<Record> {
	records
		.iter()
		.cloned()
		.map(|mut r| {
			if let Phase::ApplyExtrinsic(i) = r.phase {
				r.phase = Phase::ApplyExtrinsic(i + 1);
			}
			r
		})
		.collect()
}

/// `assemble_synth_txs` always builds EIP-1559 envelopes, so anything else here means the
/// envelope changed shape and these tests are reading the wrong fields.
fn eip1559(tx: &Transaction) -> &ethereum::EIP1559Transaction {
	match tx {
		Transaction::EIP1559(t) => t,
		_ => panic!("synth txs are always EIP1559"),
	}
}

fn input_of(tx: &Transaction) -> Vec<u8> {
	eip1559(tx).input.clone()
}

fn nonce_of(tx: &Transaction) -> U256 {
	eip1559(tx).nonce
}

/// `extrinsic hash -> synth tx hash`, for the extrinsic-anchored txs only.
///
/// v2 puts the extrinsic hash in the anchor word (`input[4..36]`) and marks the kind;
/// v1 puts it in the second word (`input[36..68]`), behind the block hash.
fn by_extrinsic(
	txs: &[(Transaction, fp_rpc::TransactionStatus, pallet_ethereum::Receipt)],
	v2: bool,
) -> BTreeMap<[u8; 32], H256> {
	let mut out = BTreeMap::new();
	for (tx, status, _) in txs {
		let input = input_of(tx);
		if input.len() < 4 + 32 * 3 {
			continue;
		}
		let selector = &input[..4];
		let mut xt = [0u8; 32];
		if v2 {
			if selector != SYNTH_V2_SELECTOR || input[67] != KIND_EXTRINSIC {
				continue;
			}
			xt.copy_from_slice(&input[4..36]);
		} else {
			if selector != SYNTH_SELECTOR {
				continue;
			}
			xt.copy_from_slice(&input[36..68]);
		}
		// hook buckets carry a zero extrinsic hash in v1; skip them here
		if xt != [0u8; 32] {
			out.insert(xt, status.transaction_hash);
		}
	}
	out
}

/// Build the same activity into two different blocks: different block hash, different
/// height, and every extrinsic shifted one position along.
fn two_blocks(
	height_a: u64,
	height_b: u64,
) -> (
	Vec<(Transaction, fp_rpc::TransactionStatus, pallet_ethereum::Receipt)>,
	Vec<(Transaction, fp_rpc::TransactionStatus, pallet_ethereum::Receipt)>,
) {
	let records = mainnet_records();
	let count = extrinsic_count(&records);
	assert!(count > 1, "fixture must span several extrinsics");

	// distinct, deterministic extrinsic hashes
	let xt_a: Vec<[u8; 32]> = (0..count).map(|i| [(i as u8) + 1; 32]).collect();
	let a = synthetic_txs_from_records(&records, 222_222, &[0x11u8; 32], &xt_a, height_a, &[]);

	let shifted = shift_extrinsics_by_one(&records);
	let mut xt_b = vec![[0xEEu8; 32]];
	xt_b.extend(xt_a.iter().copied());
	let b = synthetic_txs_from_records(&shifted, 222_222, &[0x99u8; 32], &xt_b, height_b, &[]);

	(a, b)
}

/// The property offchain indexing depends on: re-inclusion in a different block, at a
/// different height, at a different position, must not change a transaction's hash.
#[test]
fn v2_extrinsic_keeps_its_hash_when_reincluded_elsewhere() {
	let (a, b) = two_blocks(SYNTH_V2_FROM, SYNTH_V2_FROM + 13);
	let (ma, mb) = (by_extrinsic(&a, true), by_extrinsic(&b, true));

	assert!(!ma.is_empty(), "no extrinsic-anchored synth txs found in block A");
	let shared: Vec<_> = ma.keys().filter(|k| mb.contains_key(*k)).collect();
	assert!(
		!shared.is_empty(),
		"the same extrinsics must produce synth txs in both blocks"
	);
	for xt in shared {
		assert_eq!(
			ma[xt],
			mb[xt],
			"extrinsic {:?} changed synth tx hash across re-inclusion",
			H256(*xt)
		);
	}
	// and nothing block-scoped leaked into the nonce
	for (tx, _, _) in a.iter().chain(b.iter()) {
		assert!(nonce_of(tx).is_zero(), "v2 nonce must be 0");
	}
}

/// The defect itself, kept under test so the fix cannot be quietly undone: under v1 the same
/// extrinsic in a different block gets a different hash, because the block hash and the
/// position are both folded into the identity.
#[test]
fn v1_extrinsic_hash_churns_on_reinclusion() {
	let (a, b) = two_blocks(SYNTH_V2_FROM - 1, SYNTH_V2_FROM - 1);
	let (ma, mb) = (by_extrinsic(&a, false), by_extrinsic(&b, false));

	assert!(!ma.is_empty(), "no extrinsic-anchored synth txs found under v1");
	let shared: Vec<_> = ma.keys().filter(|k| mb.contains_key(*k)).collect();
	assert!(!shared.is_empty(), "same extrinsics expected in both blocks");
	assert!(
		shared.iter().all(|xt| ma[*xt] != mb[*xt]),
		"v1 is expected to churn; if this passes, v1 changed and the golden vectors are stale"
	);
}

/// The gate: one height either side of activation picks a different envelope, and the
/// selector in `input` is what tells a consumer which one it is looking at.
#[test]
fn activation_height_switches_the_envelope() {
	let records = mainnet_records();
	let count = extrinsic_count(&records);
	let xt: Vec<[u8; 32]> = (0..count).map(|i| [(i as u8) + 1; 32]).collect();

	for (height, expected) in [(SYNTH_V2_FROM - 1, SYNTH_SELECTOR), (SYNTH_V2_FROM, SYNTH_V2_SELECTOR)] {
		let txs = synthetic_txs_from_records(&records, 222_222, &[0x11u8; 32], &xt, height, &[]);
		assert!(!txs.is_empty(), "fixture must yield synth txs at height {height}");
		for (tx, _, _) in &txs {
			let input = input_of(tx);
			assert_eq!(&input[..4], &expected, "wrong envelope selector at height {height}");
			assert_eq!(input.len(), 4 + 32 * 3, "both envelopes are 3 abi words");
		}
	}
}
