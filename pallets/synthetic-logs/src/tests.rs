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
}
