// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Node-side `StorageOverride` wrapper for the synthetic-logs node-indexing
//! variant.
//!
//! Wraps the stock Frontier `StorageOverride` and augments its reads with
//! synthetic ethereum txs — substrate `Transfer`/`Swapped3` events translated
//! to ERC-20 `Transfer` / uniswap-v2 `Swap` logs by `SyntheticEthLogsApi`. The
//! synth txs are NOT in consensus state; they are produced on demand from the
//! block's events, so `eth_getLogs` / `eth_getBlockReceipts` surface
//! substrate-origin activity with no on-chain synthetic-tx injection.
//!
//! - `current_transaction_statuses` / `current_receipts`: real entries first,
//!   then synth (whose `transaction_index` continues from the real count, set
//!   by the runtime API), so positions line up across both.
//! - `current_block`: real transactions are left untouched (the tx root stays
//!   consistent); we only OR the synth-log blooms into `header.logs_bloom` so
//!   `filter_range_logs`' header-bloom prefilter doesn't skip blocks whose only
//!   matching logs are synthetic. Over-inclusive blooms are safe — the per-log
//!   scan re-checks. Synth txs surface via `eth_getLogs` and (with the hash
//!   index) `eth_getTransactionReceipt`, not the block's tx list.

use std::sync::Arc;

use fc_rpc::StorageOverride;
use fp_rpc::TransactionStatus;
use hydradx_runtime::evm::event_logs::SyntheticEthLogsApi;
use pallet_ethereum::{Block as EthBlock, Receipt as EthReceipt, Transaction as EthTransaction};
use primitives::Block;
use sp_api::ProvideRuntimeApi;
use sp_core::{H160, H256, U256};
use sp_runtime::{traits::Block as BlockT, Permill};

type Hash = <Block as BlockT>::Hash;

pub struct SyntheticStorageOverride<C> {
	inner: Arc<dyn StorageOverride<Block>>,
	client: Arc<C>,
}

impl<C> SyntheticStorageOverride<C> {
	pub fn new(inner: Arc<dyn StorageOverride<Block>>, client: Arc<C>) -> Self {
		Self { inner, client }
	}
}

impl<C> SyntheticStorageOverride<C>
where
	C: ProvideRuntimeApi<Block> + Send + Sync,
	C::Api: SyntheticEthLogsApi<Block>,
{
	fn synthetic(&self, at: Hash) -> Vec<(EthTransaction, TransactionStatus, EthReceipt)> {
		self.client.runtime_api().synthetic_transactions(at).unwrap_or_default()
	}
}

impl<C> StorageOverride<Block> for SyntheticStorageOverride<C>
where
	C: ProvideRuntimeApi<Block> + Send + Sync,
	C::Api: SyntheticEthLogsApi<Block>,
{
	fn account_code_at(&self, at: Hash, address: H160) -> Option<Vec<u8>> {
		self.inner.account_code_at(at, address)
	}

	fn account_storage_at(&self, at: Hash, address: H160, index: U256) -> Option<H256> {
		self.inner.account_storage_at(at, address, index)
	}

	fn current_block(&self, at: Hash) -> Option<EthBlock> {
		let mut block = self.inner.current_block(at)?;
		for (_, status, _) in self.synthetic(at) {
			for (h, s) in block.header.logs_bloom.0.iter_mut().zip(status.logs_bloom.0.iter()) {
				*h |= *s;
			}
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

	fn elasticity(&self, at: Hash) -> Option<Permill> {
		self.inner.elasticity(at)
	}

	fn is_eip1559(&self, at: Hash) -> bool {
		self.inner.is_eip1559(at)
	}
}
