// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Pure primitives for turning substrate token/trade events into synthetic
//! ethereum `Transaction`/`TransactionStatus`/`Receipt` records (one synth tx
//! per bucket: per-extrinsic, or per hook-phase + broadcast origin). The node
//! assembles and serves these off-chain over eth json-rpc by calling
//! `event_logs::synthetic_txs_from_records`; nothing here touches chain state.

#[cfg(test)]
mod tests;

use codec::{Decode, Encode, MaxEncodedLen};
use ethereum::EIP1559Transaction;
use ethereum_types::{Bloom, BloomInput, H160, H256, U256};
use frame_support::pallet_prelude::RuntimeDebug;
use pallet_broadcast::types::ExecutionType;
use pallet_ethereum::{Receipt, Transaction, TransactionAction, TransactionStatus};
use scale_info::TypeInfo;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;

/// `from`/`to` for synthetic txs. logs inside carry their own emitter address.
pub const SENTINEL_ADDRESS: H160 = H160([
	0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe,
	0xef,
]);

/// Constant fake `r`/`s`. Inside ECDSA range so the envelope decodes; never recovered.
pub const SYNTH_SIG_RS: H256 = H256([
	0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
	0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
]);

#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub enum HookPhase {
	Initialization,
	Finalization,
}

#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub enum Bucket {
	Extrinsic(u32),
	Hook {
		phase: HookPhase,
		origin: Option<ExecutionType>,
	},
}

/// Per-block domain-separation seed folded into each synth tx's `input`, so
/// envelope hashes are unique per block (frontier indexes txs by hash; without
/// this, `Extrinsic(2)` in block N and N+1 would hash identically).
fn block_domain(parent_hash: &[u8], block_number: u64) -> Vec<u8> {
	let mut seed = Vec::with_capacity(18 + parent_hash.len() + 8);
	seed.extend_from_slice(b"hydration-synth-v1");
	seed.extend_from_slice(parent_hash);
	seed.extend_from_slice(&block_number.to_le_bytes());
	seed
}

fn logs_bloom(logs: &[ethereum::Log]) -> Bloom {
	let mut bloom = Bloom::default();
	for log in logs {
		bloom.accrue(BloomInput::Raw(&log.address[..]));
		for topic in log.topics.iter() {
			bloom.accrue(BloomInput::Raw(&topic[..]));
		}
	}
	bloom
}

/// Assemble synthetic ethereum txs from bucketed logs. **Pure** — no storage,
/// no `T`.
///
/// One synth tx per bucket, emitted in ascending `bucket_sort_key` order
/// (init hooks < extrinsics by index < finalize hooks), insertion order
/// preserved within a bucket. `base_tx_index` is the count of real eth txs
/// already in the block; synth tx indices continue from there. Grouping is
/// O(N log G) via the BTreeMap (vs an O(N*G) linear scan).
pub fn assemble_synth_txs(
	entries: Vec<(Bucket, H160, ethereum::Log)>,
	chain_id: u64,
	parent_hash: &[u8],
	block_number: u64,
	base_tx_index: u32,
) -> Vec<(Transaction, TransactionStatus, Receipt)> {
	let mut groups: BTreeMap<(u8, u64), (Bucket, Vec<ethereum::Log>)> = BTreeMap::new();
	for (bucket, _emitter, log) in entries {
		groups
			.entry(bucket_sort_key(&bucket))
			.or_insert_with(|| (bucket, Vec::new()))
			.1
			.push(log);
	}

	let input = block_domain(parent_hash, block_number);
	let signature = ethereum::eip2930::TransactionSignature::new(false, SYNTH_SIG_RS, SYNTH_SIG_RS)
		.expect("synthetic signature constants are within valid ECDSA range; qed");

	let mut out = Vec::with_capacity(groups.len());
	for (group_index, (_key, (bucket, logs))) in groups.into_iter().enumerate() {
		let group_index = group_index as u32;
		// `value = group_index` keeps hashes distinct within a block; `input`
		// (block domain) keeps them distinct across blocks.
		let transaction = Transaction::EIP1559(EIP1559Transaction {
			chain_id,
			nonce: U256::from(bucket_nonce(bucket)),
			max_priority_fee_per_gas: U256::zero(),
			max_fee_per_gas: U256::zero(),
			gas_limit: U256::zero(),
			action: TransactionAction::Call(SENTINEL_ADDRESS),
			value: U256::from(group_index),
			input: input.clone(),
			access_list: Vec::new(),
			signature: signature.clone(),
		});
		let transaction_hash = transaction.hash();
		let bloom = logs_bloom(&logs);
		let status = TransactionStatus {
			transaction_hash,
			transaction_index: base_tx_index + group_index,
			from: SENTINEL_ADDRESS,
			to: Some(SENTINEL_ADDRESS),
			contract_address: None,
			logs: logs.clone(),
			logs_bloom: bloom,
		};
		let receipt = Receipt::EIP1559(ethereum::EIP658ReceiptData {
			status_code: 1,
			used_gas: U256::zero(),
			logs_bloom: bloom,
			logs,
		});
		out.push((transaction, status, receipt));
	}
	out
}

