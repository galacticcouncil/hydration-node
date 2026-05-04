// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! # Synthetic Logs Pallet
//!
//! Buffers ethereum-shaped logs that originate from substrate-side hooks
//! (orml-tokens transfers, pallet-broadcast trades, etc.) and flushes them on
//! `on_finalize` as synthetic `pallet_ethereum::Transaction` records. This
//! makes substrate-origin events visible to eth json-rpc (`eth_getLogs`,
//! `eth_getTransactionReceipt`) and to standard EVM tooling — ethers.js,
//! etherscan-style explorers, the graph, dex aggregators.
//!
//! ## Bucket model
//!
//! Each pushed log is tagged with a `Bucket` derived from the current
//! `frame_system::Phase` plus `pallet_broadcast::ExecutionContext` (when in
//! a hook phase). One synthetic transaction is built per bucket, so:
//! - one synth tx per substrate extrinsic (containing all logs from that extrinsic)
//! - one synth tx per `(on_initialize/on_finalize, originating broadcast context)`
//!   — e.g. each scheduled DCA execution gets its own tx attributed by `schedule_id`
//!
//! ## Pallet ordering
//!
//! Must be declared **before** `pallet_ethereum` in `construct_runtime!` so this
//! pallet's `on_finalize` runs first and writes into `pallet_ethereum::Pending`
//! before the ethereum pallet rolls Pending into the canonical block.

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

/// Sentinel address used as both `from` and (default) `to` for synthetic txs.
/// Indicates "non-real-tx origin"; logs inside still carry their own emitter
/// address (asset's erc20 contract for transfers, pool address for swaps).
pub const SENTINEL_ADDRESS: H160 = H160([
	0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe,
	0xef,
]);

/// Domain separator mixed into synthetic tx hashes so they can never collide
/// with real ethereum tx hashes computed from real signatures.
pub const SYNTH_DOMAIN: &[u8] = b"hydration-synth-v1";

/// Constant `r`/`s` value for synthetic signatures. Inside the valid ECDSA
/// range required by the ethereum crate, but clearly not a real signature.
pub const SYNTH_SIG_RS: H256 = H256([
	0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
	0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
]);

/// Hard cap on number of buffered logs per block. On overflow we drop the
/// newest pushes and emit a `log::warn!` so operators notice.
pub const MAX_PENDING_LOGS: u32 = 4096;

/// One synthetic-tx attribution bucket. All logs sharing a bucket end up in
/// the same synthetic transaction.
///
/// `Hook` covers `on_initialize` / `on_idle` / `on_finalize` work — substrate
/// doesn't expose a stable public api to distinguish initialize from finalize
/// at hook-fire time, so we collapse them and rely on the broadcast
/// `ExecutionContext` for finer attribution (e.g. DCA schedule_id, xcm origin).
#[derive(Clone, Copy, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug, PartialEq, Eq)]
pub enum Bucket {
	/// One synth tx per substrate extrinsic.
	Extrinsic(u32),
	/// One synth tx per (hook-phase + originating broadcast context).
	Hook { origin: Option<ExecutionType> },
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ethereum::Config + pallet_broadcast::Config {
		/// EVM chain id, mirrored from `pallet_evm::Config::ChainId`.
		type ChainId: Get<u64>;
	}

	/// Buffered `(bucket, emitter, log)` entries, drained on every `on_finalize`.
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
	/// Buffer a log to be emitted as part of a synthetic tx at end of block.
	/// Bucket is derived from current substrate phase and broadcast context.
	pub fn push(emitter: H160, log: ethereum::Log) {
		let bucket = Self::current_bucket();
		Pending::<T>::mutate(|v| {
			if v.len() as u32 >= MAX_PENDING_LOGS {
				log::warn!(
					target: "runtime::synthetic-logs",
					"pending buffer full ({} entries); dropping log for {:?}",
					v.len(), emitter,
				);
				return;
			}
			v.push((bucket, emitter, log));
		});
	}

	/// Determine the bucket for the currently-executing substrate phase.
	fn current_bucket() -> Bucket {
		match frame_system::Pallet::<T>::extrinsic_index() {
			Some(i) => Bucket::Extrinsic(i),
			None => Bucket::Hook {
				origin: pallet_broadcast::Pallet::<T>::get_context().first().copied(),
			},
		}
	}

