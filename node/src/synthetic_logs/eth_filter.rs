// This file is part of hydration-node.
//
// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

//! Custom `eth_getLogs` for the synthetic-logs node-indexing variant.
//!
//! fc-rpc's filter both prefilters on, and derives `log.blockHash` from,
//! `current_block().header` — so OR-ing the synth bloom into that header (to keep
//! the prefilter from skipping synth-only blocks) would change the canonical eth
//! block hash and break `eth_getBlockByHash`/receipt round-trips. Instead the
//! override keeps the header canonical, and this filter builds the prefilter bloom
//! from the block's real+synth tx statuses, so synth blocks are matched while
//! `log.blockHash` stays canonical.

use std::{
	sync::Arc,
	time::{Duration, Instant},
};

use ethereum::BlockV3 as EthereumBlock;
use fc_db::kv::Backend as FrontierBackend;
use fc_rpc::{frontier_backend_client, internal_err, EthBlockDataCacheTask};
use fc_rpc_core::types::{Bytes, Filter, FilteredParams, Log};
use fp_rpc::TransactionStatus;
use jsonrpsee::core::RpcResult;
use primitives::Block;
use sp_blockchain::HeaderBackend;
use sp_core::{hashing::keccak_256, H256, U256};
use sp_runtime::traits::{NumberFor, One, UniqueSaturatedInto};

const MAX_DURATION: Duration = Duration::from_secs(10);

/// `eth_getLogs` over the synth-aware override, preserving canonical block hashes.
pub async fn logs<C>(
	client: Arc<C>,
	backend: Arc<FrontierBackend<Block, C>>,
	block_data_cache: Arc<EthBlockDataCacheTask<Block>>,
	max_past_logs: u32,
	filter: Filter,
) -> RpcResult<Vec<Log>>
where
	C: HeaderBackend<Block> + Send + Sync + 'static,
{
	if let Some(hash) = filter.block_hash {
		let substrate_hash =
			match frontier_backend_client::load_hash::<Block, C>(client.as_ref(), backend.as_ref(), hash).await? {
				Some(h) => h,
				None => return Err(fc_rpc::err(-32000, "unknown block", None)),
			};
		let block = block_data_cache.current_block(substrate_hash).await;
		let statuses = block_data_cache.current_transaction_statuses(substrate_hash).await;
		return Ok(match (block, statuses) {
			(Some(block), Some(statuses)) => filter_block_logs(&filter, block, statuses),
			_ => Vec::new(),
		});
	}

	let best = client.info().best_number;
	let to = filter
		.to_block
		.and_then(|v| v.to_min_block_num())
		.map(|s| s.unique_saturated_into())
		.unwrap_or(best)
		.min(best);
	let from = filter
		.from_block
		.and_then(|v| v.to_min_block_num())
		.map(|s| s.unique_saturated_into())
		.unwrap_or(best);

	range_logs(client.as_ref(), &block_data_cache, max_past_logs, &filter, from, to).await
}

async fn range_logs<C>(
	client: &C,
	block_data_cache: &EthBlockDataCacheTask<Block>,
	max_past_logs: u32,
	filter: &Filter,
	from: NumberFor<Block>,
	to: NumberFor<Block>,
) -> RpcResult<Vec<Log>>
where
	C: HeaderBackend<Block>,
{
	let mut ret = Vec::new();
	if from > to {
		return Ok(ret);
	}
	let address_bloom = FilteredParams::address_bloom_filter(&filter.address);
	let topics_bloom = FilteredParams::topics_bloom_filter(&filter.topics());
	let begin = Instant::now();
	let mut current = from;

	loop {
		if let Some(substrate_hash) = client.hash(current).map_err(|e| internal_err(format!("{e:?}")))? {
			let block = block_data_cache.current_block(substrate_hash).await;
			let statuses = block_data_cache.current_transaction_statuses(substrate_hash).await;
			if let (Some(block), Some(statuses)) = (block, statuses) {
				// Synth-aware prefilter: the canonical header bloom only covers real
				// txs, so OR in each (real + synth) tx status's bloom.
				let mut bloom = block.header.logs_bloom;
				for status in &statuses {
					for (b, s) in bloom.0.iter_mut().zip(status.logs_bloom.0.iter()) {
						*b |= *s;
					}
				}
				if FilteredParams::address_in_bloom(bloom, &address_bloom)
					&& FilteredParams::topics_in_bloom(bloom, &topics_bloom)
				{
					ret.extend(filter_block_logs(filter, block, statuses));
				}
			}
		}

		if ret.len() as u32 > max_past_logs {
			return Err(internal_err(format!(
				"query returned more than {max_past_logs} results"
			)));
		}
		if begin.elapsed() > MAX_DURATION {
			return Err(internal_err(format!(
				"query timeout of {} seconds exceeded",
				MAX_DURATION.as_secs()
			)));
		}
		if current == to {
			break;
		}
		current = current.saturating_add(One::one());
	}
	Ok(ret)
}

// Mirror of fc-rpc's `filter_block_logs`, but `block` carries the canonical header
// (the override doesn't mutate it), so `block_hash` here equals the fc-db mapping
// key and round-trips via `eth_getBlockByHash`. Counters kept manual to match
// upstream (`block_log_index` spans both loops, so it isn't an `enumerate`).
#[allow(clippy::explicit_counter_loop)]
fn filter_block_logs(filter: &Filter, block: EthereumBlock, statuses: Vec<TransactionStatus>) -> Vec<Log> {
	let params = FilteredParams::new(filter.clone());
	let block_hash = H256::from(keccak_256(&rlp::encode(&block.header)));
	let mut block_log_index: u32 = 0;
	let mut logs = Vec::new();
	for status in statuses.iter() {
		let mut transaction_log_index: u32 = 0;
		let transaction_hash = status.transaction_hash;
		for ethereum_log in &status.logs {
			let mut log = Log {
				address: ethereum_log.address,
				topics: ethereum_log.topics.clone(),
				data: Bytes(ethereum_log.data.clone()),
				block_hash: None,
				block_number: None,
				transaction_hash: None,
				transaction_index: None,
				log_index: None,
				transaction_log_index: None,
				removed: false,
			};
			let topics_match = filter.topics().is_empty() || params.filter_topics(&log.topics);
			let address_match = filter
				.address
				.as_ref()
				.is_none_or(|_| params.filter_address(&log.address));
			if topics_match && address_match {
				log.block_hash = Some(block_hash);
				log.block_number = Some(block.header.number);
				log.transaction_hash = Some(transaction_hash);
				log.transaction_index = Some(U256::from(status.transaction_index));
				log.log_index = Some(U256::from(block_log_index));
				log.transaction_log_index = Some(U256::from(transaction_log_index));
				logs.push(log);
			}
			transaction_log_index += 1;
			block_log_index += 1;
		}
	}
	logs
}
