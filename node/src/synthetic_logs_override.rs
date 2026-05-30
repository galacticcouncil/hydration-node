// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Node-side `StorageOverride` wrapper for the synthetic-logs node-indexing
//! variant.
//!
//! Wraps the stock Frontier `StorageOverride` and augments its reads with
//! synthetic ethereum txs — substrate `Transfer`/`Swapped3`/`pallet_evm` log
//! events translated to ERC-20 `Transfer` / uniswap-v2 `Swap` logs. The synth
//! txs are NOT in consensus state; they're produced **client-side** from the
//! block's events read out of state, using the pure
//! `event_logs::synthetic_txs_from_records`. Because this never invokes a
//! runtime API, it works against ANY runtime version — including blocks
//! produced before this runtime shipped (bounded only by whether their events
//! still decode).
//!
//! All three views stay index-aligned (real entries first, synth appended in a
//! stable order, `transaction_index` continuing from the real count) so a synth
//! tx's mapping-DB index resolves consistently across them:
//! - `current_transaction_statuses` / `current_receipts`: real, then synth.
//! - `current_block`: synth txs appended to `transactions` (so
//!   `eth_getTransactionByHash`/`*_receipt` can index them — fc-rpc does
//!   `block.transactions[index]`), and the synth-log blooms OR'd into
//!   `header.logs_bloom` so `filter_range_logs`' header-bloom prefilter doesn't
//!   skip synth-only blocks.

use std::{marker::PhantomData, sync::Arc};

use codec::Decode;
use fc_rpc::StorageOverride;
use fp_rpc::TransactionStatus;
use frame_system::EventRecord;
use hydradx_runtime::{evm::event_logs::synthetic_txs_from_records, RuntimeEvent};
use pallet_ethereum::{Block as EthBlock, Receipt as EthReceipt, Transaction as EthTransaction};
use primitives::Block;
use sc_client_api::{backend::Backend, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_core::{hashing::twox_128, H160, H256, U256};
use sp_runtime::traits::{Block as BlockT, Header as HeaderT, UniqueSaturatedInto};
use sp_storage::StorageKey;

type Hash = <Block as BlockT>::Hash;

pub struct SyntheticStorageOverride<C, BE> {
	inner: Arc<dyn StorageOverride<Block>>,
	client: Arc<C>,
	_marker: PhantomData<BE>,
}

impl<C, BE> SyntheticStorageOverride<C, BE> {
	pub fn new(inner: Arc<dyn StorageOverride<Block>>, client: Arc<C>) -> Self {
		Self {
			inner,
			client,
			_marker: PhantomData,
		}
	}
}

fn storage_key(pallet: &[u8], item: &[u8]) -> StorageKey {
	StorageKey([twox_128(pallet), twox_128(item)].concat())
}

impl<C, BE> SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE> + HeaderBackend<Block> + Send + Sync + 'static,
	BE: Backend<Block> + Send + Sync + 'static,
{
	fn read_decode<T: Decode>(&self, at: Hash, key: &StorageKey) -> Option<T> {
		let data = self.client.storage(at, key).ok().flatten()?;
		Decode::decode(&mut &data.0[..]).ok()
	}

	fn synthetic(&self, at: Hash) -> Vec<(EthTransaction, TransactionStatus, EthReceipt)> {
		let records: Vec<EventRecord<RuntimeEvent, H256>> =
			match self.read_decode(at, &storage_key(b"System", b"Events")) {
				Some(r) => r,
				None => return Vec::new(),
			};
		if records.is_empty() {
			return Vec::new();
		}
		let header = match self.client.header(at) {
			Ok(Some(h)) => h,
			_ => return Vec::new(),
		};
		let parent_hash = *header.parent_hash();
		let block_number: u64 = (*header.number()).unique_saturated_into();
		let chain_id: u64 = self
			.read_decode(at, &storage_key(b"EVMChainId", b"ChainId"))
			.unwrap_or_default();
		let real_statuses = self.inner.current_transaction_statuses(at).unwrap_or_default();

		synthetic_txs_from_records(&records, chain_id, parent_hash.as_ref(), block_number, &real_statuses)
	}
}

impl<C, BE> StorageOverride<Block> for SyntheticStorageOverride<C, BE>
where
	C: StorageProvider<Block, BE> + HeaderBackend<Block> + Send + Sync + 'static,
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
		for (tx, status, _) in self.synthetic(at) {
			for (h, s) in block.header.logs_bloom.0.iter_mut().zip(status.logs_bloom.0.iter()) {
				*h |= *s;
			}
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
