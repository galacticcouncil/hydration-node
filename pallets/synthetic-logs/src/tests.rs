// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for the pure helpers exported by `pallet-synthetic-logs`.
//!
//! End-to-end tests (push -> on_finalize -> pallet_ethereum::Pending writes)
//! live in `integration-tests` because they require a runtime that wires
//! pallet_ethereum, pallet_evm, frame_system, and pallet_broadcast together.

use super::*;
use ethereum_types::{H160, U256};

#[test]
fn h160_to_h256_left_pads_with_zeros() {
	let addr = H160([
		0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa,
		0xbb, 0xcc,
	]);
	let topic = h160_to_h256(addr);
	// First 12 bytes zero, last 20 bytes are the address.
	assert_eq!(&topic.0[..12], &[0u8; 12]);
	assert_eq!(&topic.0[12..], &addr.0[..]);
}

#[test]
fn encode_u256_be_round_trip() {
	let value = U256::from(123_456_789u64);
	let encoded = encode_u256_be(value);
	assert_eq!(encoded.len(), 32);
	assert_eq!(U256::from_big_endian(&encoded), value);
}

#[test]
fn encode_uint256_quad_is_128_bytes() {
	let data = encode_uint256_quad(U256::from(1u64), U256::from(2u64), U256::from(3u64), U256::from(4u64));
	assert_eq!(data.len(), 128);
	assert_eq!(U256::from_big_endian(&data[0..32]), U256::from(1u64));
	assert_eq!(U256::from_big_endian(&data[32..64]), U256::from(2u64));
	assert_eq!(U256::from_big_endian(&data[64..96]), U256::from(3u64));
	assert_eq!(U256::from_big_endian(&data[96..128]), U256::from(4u64));
}

#[test]
fn synth_signature_is_in_valid_range() {
	// Confirms our constant signature passes the ECDSA range check; the
	// flusher panics with a message if this regresses.
	let sig = ethereum::eip2930::TransactionSignature::new(false, SYNTH_SIG_RS, SYNTH_SIG_RS);
	assert!(sig.is_some(), "synthetic signature constants must satisfy ECDSA range");
}

#[test]
fn known_topic_constants_match_expected_keccak256() {
	// ERC-20 Transfer(address,address,uint256)
	let expected_transfer = [
		0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa, 0x95, 0x2b,
		0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
	];
	assert_eq!(TRANSFER_TOPIC.0, expected_transfer);

	// Uniswap V2 Swap(address,uint256,uint256,uint256,uint256,address)
	let expected_swap = [
		0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b, 0x65, 0x51, 0xd0, 0xda, 0x85, 0xfc, 0x27, 0x5f, 0xe6, 0x13,
		0xce, 0x37, 0x65, 0x7f, 0xb8, 0xd5, 0xe3, 0xd1, 0x30, 0x84, 0x01, 0x59, 0xd8, 0x22,
	];
	assert_eq!(SWAP_TOPIC.0, expected_swap);

	// ERC-20 Approval(address,address,uint256)
	let expected_approval = [
		0x8c, 0x5b, 0xe1, 0xe5, 0xeb, 0xec, 0x7d, 0x5b, 0xd1, 0x4f, 0x71, 0x42, 0x7d, 0x1e, 0x84, 0xf3, 0xdd, 0x03,
		0x14, 0xc0, 0xf7, 0xb2, 0x29, 0x1e, 0x5b, 0x20, 0x0a, 0xc8, 0xc7, 0xc3, 0xb9, 0x25,
	];
	assert_eq!(APPROVAL_TOPIC.0, expected_approval);
}

#[test]
fn reserved_address_of_is_reversible() {
	let owner = H160([
		0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa,
		0xbb, 0xcc,
	]);
	let sentinel = reserved_address_of(owner);
	assert_ne!(owner, sentinel);
	assert_eq!(reserved_address_of(sentinel), owner, "xor with 0xEE is its own inverse");
}

