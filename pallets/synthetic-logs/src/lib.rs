// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! buffers ethereum-shaped logs from substrate hooks; on_finalize flushes them
//! as synthetic `pallet_ethereum::Transaction` records so eth json-rpc surfaces
//! them. one synth tx per bucket: per-extrinsic, or per hook-phase + broadcast
//! origin (so each dca schedule etc. gets its own tx).
//!
//! must be declared before `pallet_ethereum` in `construct_runtime!`.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use codec::{Decode, Encode, MaxEncodedLen};
use ethereum::EIP1559Transaction;
use ethereum_types::{Bloom, BloomInput, H160, H256, U256};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_broadcast::types::ExecutionType;
use pallet_ethereum::{Receipt, Transaction, TransactionAction, TransactionStatus};
use scale_info::TypeInfo;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;

pub use pallet::*;

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

pub const MAX_PENDING_LOGS: u32 = 4096;

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

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ethereum::Config + pallet_broadcast::Config {
		type ChainId: Get<u64>;
	}

	#[pallet::storage]
	#[pallet::whitelist_storage]
	#[pallet::unbounded]
	pub type Pending<T: Config> = StorageValue<_, Vec<(Bucket, H160, ethereum::Log)>, ValueQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(n: BlockNumberFor<T>) {
			// Must run before `pallet_ethereum::on_finalize` builds this block's
			// eth block (declared earlier in `construct_runtime!`). After that
			// runs, `CurrentBlock` carries this block's number; seeing that here
			// means the ordering is broken and our synth txs would land in the
			// wrong block.
			debug_assert!(
				pallet_ethereum::CurrentBlock::<T>::get().map_or(true, |b| b.header.number
					< U256::from(UniqueSaturatedInto::<u128>::unique_saturated_into(n))),
				"pallet-synthetic-logs on_finalize must run before pallet_ethereum",
			);
			let drained = Pending::<T>::take();
			if drained.is_empty() {
				return;
			}
			Self::flush(drained);
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn push(emitter: H160, log: ethereum::Log) {
		// `decode_len` + `append` keep this O(1) per call: neither decodes the
		// whole buffer, so N pushes in a block cost O(N) rather than the O(N^2)
		// a `mutate(|v| v.push(..))` read-modify-write would incur (every native
		// balance move hits this hook).
		let len = Pending::<T>::decode_len().unwrap_or(0) as u32;
		if len >= MAX_PENDING_LOGS {
			log::warn!(
				target: "runtime::synthetic-logs",
				"pending buffer full ({len} entries); dropping log for {emitter:?}",
			);
			return;
		}
		let bucket = Self::current_bucket();
		Pending::<T>::append((bucket, emitter, log));
	}

	// reads `frame_system::ExecutionPhase` directly so on_idle (where substrate
	// has already set Phase=Finalization) buckets as Finalization too.
	fn current_bucket() -> Bucket {
		use frame_system::Phase;
		let phase_key = frame_support::storage::storage_prefix(b"System", b"ExecutionPhase");
		let phase: Phase = frame_support::storage::unhashed::get::<Phase>(&phase_key).unwrap_or_default();
		match phase {
			Phase::ApplyExtrinsic(i) => Bucket::Extrinsic(i),
			Phase::Finalization => Bucket::Hook {
				phase: HookPhase::Finalization,
				origin: pallet_broadcast::Pallet::<T>::get_context().first().copied(),
			},
			Phase::Initialization => Bucket::Hook {
				phase: HookPhase::Initialization,
				origin: pallet_broadcast::Pallet::<T>::get_context().first().copied(),
			},
		}
	}

	fn flush(entries: Vec<(Bucket, H160, ethereum::Log)>) {
		let chain_id = <T as Config>::ChainId::get();
		let parent_hash = frame_system::Pallet::<T>::parent_hash();
		let block_number = UniqueSaturatedInto::<u64>::unique_saturated_into(frame_system::Pallet::<T>::block_number());
		// synth txs continue after the block's real eth txs.
		let base_tx_index = pallet_ethereum::Pending::<T>::count();

		for (transaction, status, receipt) in
			assemble_synth_txs(entries, chain_id, parent_hash.as_ref(), block_number, base_tx_index)
		{
			pallet_ethereum::Pending::<T>::insert(status.transaction_index, (transaction, status, receipt));
		}
	}
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
/// no `T`. Shared by the on-chain flusher (`flush`) and the node-indexing
/// runtime API, so both yield byte-identical txs (the A/B parity guarantee).
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

/// per-owner sentinel for reserved balance: `Transfer(owner, reserved_address_of(owner))`
/// on reserve, inverse on unreserve. derivation: xor first byte with `0xEE`
/// (reversible). collision with asset-prefix range `0x000…01<asset_id>` is `2^-128`.
pub fn reserved_address_of(owner: H160) -> H160 {
	let mut bytes = owner.0;
	bytes[0] ^= 0xEE;
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
// The evm-log *shape* lives here, decoupled from how it's delivered (substrate
// mutation hook → on-chain synth tx, or node-side eth-rpc indexing). Callers
// resolve addresses; these just encode. Unit-tested for evm-client parity.

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
