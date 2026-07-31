// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! `StorageOverride` that augments Frontier's reads with synthetic ethereum txs,
//! produced client-side from a block's events via
//! `event_logs::synthetic_txs_from_records` (no runtime API, any runtime version).
//!
//! Real entries come first, synth appended in stable order, so a synth tx's index
//! resolves consistently across `current_transaction_statuses`/`current_receipts`
//! and `current_block.transactions` (fc-rpc indexes `block.transactions[index]`).
//! The block header is left canonical (hash unchanged); `eth_getLogs` discovery of
//! synth-only blocks is handled by the sibling `eth_filter` module.

use std::{
	marker::PhantomData,
	num::NonZeroUsize,
	sync::{Arc, Mutex, Once},
};

use super::compat_events;
use codec::Decode;
use fc_rpc::StorageOverride;
use fp_rpc::TransactionStatus;
use hydradx_runtime::evm::event_logs::synthetic_txs_from_records;
use lru::LruCache;
use pallet_ethereum::{Block as EthBlock, Receipt as EthReceipt, Transaction as EthTransaction};
use primitives::Block;
use sc_client_api::{backend::Backend, BlockBackend, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_core::{hashing::twox_128, H160, H256, U256};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Hash as HashT};
use sp_storage::StorageKey;

type Hash = <Block as BlockT>::Hash;
type SynthTxs = Vec<(EthTransaction, TransactionStatus, EthReceipt)>;

// node/runtime sdk skew is a persistent condition, so report it once instead of
// once per block.
static SKEW_WARNED: Once = Once::new();
static DECODE_FAILED: Once = Once::new();

// `synthetic()` is invoked once each by `current_block`/`current_receipts`/
// `current_transaction_statuses`, so a range `eth_getLogs` would re-read and
// re-translate a block's events 3× without this per-block cache.
const SYNTH_CACHE_CAP: usize = 256;

pub struct SyntheticStorageOverride<C, BE> {
	inner: Arc<dyn StorageOverride<Block>>,
	client: Arc<C>,
	cache: Mutex<LruCache<Hash, Arc<SynthTxs>>>,
	_marker: PhantomData<BE>,
}

impl<C, BE> SyntheticStorageOverride<C, BE> {
	pub fn new(inner: Arc<dyn StorageOverride<Block>>, client: Arc<C>) -> Self {
		Self {
			inner,
			client,
			cache: Mutex::new(LruCache::new(
				NonZeroUsize::new(SYNTH_CACHE_CAP).expect("non-zero; qed"),
			)),
			_marker: PhantomData,
		}
	}
}

fn storage_key(pallet: &[u8], item: &[u8]) -> StorageKey {
	StorageKey([twox_128(pallet), twox_128(item)].concat())
}

