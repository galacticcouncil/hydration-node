// This file is part of HydraDX-node.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

#![allow(clippy::all)]

use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::{ParachainBlockImport as TParachainBlockImport, ParachainConsensus};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::{CollectCollationInfo, ParaId};
use cumulus_relay_chain_inprocess_interface::build_inprocess_relay_chain;
use cumulus_relay_chain_interface::{RelayChainError, RelayChainInterface, RelayChainResult};
use cumulus_relay_chain_minimal_node::build_minimal_relay_chain_node;
use fc_db::Backend as FrontierBackend;
use fc_rpc::{EthBlockDataCacheTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use fp_rpc::{ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use jsonrpsee::RpcModule;
use polkadot_service::CollatorPair;
use primitives::{AccountId, Balance, Block, Index};
use sc_consensus::ImportQueue;
use sc_executor::{NativeElseWasmExecutor, NativeExecutionDispatch, NativeVersion, WasmExecutor};
use sc_network::NetworkService;
use sc_network_common::service::NetworkBlock;
use sc_rpc::SubscriptionTaskExecutor;
use sc_rpc_api::DenyUnsafe;
use sc_service::{Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_api::ConstructRuntimeApi;
use sp_keystore::SyncCryptoStorePtr;
use std::{
	collections::BTreeMap,
	sync::{Arc, Mutex},
	time::Duration,
};
use substrate_prometheus_endpoint::Registry;

use crate::evm::Hash;
use crate::{evm, rpc};

// native executor instance.
pub struct HydraDXExecutorDispatch;
impl NativeExecutionDispatch for HydraDXExecutorDispatch {
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		hydradx_runtime::api::dispatch(method, data)
	}

	fn native_version() -> NativeVersion {
		hydradx_runtime::native_version()
	}
}

#[cfg(not(feature = "runtime-benchmarks"))]
pub type HostFunctions = sp_io::SubstrateHostFunctions;

#[cfg(feature = "runtime-benchmarks")]
pub type HostFunctions = (
	sp_io::SubstrateHostFunctions,
	frame_benchmarking::benchmarking::HostFunctions,
);

pub type FullBackend = TFullBackend<Block>;
pub type FullClient<RuntimeApi> = TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<HostFunctions>>;

pub type ParachainBlockImport<RuntimeApi> = TParachainBlockImport<Block, Arc<FullClient<RuntimeApi>>, FullBackend>;

/// Build the import queue for the parachain runtime.
pub fn parachain_build_import_queue<RuntimeApi, Executor>(
	client: Arc<FullClient<RuntimeApi>>,
	backend: Arc<FullBackend>,
	config: &Configuration,
	telemetry: Option<TelemetryHandle>,
	task_manager: &TaskManager,
	frontier_backend: Arc<FrontierBackend<Block>>,
) -> Result<sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi>>, sc_service::Error>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		// + sp_api::ApiExt<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_consensus_aura::AuraApi<Block, sp_consensus_aura::sr25519::AuthorityId>,
	Executor: NativeExecutionDispatch + 'static,
{
	let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
	let block_import = evm::BlockImport::new(
		ParachainBlockImport::new(client.clone(), backend.clone()),
		client.clone(),
		frontier_backend,
	);

	cumulus_client_consensus_aura::import_queue::<sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _>(
		cumulus_client_consensus_aura::ImportQueueParams {
			block_import,
			client,
			create_inherent_data_providers: move |_, _| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
				);

				Ok((slot, timestamp))
			},
			registry: config.prometheus_registry().clone(),
			spawner: &task_manager.spawn_essential_handle(),
			telemetry,
		},
	)
	.map_err(Into::into)
}

pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi>,
		FullBackend,
		(),
		sc_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>,
		(
			Option<Telemetry>,
			Option<TelemetryWorkerHandle>,
			Arc<FrontierBackend<Block>>,
			FilterPool,
			FeeHistoryCache,
		),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		// + sp_api::ApiExt<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_consensus_aura::AuraApi<Block, sp_consensus_aura::sr25519::AuthorityId>,
	Executor: NativeExecutionDispatch + 'static,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<Executor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let registry = config.prometheus_registry();

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		registry,
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let frontier_backend = Arc::new(FrontierBackend::open(
		Arc::clone(&client),
		&config.database,
		&evm::db_config_dir(config),
	)?);

	let import_queue = parachain_build_import_queue::<RuntimeApi, Executor>(
		client.clone(),
		backend.clone(),
		config,
		telemetry.as_ref().map(|telemetry| telemetry.handle()),
		&task_manager,
		frontier_backend,
	)?;

	let filter_pool: FilterPool = Arc::new(Mutex::new(BTreeMap::new()));
	let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));

	Ok(PartialComponents {
		client,
		backend,
		import_queue,
		task_manager,
		keystore_container,
		transaction_pool,
		select_chain: (),
		other: (
			telemetry,
			telemetry_worker_handle,
			frontier_backend,
			filter_pool,
			fee_history_cache,
		),
	})
}

/// Build a relay chain interface.
/// Will return a minimal relay chain node with RPC
/// client or an inprocess node, based on the [`CollatorOptions`] passed in.
async fn build_relay_chain_interface(
	polkadot_config: Configuration,
	parachain_config: &Configuration,
	telemetry_worker_handle: Option<TelemetryWorkerHandle>,
	task_manager: &mut TaskManager,
	collator_options: CollatorOptions,
) -> RelayChainResult<(Arc<(dyn RelayChainInterface + 'static)>, Option<CollatorPair>)> {
	if !collator_options.relay_chain_rpc_urls.is_empty() {
		build_minimal_relay_chain_node(polkadot_config, task_manager, collator_options.relay_chain_rpc_urls).await
	} else {
		build_inprocess_relay_chain(
			polkadot_config,
			parachain_config,
			telemetry_worker_handle,
			task_manager,
			None,
		)
	}
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
async fn start_node_impl<RuntimeApi, Executor, RpcExtBuilder, ConsensusBuilder>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	ethereum_config: evm::EthereumConfig,
	collator_options: CollatorOptions,
	para_id: ParaId,
	rpc_ext_builder: RpcExtBuilder,
	build_consensus: ConsensusBuilder,
) -> sc_service::error::Result<(Arc<FullClient<RuntimeApi>>, TaskManager)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		// + sp_api::ApiExt<Block>
		+ sp_api::ApiExt<Block, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ CollectCollationInfo<Block>
		+ sp_consensus_aura::AuraApi<Block, sp_consensus_aura::sr25519::AuthorityId>
		+ cumulus_primitives_core::CollectCollationInfo<Block>
		+ EthereumRuntimeRPCApi<Block>
		+ ConvertTransactionRuntimeApi<Block>,
	Executor: NativeExecutionDispatch + 'static,
	RpcExtBuilder: Fn(
			Arc<FullClient<RuntimeApi>>,
			Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
			DenyUnsafe,
			SubscriptionTaskExecutor,
			Arc<NetworkService<Block, Hash>>,
			Arc<FrontierBackend<Block>>,
			FilterPool,
			FeeHistoryCache,
			Arc<OverrideHandle<Block>>,
			Arc<EthBlockDataCacheTask<Block>>,
		) -> Result<rpc::RpcExtension, sc_service::Error>
		+ 'static,
	ConsensusBuilder: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		Option<&Registry>,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<dyn RelayChainInterface>,
		Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
		Arc<NetworkService<Block, Hash>>,
		SyncCryptoStorePtr,
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial::<RuntimeApi, Executor>(&parachain_config)?;
	let (mut telemetry, telemetry_worker_handle, frontier_backend, filter_pool, fee_history_cache) = params.other;
	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) = build_relay_chain_interface(
		polkadot_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
	)
	.await
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let block_announce_validator = BlockAnnounceValidator::new(relay_chain_interface.clone(), para_id);

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let (network, system_rpc_tx, tx_handler_controller, start_network) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue: params.import_queue,
			block_announce_validator_builder: Some(Box::new(|_| Box::new(block_announce_validator))),
			warp_sync: None,
		})?;

	let rpc_client = client.clone();
	let overrides = evm::overrides_handle(client.clone());
	let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
		task_manager.spawn_handle(),
		overrides.clone(),
		ethereum_config.eth_log_block_cache,
		ethereum_config.eth_statuses_cache,
		prometheus_registry.clone(),
	));

	let rpc_builder = {
		let network = network.clone();
		let pool = transaction_pool.clone();
		let frontier_backend = frontier_backend.clone();
		let fee_history_cache = fee_history_cache.clone();
		let filter_pool = filter_pool.clone();
		let overrides = overrides.clone();
		move |deny, subscription_task_executor| {
			rpc_ext_builder(
				rpc_client.clone(),
				pool.clone(),
				deny,
				subscription_task_executor,
				network.clone(),
				frontier_backend.clone(),
				filter_pool.clone(),
				fee_history_cache.clone(),
				overrides.clone(),
				block_data_cache.clone(),
			)
		}
	};

	if parachain_config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&parachain_config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder: Box::new(rpc_builder),
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		system_rpc_tx,
		tx_handler_controller,
		telemetry: telemetry.as_mut(),
	})?;

	evm::spawn_frontier_tasks::<RuntimeApi, Executor>(
		&task_manager,
		client.clone(),
		backend.clone(),
		frontier_backend.clone(),
		filter_pool.clone(),
		overrides,
		fee_history_cache.clone(),
		ethereum_config.fee_history_limit,
	);

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			ParachainBlockImport::new(client.clone(), backend.clone()),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			network,
			params.keystore_container.sync_keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();

		let params = StartCollatorParams {
			para_id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue: import_queue_service,
			collator_key: collator_key.expect("Command line arguments do not allow this. qed"),
			relay_chain_slot_duration,
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id,
			relay_chain_interface,
			relay_chain_slot_duration,
			import_queue: import_queue_service,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((client, task_manager))
}

