// Adapted from Frontier's `fc-mapping-sync` (kv backend).
// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0
//
// Vendored verbatim except for ONE change in `sync_block`: the commitment's
// `ethereum_transaction_hashes` is taken from the synth-aware `StorageOverride`
// (our `SyntheticStorageOverride`, which returns `[real ++ synth]` statuses),
// instead of only the consensus-digest (real) hashes. That indexes the
// synthetic-tx hashes in `fc-db` so `eth_getTransactionByHash` /
// `eth_getTransactionReceipt(synthHash)` resolve. Everything else (catch-up
// sync, reorg handling, notifications) is the upstream logic unchanged.

#![allow(clippy::too_many_arguments)]

use std::{collections::HashMap, pin::Pin, sync::Arc, time::Duration};

use futures::{
	prelude::*,
	task::{Context, Poll},
};
use futures_timer::Delay;
use log::debug;
use sc_client_api::{
	backend::{Backend, StorageProvider},
	client::ImportNotifications,
};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::{Backend as _, HeaderBackend};
use sp_consensus::SyncOracle;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT, UniqueSaturatedInto, Zero};

use fc_mapping_sync::{
	emit_block_notification, BlockNotificationContext, EthereumBlockNotification, EthereumBlockNotificationSinks,
	ReorgInfo, SyncStrategy,
};
use fc_rpc::StorageOverride;
use fp_consensus::{FindLogError, Hashes, Log, PostLog, PreLog};
use fp_rpc::EthereumRuntimeRPCApi;

pub fn sync_block<Block: BlockT, C: HeaderBackend<Block>>(
	storage_override: Arc<dyn StorageOverride<Block>>,
	backend: &fc_db::kv::Backend<Block, C>,
	header: &Block::Header,
) -> Result<(), String> {
	let substrate_block_hash = header.hash();
	let block_number: u64 = (*header.number()).unique_saturated_into();

	// `[real ++ synth]` tx hashes via the synth-aware override (phase 3). Falls
	// back to the consensus-digest (real-only) hashes if the override yields
	// nothing for this block.
	let augment = |fallback: Vec<sp_core::H256>| -> Vec<sp_core::H256> {
		match storage_override.current_transaction_statuses(substrate_block_hash) {
			Some(statuses) if !statuses.is_empty() => statuses.into_iter().map(|s| s.transaction_hash).collect(),
			_ => fallback,
		}
	};

	match fp_consensus::find_log(header.digest()) {
		Ok(log) => {
			let gen_from_hashes = |hashes: Hashes| -> fc_db::kv::MappingCommitment<Block> {
				fc_db::kv::MappingCommitment {
					block_hash: substrate_block_hash,
					ethereum_block_hash: hashes.block_hash,
					ethereum_transaction_hashes: augment(hashes.transaction_hashes),
				}
			};
			let gen_from_block = |block| -> fc_db::kv::MappingCommitment<Block> {
				let hashes = Hashes::from_block(block);
				gen_from_hashes(hashes)
			};

			match log {
				Log::Pre(PreLog::Block(block)) => {
					let mapping_commitment = gen_from_block(block);
					backend.mapping().write_hashes(mapping_commitment, block_number)
				}
				Log::Post(post_log) => match post_log {
					PostLog::Hashes(hashes) => {
						let mapping_commitment = gen_from_hashes(hashes);
						backend.mapping().write_hashes(mapping_commitment, block_number)
					}
					PostLog::Block(block) => {
						let mapping_commitment = gen_from_block(block);
						backend.mapping().write_hashes(mapping_commitment, block_number)
					}
					PostLog::BlockHash(expect_eth_block_hash) => {
						let ethereum_block = storage_override.current_block(substrate_block_hash);
						match ethereum_block {
							Some(block) => {
								let got_eth_block_hash = block.header.hash();
								if got_eth_block_hash != expect_eth_block_hash {
									Err(format!(
										"Ethereum block hash mismatch: \
										frontier consensus digest ({expect_eth_block_hash:?}), \
										db state ({got_eth_block_hash:?})"
									))
								} else {
									let mapping_commitment = gen_from_block(block);
									backend.mapping().write_hashes(mapping_commitment, block_number)
								}
							}
							None => backend.mapping().write_none(substrate_block_hash),
						}
					}
				},
			}
		}
		Err(FindLogError::NotFound) => backend.mapping().write_none(substrate_block_hash),
		Err(FindLogError::MultipleLogs) => Err("Multiple logs found".to_string()),
	}
}