impl<C, BE> SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE> + HeaderBackend<Block> + BlockBackend<Block> + Send + Sync + 'static,
	BE: Backend<Block> + Send + Sync + 'static,
{
	fn read_decode<T: Decode>(&self, at: Hash, key: &StorageKey) -> Option<T> {
		let data = self.client.storage(at, key).ok().flatten()?;
		Decode::decode(&mut &data.0[..]).ok()
	}

	fn synthetic(&self, at: Hash) -> SynthTxs {
		if let Some(hit) = self.cache.lock().expect("synth cache mutex; qed").get(&at) {
			return (**hit).clone();
		}
		let txs = Arc::new(self.compute_synthetic(at));
		self.cache.lock().expect("synth cache mutex; qed").put(at, txs.clone());
		(*txs).clone()
	}

	fn compute_synthetic(&self, at: Hash) -> SynthTxs {
		let raw = match self.client.storage(at, &storage_key(b"System", b"Events")) {
			Ok(Some(data)) => data.0,
			_ => return Vec::new(),
		};
		let records = match compat_events::read_events(&raw) {
			Some((records, compat_events::Source::Native)) => records,
			Some((records, compat_events::Source::Compat)) => {
				SKEW_WARNED.call_once(|| {
					log::warn!(
						target: "synthetic-logs",
						"System::Events read through a compat balances layout: this node's polkadot-sdk \
						 differs from the on-chain runtime's. synth logs are being recovered, but deploy a \
						 node built against the live runtime's sdk."
					)
				});
				records
			}
			Some((records, compat_events::Source::Partial { skipped, trailing })) => {
				SKEW_WARNED.call_once(|| {
					log::error!(
						target: "synthetic-logs",
						"System::Events only partially decodable by this node — synth logs are \
						 INCOMPLETE (first affected block dropped {skipped} event(s), {trailing} \
						 trailing byte(s)). the node's RuntimeEvent does not match the on-chain \
						 runtime's; add its pallet_balances layout to compat_events.rs or deploy a \
						 node built against the live runtime's sdk."
					)
				});
				records
			}
			// never silent: an undecodable block otherwise looks event-free and drops every
			// synth log in it — that is how v49.2.0 hid wormhole LogMessagePublished.
			None => {
				DECODE_FAILED.call_once(|| {
					log::error!(
						target: "synthetic-logs",
						"cannot decode System::Events with any known layout — synthetic eth logs are \
						 INCOMPLETE. this node's RuntimeEvent does not match the on-chain runtime's; see \
						 node/src/synthetic_logs/compat_events.rs."
					)
				});
				log::debug!(target: "synthetic-logs", "System::Events undecodable at {at:?}");
				return Vec::new();
			}
		};
		if records.is_empty() {
			return Vec::new();
		}
		let chain_id: u64 = self
			.read_decode(at, &storage_key(b"EVMChainId", b"ChainId"))
			.unwrap_or_default();
		let real_statuses = self.inner.current_transaction_statuses(at).unwrap_or_default();
		// The extrinsic hashes go into each synth tx's `input` so an indexer can join it
		// back to its extrinsic. `Block::Extrinsic` is `OpaqueExtrinsic`, so this hashes
		// raw bytes and decodes no `RuntimeCall` — it cannot break on the sdk skew that
		// `compat_events` exists for. An unavailable body degrades to the zero hash,
		// which is still unique because `at` is folded in.
		let extrinsic_hashes: Vec<[u8; 32]> = self
			.client
			.block_body(at)
			.ok()
			.flatten()
			.map(|body| body.iter().map(|xt| BlakeTwo256::hash_of(xt).0).collect())
			.unwrap_or_default();

		// Only used to order the synth `nonce`; identity comes from `at` + extrinsic hash.
		let block_number: u64 = self.client.number(at).ok().flatten().map_or(0, u64::from);

		// `at` is the block's OWN hash: sibling blocks share a parent, so a parent-based
		// domain gave colliding synth tx hashes on every fork.
		synthetic_txs_from_records(
			&records,
			chain_id,
			at.as_ref(),
			&extrinsic_hashes,
			block_number,
			&real_statuses,
		)
	}
}

impl<C, BE> StorageOverride<Block> for SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE> + HeaderBackend<Block> + BlockBackend<Block> + Send + Sync + 'static,
	BE: Backend<Block> + Send + Sync + 'static,
{
	fn account_code_at(&self, at: Hash, address: H160) -> Option<Vec<u8>> {
		self.inner.account_code_at(at, address)
	}

	fn account_storage_at(&self, at: Hash, address: H160, index: U256) -> Option<H256> {
		self.inner.account_storage_at(at, address, index)
	}

	fn current_block(&self, at: Hash) -> Option<EthBlock> {
		let mut block = self.inner.current_block(at)?;
		// Append synth txs so `eth_getTransactionByHash`/`*_receipt` can index them
		// (fc-rpc does `block.transactions[index]`). The header is left UNTOUCHED so
		// the canonical eth block hash is preserved — surfacing synth logs in
		// `eth_getLogs` is handled by the `eth_filter` module, not by mutating the
		// header bloom (which would change the block hash).
		for (tx, _, _) in self.synthetic(at) {
			block.transactions.push(tx);
		}
		Some(block)
	}

	fn current_receipts(&self, at: Hash) -> Option<Vec<EthReceipt>> {
		let synth = self.synthetic(at);
		match self.inner.current_receipts(at) {
			Some(mut real) => {
				real.extend(synth.into_iter().map(|(_, _, r)| r));
				Some(real)
			}
			None if !synth.is_empty() => Some(synth.into_iter().map(|(_, _, r)| r).collect()),
			None => None,
		}
	}

	fn current_transaction_statuses(&self, at: Hash) -> Option<Vec<TransactionStatus>> {
		let synth = self.synthetic(at);
		match self.inner.current_transaction_statuses(at) {
			Some(mut real) => {
				real.extend(synth.into_iter().map(|(_, s, _)| s));
				Some(real)
			}
			None if !synth.is_empty() => Some(synth.into_iter().map(|(_, s, _)| s).collect()),
			None => None,
		}
	}

	fn elasticity(&self, at: Hash) -> Option<sp_runtime::Permill> {
		self.inner.elasticity(at)
	}

	fn is_eip1559(&self, at: Hash) -> bool {
		self.inner.is_eip1559(at)
	}
}