/// keccak256("Transfer(address,address,uint256)")
pub const TRANSFER_TOPIC: H256 = H256([
	0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa, 0x95, 0x2b, 0xa7,
	0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
]);

/// keccak256("Swap(address,uint256,uint256,uint256,uint256,address)") (uniswap v2)
pub const SWAP_TOPIC: H256 = H256([
	0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b, 0x65, 0x51, 0xd0, 0xda, 0x85, 0xfc, 0x27, 0x5f, 0xe6, 0x13, 0xce,
	0x37, 0x65, 0x7f, 0xb8, 0xd5, 0xe3, 0xd1, 0x30, 0x84, 0x01, 0x59, 0xd8, 0x22,
]);

/// keccak256("Approval(address,address,uint256)")
pub const APPROVAL_TOPIC: H256 = H256([
	0x8c, 0x5b, 0xe1, 0xe5, 0xeb, 0xec, 0x7d, 0x5b, 0xd1, 0x4f, 0x71, 0x42, 0x7d, 0x1e, 0x84, 0xf3, 0xdd, 0x03, 0x14,
	0xc0, 0xf7, 0xb2, 0x29, 0x1e, 0x5b, 0x20, 0x0a, 0xc8, 0xc7, 0xc3, 0xb9, 0x25,
]);

pub fn h160_to_h256(addr: H160) -> H256 {
	let mut bytes = [0u8; 32];
	bytes[12..].copy_from_slice(&addr.0);
	H256(bytes)
}

/// Pure mirror of `pallet_evm_accounts::evm_address`: an EVM-derived account is
/// `b"ETH\0" ++ <20-byte h160> ++ [0u8; 8]`; otherwise truncate to 20 bytes. No
/// state read — so it works client-side and against any runtime version.
pub fn account_to_evm_address(account: &[u8]) -> H160 {
	if account.len() >= 32 && &account[0..4] == b"ETH\0" && account[24..32] == [0u8; 8] {
		H160::from_slice(&account[4..24])
	} else {
		H160::from_slice(&account[..20])
	}
}

/// Pure mirror of `HydraErc20Mapping::encode_evm_address`: `0x..01 ++ asset_id`
/// (big-endian in the last 4 bytes). Correct for the registry assets that emit
/// orml/balances events; bound real-ERC20 assets transact as real contracts and
/// surface via real EVM logs, so they never reach this path.
pub fn asset_evm_address(asset_id: u32) -> H160 {
	let mut bytes = [0u8; 20];
	bytes[15] = 1;
	bytes[16..20].copy_from_slice(&asset_id.to_be_bytes());
	H160(bytes)
}

/// per-owner sentinel for reserved balance: `Transfer(owner, reserved_address_of(owner))`
/// on reserve, inverse on unreserve. derivation: xor first byte with `0xEE`
/// (reversible). collision with asset-prefix range `0x000…01<asset_id>` is `2^-128`.
pub fn reserved_address_of(owner: H160) -> H160 {
	let mut bytes = owner.0;
	bytes[0] ^= 0xEE;
	H160(bytes)
}

/// per-owner sentinel for frozen (locked) balance, kept DISTINCT from
/// [`reserved_address_of`] (xor `0xDD` vs `0xEE`) so locks and reserves never
/// alias. moving the frozen delta to this sentinel makes the owner's aggregated
/// transfer balance equal its *transferable* balance (free minus frozen): a lock
/// freezes free in place, so we mirror that as `Transfer(owner, frozen_address_of(owner))`.
pub fn frozen_address_of(owner: H160) -> H160 {
	let mut bytes = owner.0;
	bytes[0] ^= 0xDD;
	H160(bytes)
}

