//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

use std::{
	collections::{BTreeMap, HashMap},
	path::PathBuf,
	sync::Arc,
	time::Duration,
};

use crate::service::{
	rpc::{RuntimeApiStorageOverride, SchemaV1Override, SchemaV2Override, SchemaV3Override, StorageOverride},
	FullClient,
};
use cumulus_client_consensus_common::ParachainBlockImportMarker;
use fc_consensus::FrontierBlockImport;
use fc_db::Backend as FrontierBackend;
use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use fc_rpc::{EthTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_rpc::EthereumRuntimeRPCApi;
use fp_storage::EthereumStorageSchema;
use futures::{future, StreamExt};
use polkadot_cli::Cli;
use primitives::Block;
use sc_cli::SubstrateCli;
use sc_client_api::{backend::AuxStore, Backend, BlockOf, BlockchainEvents, StateBackend, StorageProvider};
use sc_consensus::{BlockCheckParams, BlockImport as BlockImportT, BlockImportParams, ImportResult};
use sc_service::{BasePath, Configuration, TFullBackend, TaskManager};
use sp_api::{ConstructRuntimeApi, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{well_known_cache_keys::Id as CacheKeyId, Error as BlockchainError, HeaderBackend, HeaderMetadata};
use sp_consensus::Error as ConsensusError;
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

/// The ethereum-compatibility configuration used to run a node.
/// evmTODO: revise settings, these are by Centrifuge
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct EthereumConfig {
	/// Maximum number of logs in a query.
	#[clap(long, default_value = "10000")]
	pub max_past_logs: u32,

	/// Maximum fee history cache size.
	#[clap(long, default_value = "2048")]
	pub fee_history_limit: u64,

	#[clap(long)]
	pub enable_dev_signer: bool,

	/// Maximum allowed gas limit will be `block.gas_limit *
	/// execute_gas_limit_multiplier` when using eth_call/eth_estimateGas.
	#[clap(long, default_value = "10")]
	pub execute_gas_limit_multiplier: u64,

	/// Size in bytes of the LRU cache for block data.
	#[clap(long, default_value = "50")]
	pub eth_log_block_cache: usize,

	/// Size in bytes of the LRU cache for transactions statuses data.
	#[clap(long, default_value = "50")]
	pub eth_statuses_cache: usize,
}

pub type Hash = sp_core::H256;

#[derive(Clone)]
pub struct BlockImport<B: BlockT, I: BlockImportT<B>, C>(FrontierBlockImport<B, I, C>);

impl<B, I, C> BlockImport<B, I, C>
where
	B: BlockT,
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	pub fn new(inner: I, client: Arc<C>, backend: Arc<fc_db::Backend<B>>) -> Self {
		Self(FrontierBlockImport::new(inner, client, backend))
	}
}

#[async_trait::async_trait]
impl<B, I, C> BlockImportT<B> for BlockImport<B, I, C>
where
	B: BlockT,
	I: BlockImportT<B, Transaction = sp_api::TransactionFor<C, B>> + Send + Sync,
	I::Error: Into<ConsensusError>,
	C: ProvideRuntimeApi<B> + Send + Sync + HeaderBackend<B> + AuxStore + BlockOf,
	C::Api: EthereumRuntimeRPCApi<B>,
	C::Api: BlockBuilderApi<B>,
{
	type Error = ConsensusError;
	type Transaction = sp_api::TransactionFor<C, B>;

	async fn check_block(&mut self, block: BlockCheckParams<B>) -> Result<ImportResult, Self::Error> {
		self.0.check_block(block).await
	}

	async fn import_block(
		&mut self,
		block: BlockImportParams<B, Self::Transaction>,
		new_cache: HashMap<CacheKeyId, Vec<u8>>,
	) -> Result<ImportResult, Self::Error> {
		self.0.import_block(block, new_cache).await
	}
}

impl<B: BlockT, I: BlockImportT<B>, C> ParachainBlockImportMarker for BlockImport<B, I, C> {}

pub fn db_config_dir(config: &Configuration) -> PathBuf {
	config
		.base_path
		.as_ref()
		.map(|base_path| base_path.config_dir(config.chain_spec.id()))
		.unwrap_or_else(|| BasePath::from_project("", "", &Cli::executable_name()).config_dir(config.chain_spec.id()))
}

pub fn spawn_frontier_tasks<RuntimeApi, Executor>(
	task_manager: &TaskManager,
	client: Arc<FullClient<RuntimeApi>>,
	backend: Arc<TFullBackend<Block>>,
	frontier_backend: Arc<FrontierBackend<Block>>,
	filter_pool: FilterPool,
	overrides: Arc<OverrideHandle<Block>>,
	fee_history_cache: FeeHistoryCache,
	fee_history_cache_limit: FeeHistoryCacheLimit,
) where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::Metadata<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<TFullBackend<Block>, Block>>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
		+ fp_rpc::EthereumRuntimeRPCApi<Block>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	task_manager.spawn_essential_handle().spawn(
		"frontier-mapping-sync-worker",
		None,
		MappingSyncWorker::new(
			client.import_notification_stream(),
			Duration::new(6, 0),
			client.clone(),
			backend,
			frontier_backend,
			3,
			0,
			SyncStrategy::Parachain,
		)
		.for_each(|()| future::ready(())),
	);

	// Spawn Frontier EthFilterApi maintenance task.
	// Each filter is allowed to stay in the pool for 100 blocks.
	const FILTER_RETAIN_THRESHOLD: u64 = 100;
	task_manager.spawn_essential_handle().spawn(
		"frontier-filter-pool",
		None,
		EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
	);

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		None,
		EthTask::fee_history_task(client, overrides, fee_history_cache, fee_history_cache_limit),
	);
}

pub fn overrides_handle<B: BlockT<Hash = H256>, C, BE>(client: Arc<C>) -> Arc<OverrideHandle<B>>
where
	C: ProvideRuntimeApi<B> + StorageProvider<B, BE> + AuxStore,
	C: HeaderBackend<B> + HeaderMetadata<B, Error = BlockchainError>,
	C: Send + Sync + 'static,
	C::Api: sp_api::ApiExt<B> + fp_rpc::EthereumRuntimeRPCApi<B> + fp_rpc::ConvertTransactionRuntimeApi<B>,
	BE: Backend<B> + 'static,
	BE::State: StateBackend<BlakeTwo256>,
{
	let mut overrides_map = BTreeMap::new();
	overrides_map.insert(
		EthereumStorageSchema::V1,
		Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V2,
		Box::new(SchemaV2Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);
	overrides_map.insert(
		EthereumStorageSchema::V3,
		Box::new(SchemaV3Override::new(client.clone())) as Box<dyn StorageOverride<_>>,
	);

	Arc::new(OverrideHandle {
		schemas: overrides_map,
		fallback: Box::new(RuntimeApiStorageOverride::new(client)),
	})
}