pub fn sync_genesis_block<Block: BlockT, C>(
	client: &C,
	backend: &fc_db::kv::Backend<Block, C>,
	header: &Block::Header,
) -> Result<(), String>
where
	C: HeaderBackend<Block> + ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
{
	let substrate_block_hash = header.hash();
	let block_number: u64 = (*header.number()).unique_saturated_into();

	if let Some(api_version) = client
		.runtime_api()
		.api_version::<dyn EthereumRuntimeRPCApi<Block>>(substrate_block_hash)
		.map_err(|e| format!("{e:?}"))?
	{
		let block = if api_version > 1 {
			client
				.runtime_api()
				.current_block(substrate_block_hash)
				.map_err(|e| format!("{e:?}"))?
		} else {
			#[allow(deprecated)]
			let legacy_block = client
				.runtime_api()
				.current_block_before_version_2(substrate_block_hash)
				.map_err(|e| format!("{e:?}"))?;
			legacy_block.map(|block| block.into())
		};
		let block_hash = block
			.ok_or_else(|| "Ethereum genesis block not found".to_string())?
			.header
			.hash();
		let mapping_commitment = fc_db::kv::MappingCommitment::<Block> {
			block_hash: substrate_block_hash,
			ethereum_block_hash: block_hash,
			ethereum_transaction_hashes: Vec::new(),
		};
		backend.mapping().write_hashes(mapping_commitment, block_number)?;
	} else {
		backend.mapping().write_none(substrate_block_hash)?;
	};

	Ok(())
}

pub fn sync_one_block<Block: BlockT, C, BE>(
	client: &C,
	substrate_backend: &BE,
	storage_override: Arc<dyn StorageOverride<Block>>,
	frontier_backend: &fc_db::kv::Backend<Block, C>,
	sync_from: <Block::Header as HeaderT>::Number,
	strategy: SyncStrategy,
	sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
	pubsub_notification_sinks: Arc<EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>>,
	best_at_import: &mut HashMap<Block::Hash, BestBlockInfo<Block>>,
) -> Result<bool, String>
where
	C: ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	C: HeaderBackend<Block> + StorageProvider<Block, BE>,
	BE: Backend<Block>,
{
	let mut current_syncing_tips = frontier_backend.meta().current_syncing_tips()?;

	if current_syncing_tips.is_empty() {
		let mut leaves = substrate_backend.blockchain().leaves().map_err(|e| format!("{e:?}"))?;
		if leaves.is_empty() {
			return Ok(false);
		}
		current_syncing_tips.append(&mut leaves);
	}

	let best_hash = client.info().best_hash;
	if SyncStrategy::Parachain == strategy && !frontier_backend.mapping().is_synced(&best_hash)? {
		current_syncing_tips.push(best_hash);
	}

	let mut operating_header = None;
	while let Some(checking_tip) = current_syncing_tips.pop() {
		if let Some(checking_header) = fetch_header(
			substrate_backend.blockchain(),
			frontier_backend,
			checking_tip,
			sync_from,
		)? {
			operating_header = Some(checking_header);
			break;
		}
	}
	let operating_header = match operating_header {
		Some(operating_header) => operating_header,
		None => {
			frontier_backend
				.meta()
				.write_current_syncing_tips(current_syncing_tips)?;
			return Ok(false);
		}
	};

	if operating_header.number() == &Zero::zero() {
		sync_genesis_block(client, frontier_backend, &operating_header)?;
		frontier_backend
			.meta()
			.write_current_syncing_tips(current_syncing_tips)?;
	} else {
		if SyncStrategy::Parachain == strategy && operating_header.number() > &client.info().best_number {
			return Ok(false);
		}
		sync_block(storage_override, frontier_backend, &operating_header)?;

		current_syncing_tips.push(*operating_header.parent_hash());
		frontier_backend
			.meta()
			.write_current_syncing_tips(current_syncing_tips)?;
	}
	let hash = operating_header.hash();
	let best_info = best_at_import.remove(&hash);
	let is_new_best = best_info.is_some() || client.info().best_hash == hash;
	let reorg_info = best_info.and_then(|info| info.reorg_info);

	if is_new_best {
		let block_number: u64 = (*operating_header.number()).unique_saturated_into();
		frontier_backend
			.mapping()
			.set_latest_canonical_indexed_block(block_number)?;
	}

	emit_block_notification(
		pubsub_notification_sinks.as_ref(),
		sync_oracle.as_ref(),
		BlockNotificationContext {
			hash,
			is_new_best,
			reorg_info,
		},
	);

	Ok(true)
}

pub fn sync_blocks<Block: BlockT, C, BE>(
	client: &C,
	substrate_backend: &BE,
	storage_override: Arc<dyn StorageOverride<Block>>,
	frontier_backend: &fc_db::kv::Backend<Block, C>,
	limit: usize,
	sync_from: <Block::Header as HeaderT>::Number,
	strategy: SyncStrategy,
	sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
	pubsub_notification_sinks: Arc<EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>>,
	best_at_import: &mut HashMap<Block::Hash, BestBlockInfo<Block>>,
) -> Result<bool, String>
where
	C: ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	C: HeaderBackend<Block> + StorageProvider<Block, BE>,
	BE: Backend<Block>,
{
	let mut synced_any = false;

	for _ in 0..limit {
		synced_any = synced_any
			|| sync_one_block(
				client,
				substrate_backend,
				storage_override.clone(),
				frontier_backend,
				sync_from,
				strategy,
				sync_oracle.clone(),
				pubsub_notification_sinks.clone(),
				best_at_import,
			)?;
	}

	let finalized_number = client.info().finalized_number;
	best_at_import.retain(|_, info| info.block_number > finalized_number);

	Ok(synced_any)
}