pub fn encode_u256_be(value: U256) -> [u8; 32] {
	value.to_big_endian()
}

/// 4 × u256 = 128 bytes — abi shape of uniswap v2 `Swap` non-indexed fields.
pub fn encode_uint256_quad(a: U256, b: U256, c: U256, d: U256) -> Vec<u8> {
	let mut data = Vec::with_capacity(128);
	data.extend_from_slice(&encode_u256_be(a));
	data.extend_from_slice(&encode_u256_be(b));
	data.extend_from_slice(&encode_u256_be(c));
	data.extend_from_slice(&encode_u256_be(d));
	data
}

// --- pure log-shape builders ------------------------------------------------
// Callers resolve addresses; these just encode the evm-log shape.

/// ERC-20 `Transfer(from, to, value)` log emitted from `token`'s address.
pub fn build_erc20_transfer_log(token: H160, from: H160, to: H160, amount: U256) -> ethereum::Log {
	ethereum::Log {
		address: token,
		topics: vec![TRANSFER_TOPIC, h160_to_h256(from), h160_to_h256(to)],
		data: encode_u256_be(amount).to_vec(),
	}
}

/// Uniswap-v2 `Swap(sender, a0In, a1In, a0Out, a1Out, to)` log from `pool`.
///
/// `input_is_token0` is whether the trade's input asset sorts as token0 of the
/// pair (token0 = the lower asset id); it selects which `amountN{In,Out}` slots
/// the in/out amounts land in.
pub fn build_uniswap_v2_swap_log(
	pool: H160,
	sender: H160,
	recipient: H160,
	input_is_token0: bool,
	in_amount: U256,
	out_amount: U256,
) -> ethereum::Log {
	let (a0_in, a1_in, a0_out, a1_out) = if input_is_token0 {
		(in_amount, U256::zero(), U256::zero(), out_amount)
	} else {
		(U256::zero(), in_amount, out_amount, U256::zero())
	};
	ethereum::Log {
		address: pool,
		topics: vec![SWAP_TOPIC, h160_to_h256(sender), h160_to_h256(recipient)],
		data: encode_uint256_quad(a0_in, a1_in, a0_out, a1_out),
	}
}

// 0=init hooks, 1=extrinsics (by index), 2=finalize hooks — preserves wall-clock order.
fn bucket_sort_key(bucket: &Bucket) -> (u8, u64) {
	match bucket {
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: None,
		} => (0, 0),
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: Some(_),
		} => (0, bucket_nonce(*bucket)),
		Bucket::Extrinsic(i) => (1, *i as u64),
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: None,
		} => (2, 0),
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: Some(_),
		} => (2, bucket_nonce(*bucket)),
	}
}

/// `nonce` field on the synth tx. lets indexers reverse a synth tx to its origin.
///
/// layout:
///   Extrinsic(i)                              → i           (low)
///   Hook { Init,     None }                   → MAX - 3
///   Hook { Init,     Some(t) }                → 0xDCA0…   | tag(t)
///   Hook { Final,    None }                   → MAX - 2
///   Hook { Final,    Some(t) }                → 0xF1A1…   | tag(t)
pub fn bucket_nonce(bucket: Bucket) -> u64 {
	match bucket {
		Bucket::Extrinsic(i) => i as u64,
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: None,
		} => u64::MAX - 3,
		Bucket::Hook {
			phase: HookPhase::Initialization,
			origin: Some(o),
		} => 0xDCA0_0000_0000_0000u64 | origin_tag(&o),
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: None,
		} => u64::MAX - 2,
		Bucket::Hook {
			phase: HookPhase::Finalization,
			origin: Some(o),
		} => 0xF1A1_0000_0000_0000u64 | origin_tag(&o),
	}
}

fn origin_tag(origin: &ExecutionType) -> u64 {
	match origin {
		ExecutionType::Router(id) => 0x0100_0000_0000 | (*id as u64),
		ExecutionType::DCA(schedule_id, _) => 0x0200_0000_0000 | (*schedule_id as u64),
		ExecutionType::Batch(id) => 0x0300_0000_0000 | (*id as u64),
		ExecutionType::Omnipool(id) => 0x0400_0000_0000 | (*id as u64),
		ExecutionType::XcmExchange(id) => 0x0500_0000_0000 | (*id as u64),
		ExecutionType::Xcm(_, id) => 0x0600_0000_0000 | (*id as u64),
	}
}