/// Start a normal parachain node.
// #[sc_tracing::logging::prefix_logs_with("Parachain")]
pub async fn start_node(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	ethereum_config: evm::EthereumConfig,
	collator_options: CollatorOptions,
	para_id: ParaId,
) -> sc_service::error::Result<(Arc<FullClient<hydradx_runtime::RuntimeApi>>, TaskManager)> {
	let is_authority = parachain_config.role.is_authority();

	start_node_impl::<hydradx_runtime::RuntimeApi, HydraDXExecutorDispatch, _, _>(
		parachain_config,
		polkadot_config,
		ethereum_config,
		collator_options,
		para_id,
		move |client,
		      pool,
		      deny_unsafe,
		      subscription_task_executor,
		      network,
		      frontier_backend,
		      filter_pool,
		      fee_history_cache,
		      overrides,
		      block_data_cache| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
			};
			let mut module = rpc::create_full(deps)?;
			let eth_deps = rpc::Deps {
				client,
				pool: pool.clone(),
				graph: pool.pool().clone(),
				converter: Some(hydradx_runtime::TransactionConverter),
				is_authority,
				enable_dev_signer: ethereum_config.enable_dev_signer,
				network,
				frontier_backend,
				overrides,
				block_data_cache,
				filter_pool,
				max_past_logs: ethereum_config.max_past_logs,
				fee_history_cache,
				fee_history_cache_limit: ethereum_config.fee_history_limit,
				execute_gas_limit_multiplier: ethereum_config.execute_gas_limit_multiplier,
			};
			let module = rpc::create(module, eth_deps, subscription_task_executor)?;
			Ok(module)
		},
		|client,
		 block_import,
		 prometheus_registry,
		 telemetry,
		 task_manager,
		 relay_chain_interface,
		 transaction_pool,
		 sync_oracle,
		 keystore,
		 force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry.clone(),
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
			>(BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								para_id,
							)
							.await;

						let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

						let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from("Failed to create parachain inherent")
						})?;
						Ok((slot, timestamp, parachain_inherent))
					}
				},
				block_import,
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry,
			}))
		},
	)
	.await
}