// Two synth txs sharing the same bucket-nonce class but different group_index
// must have distinct canonical envelope hashes — frontier indexes by hash, so
// a collision would mean one synth tx shadows the other in eth_getTransactionByHash.
#[test]
fn synth_envelope_hash_is_unique_per_group_index() {
	use crate::Transaction;
	use ethereum::{eip2930::TransactionSignature, EIP1559Transaction, TransactionAction};

	let signature = TransactionSignature::new(false, SYNTH_SIG_RS, SYNTH_SIG_RS).expect("synth sig in range");
	let mk = |group_index: u32, nonce: u64| {
		Transaction::EIP1559(EIP1559Transaction {
			chain_id: 222_222,
			nonce: U256::from(nonce),
			max_priority_fee_per_gas: U256::zero(),
			max_fee_per_gas: U256::zero(),
			gas_limit: U256::zero(),
			action: TransactionAction::Call(SENTINEL_ADDRESS),
			value: U256::from(group_index),
			input: Vec::new(),
			access_list: Vec::new(),
			signature: signature.clone(),
		})
	};

	// same nonce (same bucket-nonce class) but different group_index → distinct hashes
	let nonce = u64::MAX - 3; // Hook { Init, None }
	assert_ne!(mk(0, nonce).hash(), mk(1, nonce).hash());
	assert_ne!(mk(0, nonce).hash(), mk(2, nonce).hash());
	assert_ne!(mk(1, nonce).hash(), mk(2, nonce).hash());

	// determinism: same (group_index, nonce) → same hash
	assert_eq!(mk(7, 42).hash(), mk(7, 42).hash());
}

// Bucket grouping must (a) collapse repeated (bucket, log) entries from the
// same bucket into one group, and (b) sort init < extrinsic < finalize.
#[test]
fn flush_bucket_grouping_and_sort_order() {
	use pallet_broadcast::types::ExecutionType;

	// Bare grouping helper that mirrors `flush`'s grouping step (without
	// driving the full pallet runtime). We test the visible invariants:
	// 1. preserves insertion order within a bucket
	// 2. produces one group per distinct bucket
	let entries: Vec<(Bucket, ethereum::Log)> = vec![
		(Bucket::Extrinsic(2), log(1)),
		(
			Bucket::Hook {
				phase: HookPhase::Initialization,
				origin: None,
			},
			log(2),
		),
		(Bucket::Extrinsic(2), log(3)),
		(
			Bucket::Hook {
				phase: HookPhase::Finalization,
				origin: None,
			},
			log(4),
		),
		(Bucket::Extrinsic(0), log(5)),
		(
			Bucket::Hook {
				phase: HookPhase::Initialization,
				origin: Some(ExecutionType::DCA(7, 1)),
			},
			log(6),
		),
	];

	let mut groups: Vec<(Bucket, Vec<ethereum::Log>)> = Vec::new();
	for (bucket, log) in entries {
		match groups.iter_mut().find(|(b, _)| *b == bucket) {
			Some((_, logs)) => logs.push(log),
			None => groups.push((bucket, vec![log])),
		}
	}
	groups.sort_by(|a, b| bucket_sort_key(&a.0).cmp(&bucket_sort_key(&b.0)));

	// 5 distinct buckets, in order: Init/None < Init/DCA < Extrinsic(0) < Extrinsic(2) < Final/None
	let order: Vec<Bucket> = groups.iter().map(|(b, _)| *b).collect();
	assert!(matches!(
		order[0],
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: None
		}
	));
	assert!(matches!(
		order[1],
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: Some(ExecutionType::DCA(_, _))
		}
	));
	assert!(matches!(order[2], Bucket::Extrinsic(0)));
	assert!(matches!(order[3], Bucket::Extrinsic(2)));
	assert!(matches!(
		order[4],
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: None
		}
	));

	// Extrinsic(2) holds both its logs in insertion order.
	let ext2_logs = &groups
		.iter()
		.find(|(b, _)| matches!(b, Bucket::Extrinsic(2)))
		.unwrap()
		.1;
	assert_eq!(ext2_logs.len(), 2);
	assert_eq!(ext2_logs[0].address.0[19], 1);
	assert_eq!(ext2_logs[1].address.0[19], 3);
}

#[test]
fn bucket_nonce_layout_is_distinct_per_class() {
	use pallet_broadcast::types::ExecutionType;
	let cases = [
		Bucket::Extrinsic(0),
		Bucket::Extrinsic(7),
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: None,
		},
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: Some(ExecutionType::DCA(123, 1)),
		},
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: None,
		},
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: Some(ExecutionType::Router(99)),
		},
	];
	let nonces: Vec<u64> = cases.iter().map(|b| bucket_nonce(*b)).collect();
	let unique: std::collections::BTreeSet<_> = nonces.iter().collect();
	assert_eq!(
		unique.len(),
		cases.len(),
		"every bucket class must produce a distinct nonce"
	);
}

fn log(tag: u8) -> ethereum::Log {
	let mut addr = [0u8; 20];
	addr[19] = tag;
	ethereum::Log {
		address: H160(addr),
		topics: vec![TRANSFER_TOPIC],
		data: Vec::new(),
	}
}