pub fn fetch_header<Block: BlockT, C, BE>(
	substrate_backend: &BE,
	frontier_backend: &fc_db::kv::Backend<Block, C>,
	checking_tip: Block::Hash,
	sync_from: <Block::Header as HeaderT>::Number,
) -> Result<Option<Block::Header>, String>
where
	C: HeaderBackend<Block>,
	BE: HeaderBackend<Block>,
{
	if frontier_backend.mapping().is_synced(&checking_tip)? {
		return Ok(None);
	}

	match substrate_backend.header(checking_tip) {
		Ok(Some(checking_header)) if checking_header.number() >= &sync_from => Ok(Some(checking_header)),
		Ok(Some(_)) => Ok(None),
		Ok(None) | Err(_) => Err("Header not found".to_string()),
	}
}

/// Information tracked at import time for a block that was `is_new_best`.
pub struct BestBlockInfo<Block: BlockT> {
	pub block_number: <Block::Header as HeaderT>::Number,
	pub reorg_info: Option<Arc<ReorgInfo<Block>>>,
}

pub struct SyntheticMappingSyncWorker<Block: BlockT, C, BE> {
	import_notifications: ImportNotifications<Block>,
	timeout: Duration,
	inner_delay: Option<Delay>,

	client: Arc<C>,
	substrate_backend: Arc<BE>,
	storage_override: Arc<dyn StorageOverride<Block>>,
	frontier_backend: Arc<fc_db::kv::Backend<Block, C>>,

	have_next: bool,
	retry_times: usize,
	sync_from: <Block::Header as HeaderT>::Number,
	strategy: SyncStrategy,

	sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
	pubsub_notification_sinks: Arc<EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>>,

	best_at_import: HashMap<Block::Hash, BestBlockInfo<Block>>,
}

impl<Block: BlockT, C, BE> Unpin for SyntheticMappingSyncWorker<Block, C, BE> {}

impl<Block: BlockT, C, BE> SyntheticMappingSyncWorker<Block, C, BE> {
	pub fn new(
		import_notifications: ImportNotifications<Block>,
		timeout: Duration,
		client: Arc<C>,
		substrate_backend: Arc<BE>,
		storage_override: Arc<dyn StorageOverride<Block>>,
		frontier_backend: Arc<fc_db::kv::Backend<Block, C>>,
		retry_times: usize,
		sync_from: <Block::Header as HeaderT>::Number,
		strategy: SyncStrategy,
		sync_oracle: Arc<dyn SyncOracle + Send + Sync + 'static>,
		pubsub_notification_sinks: Arc<EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>>,
	) -> Self {
		Self {
			import_notifications,
			timeout,
			inner_delay: None,

			client,
			substrate_backend,
			storage_override,
			frontier_backend,

			have_next: true,
			retry_times,
			sync_from,
			strategy,

			sync_oracle,
			pubsub_notification_sinks,
			best_at_import: HashMap::new(),
		}
	}
}

impl<Block, C, BE> Stream for SyntheticMappingSyncWorker<Block, C, BE>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	C: HeaderBackend<Block> + StorageProvider<Block, BE>,
	BE: Backend<Block>,
{
	type Item = ();

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<()>> {
		let mut fire = false;

		loop {
			match Stream::poll_next(Pin::new(&mut self.import_notifications), cx) {
				Poll::Pending => break,
				Poll::Ready(Some(notification)) => {
					fire = true;
					if notification.is_new_best {
						let reorg_info = notification
							.tree_route
							.as_ref()
							.map(|tree_route| Arc::new(ReorgInfo::from_tree_route(tree_route, notification.hash)));
						self.best_at_import.insert(
							notification.hash,
							BestBlockInfo {
								block_number: *notification.header.number(),
								reorg_info,
							},
						);
					}
				}
				Poll::Ready(None) => return Poll::Ready(None),
			}
		}

		let timeout = self.timeout;
		let inner_delay = self.inner_delay.get_or_insert_with(|| Delay::new(timeout));

		match Future::poll(Pin::new(inner_delay), cx) {
			Poll::Pending => (),
			Poll::Ready(()) => {
				fire = true;
			}
		}

		if self.have_next {
			fire = true;
		}

		if fire {
			self.inner_delay = None;

			let mut best_at_import = std::mem::take(&mut self.best_at_import);

			let result = sync_blocks(
				self.client.as_ref(),
				self.substrate_backend.as_ref(),
				self.storage_override.clone(),
				self.frontier_backend.as_ref(),
				self.retry_times,
				self.sync_from,
				self.strategy,
				self.sync_oracle.clone(),
				self.pubsub_notification_sinks.clone(),
				&mut best_at_import,
			);

			self.best_at_import = best_at_import;

			match result {
				Ok(have_next) => {
					self.have_next = have_next;
					Poll::Ready(Some(()))
				}
				Err(e) => {
					self.have_next = false;
					debug!(target: "mapping-sync", "Syncing failed with error {e:?}, retrying.");
					Poll::Ready(Some(()))
				}
			}
		} else {
			Poll::Pending
		}
	}
}