	/// Drain buffer, group by bucket, and write one synthetic tx per bucket
	/// into `pallet_ethereum::Pending`. Must run before `pallet_ethereum::on_finalize`.
	fn flush(entries: Vec<(Bucket, H160, ethereum::Log)>) {
		let mut groups: Vec<(Bucket, Vec<ethereum::Log>)> = Vec::new();
		for (bucket, _emitter, log) in entries {
			match groups.iter_mut().find(|(b, _)| *b == bucket) {
				Some((_, logs)) => logs.push(log),
				None => groups.push((bucket, vec![log])),
			}
		}
		groups.sort_by(|a, b| Self::bucket_sort_key(&a.0).cmp(&Self::bucket_sort_key(&b.0)));

		let block_number = frame_system::Pallet::<T>::block_number();
		let parent_hash = frame_system::Pallet::<T>::parent_hash();

		for (idx, (bucket, logs)) in groups.into_iter().enumerate() {
			Self::insert_synth_tx(bucket, logs, idx as u32, &parent_hash, block_number);
		}
	}

	fn bucket_sort_key(bucket: &Bucket) -> (u8, u64) {
		match bucket {
			Bucket::Hook { origin: None } => (0, 0),
			Bucket::Hook { origin: Some(_) } => (0, Self::bucket_nonce(*bucket)),
			Bucket::Extrinsic(i) => (1, *i as u64),
		}
	}

	/// Encodes the bucket as the synthetic tx's `nonce`, so indexers can
	/// reverse the synth tx back to its substrate origin.
	pub fn bucket_nonce(bucket: Bucket) -> u64 {
		match bucket {
			Bucket::Extrinsic(i) => i as u64,
			Bucket::Hook { origin: None } => u64::MAX - 1,
			Bucket::Hook { origin: Some(o) } => 0xDCA0_0000_0000_0000u64 | Self::origin_tag(&o),
		}
	}

	fn origin_tag(origin: &ExecutionType) -> u64 {
		match origin {
			ExecutionType::Router(id) => 0x01_00_0000_0000 | (*id as u64),
			ExecutionType::DCA(schedule_id, _) => 0x02_00_0000_0000 | (*schedule_id as u64),
			ExecutionType::Batch(id) => 0x03_00_0000_0000 | (*id as u64),
			ExecutionType::Omnipool(id) => 0x04_00_0000_0000 | (*id as u64),
			ExecutionType::XcmExchange(id) => 0x05_00_0000_0000 | (*id as u64),
			ExecutionType::Xcm(_, id) => 0x06_00_0000_0000 | (*id as u64),
		}
	}

