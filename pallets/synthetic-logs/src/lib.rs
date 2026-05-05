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
	Hook { phase: HookPhase, origin: Option<ExecutionType> },
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
		fn on_finalize(_n: BlockNumberFor<T>) {
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
		let bucket = Self::current_bucket();
		Pending::<T>::mutate(|v| {
			if v.len() as u32 >= MAX_PENDING_LOGS {
				let n = v.len();
				log::warn!(
					target: "runtime::synthetic-logs",
					"pending buffer full ({n} entries); dropping log for {emitter:?}",
				);
				return;
			}
			v.push((bucket, emitter, log));
		});
	}

	// reads `frame_system::ExecutionPhase` directly so on_idle (where substrate
	// has already set Phase=Finalization) buckets as Finalization too.
	fn current_bucket() -> Bucket {
		use frame_system::Phase;
		let phase_key =
			frame_support::storage::storage_prefix(b"System", b"ExecutionPhase");
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
		let mut groups: Vec<(Bucket, Vec<ethereum::Log>)> = Vec::new();
		for (bucket, _emitter, log) in entries {
			match groups.iter_mut().find(|(b, _)| *b == bucket) {
				Some((_, logs)) => logs.push(log),
				None => groups.push((bucket, vec![log])),
			}
		}

		groups.sort_by(|a, b| bucket_sort_key(&a.0).cmp(&bucket_sort_key(&b.0)));

		for (idx, (bucket, logs)) in groups.into_iter().enumerate() {
			Self::insert_synth_tx(bucket, logs, idx as u32);
		}
	}

	fn insert_synth_tx(bucket: Bucket, logs: Vec<ethereum::Log>, group_index: u32) {
		let chain_id = <T as Config>::ChainId::get();
		let nonce = bucket_nonce(bucket);

		let signature = ethereum::eip2930::TransactionSignature::new(false, SYNTH_SIG_RS, SYNTH_SIG_RS)
			.expect("synthetic signature constants are within valid ECDSA range; qed");
		// `value = group_index` so two synth txs sharing the same bucket-nonce in
		// one block produce distinct envelope hashes (frontier indexes by hash).
		let transaction = Transaction::EIP1559(EIP1559Transaction {
			chain_id,
			nonce: U256::from(nonce),
			max_priority_fee_per_gas: U256::zero(),
			max_fee_per_gas: U256::zero(),
			gas_limit: U256::zero(),
			action: TransactionAction::Call(SENTINEL_ADDRESS),
			value: U256::from(group_index),
			input: Vec::new(),
			access_list: Vec::new(),
			signature,
		});

		// canonical envelope hash so frontier's tx-index resolves it.
		let transaction_hash = transaction.hash();

		let mut bloom: Bloom = Bloom::default();
		Self::compute_logs_bloom(&logs, &mut bloom);

		let tx_index = pallet_ethereum::Pending::<T>::count();

		let status = TransactionStatus {
			transaction_hash,
			transaction_index: tx_index,
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

		pallet_ethereum::Pending::<T>::insert(tx_index, (transaction, status, receipt));
	}

	fn compute_logs_bloom(logs: &[ethereum::Log], bloom: &mut Bloom) {
		for log in logs {
			bloom.accrue(BloomInput::Raw(&log.address[..]));
			for topic in log.topics.iter() {
				bloom.accrue(BloomInput::Raw(&topic[..]));
			}
		}
	}
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