	fn insert_synth_tx(
		bucket: Bucket,
		logs: Vec<ethereum::Log>,
		group_index: u32,
		parent_hash: &T::Hash,
		block_number: BlockNumberFor<T>,
	) {
		let chain_id = <T as Config>::ChainId::get();
		let nonce = Self::bucket_nonce(bucket);

		// Build a deterministic synthetic tx hash:
		//   keccak256(SYNTH_DOMAIN || parent_hash || block_number || nonce || group_index)
		// `parent_hash` makes hashes unique across forks; nonce + group_index make them
		// unique within a block.
		let mut preimage: Vec<u8> = Vec::with_capacity(64);
		preimage.extend_from_slice(SYNTH_DOMAIN);
		preimage.extend_from_slice(parent_hash.as_ref());
		preimage.extend_from_slice(&block_number.encode());
		preimage.extend_from_slice(&nonce.to_be_bytes());
		preimage.extend_from_slice(&group_index.to_be_bytes());
		let transaction_hash = H256::from(sp_io::hashing::keccak_256(&preimage));

		// Synthetic transaction. EIP-1559 envelope; fees and gas are zero. The
		// signature is a constant fake (r=s=0x01..01) that satisfies the ECDSA
		// range check imposed by the ethereum crate but obviously corresponds
		// to no real signer. Synthetic txs are never recovered — `from` is
		// sourced from `TransactionStatus.from`.
		let signature = ethereum::eip2930::TransactionSignature::new(false, SYNTH_SIG_RS, SYNTH_SIG_RS)
			.expect("synthetic signature constants are within valid ECDSA range; qed");
		let transaction = Transaction::EIP1559(EIP1559Transaction {
			chain_id,
			nonce: U256::from(nonce),
			max_priority_fee_per_gas: U256::zero(),
			max_fee_per_gas: U256::zero(),
			gas_limit: U256::zero(),
			action: TransactionAction::Call(SENTINEL_ADDRESS),
			value: U256::zero(),
			input: Vec::new(),
			access_list: Vec::new(),
			signature,
		});

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

/// Helper: standard ERC-20 `Transfer(address,address,uint256)` topic0.
pub const TRANSFER_TOPIC: H256 = H256([
	0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc, 0x37, 0x8d, 0xaa, 0x95, 0x2b, 0xa7,
	0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a, 0x4d, 0xf5, 0x23, 0xb3, 0xef,
]);

/// Helper: Uniswap V2 `Swap(address,uint256,uint256,uint256,uint256,address)` topic0.
pub const SWAP_TOPIC: H256 = H256([
	0xd7, 0x8a, 0xd9, 0x5f, 0xa4, 0x6c, 0x99, 0x4b, 0x65, 0x51, 0xd0, 0xda, 0x85, 0xfc, 0x27, 0x5f, 0xe6, 0x13, 0xce,
	0x37, 0x65, 0x7f, 0xb8, 0xd5, 0xe3, 0xd1, 0x30, 0x84, 0x01, 0x59, 0xd8, 0x22,
]);

/// Helper: standard ERC-20 `Approval(address,address,uint256)` topic0.
pub const APPROVAL_TOPIC: H256 = H256([
	0x8c, 0x5b, 0xe1, 0xe5, 0xeb, 0xec, 0x7d, 0x5b, 0xd1, 0x4f, 0x71, 0x42, 0x7d, 0x1e, 0x84, 0xf3, 0xdd, 0x03, 0x14,
	0xc0, 0xf7, 0xb2, 0x29, 0x1e, 0x5b, 0x20, 0x0a, 0xc8, 0xc7, 0xc3, 0xb9, 0x25,
]);

/// Pad an `H160` to a 32-byte topic word (left-pad with 12 zero bytes).
pub fn h160_to_h256(addr: H160) -> H256 {
	let mut bytes = [0u8; 32];
	bytes[12..].copy_from_slice(&addr.0);
	H256(bytes)
}

/// Per-owner sentinel address representing the owner's reserved balance
/// bucket. Used as the `to` topic on `reserve` and the `from` topic on
/// `unreserve` so erc20 indexers reconstructing balances from `Transfer`
/// events stay consistent with `balanceOf` (which returns free balance only).
///
/// Derivation: XOR the first byte of `owner` with `0xEE`. The mapping is
/// trivially reversible (XOR again) so a bookkeeping consumer that wants
/// owner attribution from a sentinel can recover it; reversibility is
/// optional for forward-only indexers (which compute the sentinel from a
/// known owner). Collision with the asset-prefix range
/// (`0x000…01<asset_id>`) requires the owner to start with `0xEE` followed
/// by 11 zero bytes, then `0x01`, then 3 zero bytes — pre-image probability
/// `2^-128` for uniformly distributed addresses.
pub fn reserved_address_of(owner: H160) -> H160 {
	let mut bytes = owner.0;
	bytes[0] ^= 0xEE;
	H160(bytes)
}

/// Encode a single u256 as a 32-byte big-endian word for log `data`.
pub fn encode_u256_be(value: U256) -> [u8; 32] {
	value.to_big_endian()
}

/// Encode four u256 values as 128 bytes (the ABI shape of the
/// uniswap v2 Swap event's non-indexed fields).
pub fn encode_uint256_quad(a: U256, b: U256, c: U256, d: U256) -> Vec<u8> {
	let mut data = Vec::with_capacity(128);
	data.extend_from_slice(&encode_u256_be(a));
	data.extend_from_slice(&encode_u256_be(b));
	data.extend_from_slice(&encode_u256_be(c));
	data.extend_from_slice(&encode_u256_be(d));
	data
}
