//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

#![allow(clippy::all)]

pub use crate::client::{AbstractClient, Client, ClientHandle, ExecuteWithClient, RuntimeApiCollection};
use hydra_dx_runtime;
use common_runtime::Block;
use sc_client_api::{ExecutorProvider, RemoteBackend};
pub use sc_executor::NativeExecutor;
use sc_executor::{native_executor_instance, NativeExecutionDispatch};
use sc_finality_grandpa::FinalityProofProvider as GrandpaFinalityProofProvider;
pub use sp_api::{ConstructRuntimeApi, ProvideRuntimeApi, StateBackend};
use sc_service::{error::Error as ServiceError, ChainSpec, Configuration, TaskManager};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_inherents::InherentDataProviders;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use sp_trie::PrefixedMemoryDB;
use std::sync::Arc;
use std::time::Duration;
use testing_hydra_dx_runtime;

// Our native executor instance.
native_executor_instance!(
	pub HydraExecutor,
	hydra_dx_runtime::api::dispatch,
	hydra_dx_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);

native_executor_instance!(
	pub TestingHydraExecutor,
	testing_hydra_dx_runtime::api::dispatch,
	testing_hydra_dx_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);

pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

pub type FullBackend = sc_service::TFullBackend<Block>;
pub type LightBackend = sc_service::TLightBackendWithHash<Block, sp_runtime::traits::BlakeTwo256>;

pub type FullClient<RuntimeApi, Executor> = sc_service::TFullClient<Block, RuntimeApi, Executor>;
pub type LightClient<RuntimeApi, Executor> =
	sc_service::TLightClientWithBackend<Block, RuntimeApi, Executor, LightBackend>;

pub type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
pub type FullGrandpaBlockImport<RuntimeApi, Executor> =
	sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient<RuntimeApi, Executor>, FullSelectChain>;

/// Can be called for a `Configuration` to check what node it belongs to.
pub trait IdentifyVariant {
	/// Returns if this is a configuration for the `Hydra DX` node.
	fn is_hydra_dx_runtime(&self) -> bool;

	/// Returns if this is a configuration for the `Testing Hydra DX` node.
	fn is_testing_runtime(&self) -> bool;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
	fn is_hydra_dx_runtime(&self) -> bool {
		self.name().to_lowercase().starts_with("hydra") || self.name().to_lowercase().starts_with("hdx")
	}
	fn is_testing_runtime(&self) -> bool {
		self.name().to_lowercase().starts_with("test")
	}
}

pub fn new_chain_ops(
	mut config: &mut Configuration,
) -> Result<
	(
		Arc<Client>,
		Arc<FullBackend>,
		sp_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		TaskManager,
	),
	ServiceError,
> {
	config.keystore = sc_service::config::KeystoreConfig::InMemory;
	if config.chain_spec.is_testing_runtime() {
		let sc_service::PartialComponents {
			client,
			backend,
			import_queue,
			task_manager,
			..
		} = new_partial::<testing_hydra_dx_runtime::RuntimeApi, TestingHydraExecutor>(config)?;
		Ok((
			Arc::new(Client::TestingHydraDX(client)),
			backend,
			import_queue,
			task_manager,
		))
	} else {
		let sc_service::PartialComponents {
			client,
			backend,
			import_queue,
			task_manager,
			..
		} = new_partial::<hydra_dx_runtime::RuntimeApi, HydraExecutor>(config)?;
		Ok((Arc::new(Client::HydraDX(client)), backend, import_queue, task_manager))
	}
}

pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
) -> Result<
	sc_service::PartialComponents<
		FullClient<RuntimeApi, Executor>,
		FullBackend,
		FullSelectChain,
		sp_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
		(
			impl Fn(sc_rpc::DenyUnsafe, sc_rpc::SubscriptionTaskExecutor) -> RpcExtension,
			(
				sc_consensus_babe::BabeBlockImport<
					Block,
					FullClient<RuntimeApi, Executor>,
					FullGrandpaBlockImport<RuntimeApi, Executor>,
				>,
				sc_finality_grandpa::LinkHalf<Block, FullClient<RuntimeApi, Executor>, FullSelectChain>,
				sc_consensus_babe::BabeLink<Block>,
			),
			sc_finality_grandpa::SharedVoterState,
			Option<Telemetry>,
		),
	>,
	ServiceError,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	let inherent_data_providers = sp_inherents::InherentDataProviders::new();
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(move |endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, Executor>(
		&config,
		telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
	)?;

	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
		client.clone(),
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;
  
	let justification_import = grandpa_block_import.clone();

	let (block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::Config::get_or_compute(&*client)?,
		grandpa_block_import,
		client.clone(),
	)?;

	let import_queue = sc_consensus_babe::import_queue(
		babe_link.clone(),
		block_import.clone(),
		Some(Box::new(justification_import)),
		client.clone(),
		select_chain.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
		sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let import_setup = (block_import, grandpa_link, babe_link);

	let (rpc_extensions_builder, rpc_setup) = {
		let (_, grandpa_link, babe_link) = &import_setup;

		let justification_stream = grandpa_link.justification_stream();
		let shared_authority_set = grandpa_link.shared_authority_set().clone();
		let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();
		let finality_proof_provider =
			GrandpaFinalityProofProvider::new_for_service(backend.clone(), Some(shared_authority_set.clone()));

		let rpc_setup = shared_voter_state.clone();

		let babe_config = babe_link.config().clone();
		let shared_epoch_changes = babe_link.epoch_changes().clone();

		let client = client.clone();
		let pool = transaction_pool.clone();
		let select_chain = select_chain.clone();
		let keystore = keystore_container.sync_keystore();

		let rpc_extensions_builder = move |deny_unsafe, subscription_executor| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				select_chain: select_chain.clone(),
				deny_unsafe,
				babe: crate::rpc::BabeDeps {
					babe_config: babe_config.clone(),
					shared_epoch_changes: shared_epoch_changes.clone(),
					keystore: keystore.clone(),
				},
				grandpa: crate::rpc::GrandpaDeps {
					shared_voter_state: shared_voter_state.clone(),
					shared_authority_set: shared_authority_set.clone(),
					justification_stream: justification_stream.clone(),
					subscription_executor,
					finality_provider: finality_proof_provider.clone(),
				},
			};

			crate::rpc::create_full(deps)
		};

		(rpc_extensions_builder, rpc_setup)
	};

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		keystore_container,
		select_chain,
		import_queue,
		transaction_pool,
		inherent_data_providers,
		other: (rpc_extensions_builder, import_setup, rpc_setup, telemetry),
	})
}

pub struct NewFull<C> {
	pub task_manager: TaskManager,
	pub client: C,
	pub inherent_data_providers: InherentDataProviders,
	pub network: Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
	pub network_status_sinks: sc_service::NetworkStatusSinks<Block>,
}

impl<C> NewFull<C> {
	/// Convert the client type using the given `func`.
	pub fn with_client<NC>(self, func: impl FnOnce(C) -> NC) -> NewFull<NC> {
		NewFull {
			task_manager: self.task_manager,
			client: func(self.client),
			inherent_data_providers: self.inherent_data_providers,
			network: self.network,
			network_status_sinks: self.network_status_sinks,
		}
	}
}

pub fn build_full(config: Configuration, run_testing_runtime: bool) -> Result<NewFull<Client>, ServiceError> {
	if run_testing_runtime {
		new_full::<testing_hydra_dx_runtime::RuntimeApi, TestingHydraExecutor>(config)
			.map(|full| full.with_client(Client::TestingHydraDX))
	} else {
		new_full::<hydra_dx_runtime::RuntimeApi, HydraExecutor>(config).map(|full| full.with_client(Client::HydraDX))
	}
}

/// Builds a new service for a full client.
pub fn new_full<RuntimeApi, Executor>(
	mut config: Configuration,
) -> Result<NewFull<Arc<FullClient<RuntimeApi, Executor>>>, ServiceError>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		inherent_data_providers,
		other: (rpc_extensions_builder, import_setup, rpc_setup, mut telemetry),
	} = new_partial::<RuntimeApi, Executor>(&config)?;

	let backoff_authoring_blocks = Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());

	let shared_voter_state = rpc_setup;

	config
		.network
		.extra_sets
		.push(sc_finality_grandpa::grandpa_peers_set_config());

	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(&config, task_manager.spawn_handle(), client.clone(), network.clone());
	}

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let name = config.network.node_name.clone();
	let enable_grandpa = !config.disable_grandpa;
	let prometheus_registry = config.prometheus_registry().cloned();

	let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		config,
		backend: backend,
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		network: network.clone(),
		rpc_extensions_builder: Box::new(rpc_extensions_builder),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		on_demand: None,
		remote_blockchain: None,
		network_status_sinks: network_status_sinks.clone(),
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	let (block_import, grandpa_link, babe_link) = import_setup;

	if role.is_authority() {
		let proposer = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		let babe_config = sc_consensus_babe::BabeParams {
			keystore: keystore_container.sync_keystore(),
			client: client.clone(),
			select_chain,
			block_import,
			env: proposer,
			sync_oracle: network.clone(),
			inherent_data_providers: inherent_data_providers.clone(),
			force_authoring,
			backoff_authoring_blocks,
			babe_link,
			can_author_with,
			block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(2f32 / 3f32),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
		};

		let babe = sc_consensus_babe::start_babe(babe_config)?;
		task_manager.spawn_essential_handle().spawn_blocking("babe", babe);
	}

	let _authority_discovery_service = if role.is_authority() {
		use futures::StreamExt;
		use sc_network::Event;

		let authority_discovery_role = if role.is_authority() {
			sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore())
		} else {
			// don't publish our addresses when we're only a collator
			sc_authority_discovery::Role::Discover
		};
		let dht_event_stream = network.event_stream("authority-discovery").filter_map(|e| async move {
			match e {
				Event::Dht(e) => Some(e),
				_ => None,
			}
		});
		let (worker, service) = sc_authority_discovery::new_worker_and_service(
			client.clone(),
			network.clone(),
			Box::pin(dht_event_stream),
			authority_discovery_role,
			prometheus_registry.clone(),
		);

		task_manager
			.spawn_handle()
			.spawn("authority-discovery-worker", worker.run());
		Some(service)
	} else {
		None
	};

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.

	let keystore = if role.is_authority() {
		Some(keystore_container.sync_keystore())
	} else {
		None
	};

	let grandpa_config = sc_finality_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: Duration::from_millis(1000),
		justification_period: 512,
		name: Some(name),
		observer_enabled: false,
		keystore,
		is_authority: role.is_authority(),
		telemetry: telemetry.as_ref().map(|x| x.handle()),
	};

	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_finality_grandpa::GrandpaParams {
			config: grandpa_config,
			link: grandpa_link,
			network: network.clone(),
			voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry: prometheus_registry,
			shared_voter_state,
			telemetry: telemetry.as_ref().map(|x| x.handle()),
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager
			.spawn_essential_handle()
			.spawn_blocking("grandpa-voter", sc_finality_grandpa::run_grandpa_voter(grandpa_config)?);
	}

	network_starter.start_network();
	Ok(NewFull {
		task_manager,
		client,
		inherent_data_providers,
		network,
		network_status_sinks,
	})
}

pub fn build_light(config: Configuration, is_testing_runtime: bool) -> Result<TaskManager, ServiceError> {
	if is_testing_runtime {
		new_light::<testing_hydra_dx_runtime::RuntimeApi, TestingHydraExecutor>(config)
	} else {
		new_light::<hydra_dx_runtime::RuntimeApi, HydraExecutor>(config)
	}
}

/// Builds a new service for a light client.
pub fn new_light<RuntimeApi, Executor>(config: Configuration) -> Result<TaskManager, ServiceError>
where
	RuntimeApi: 'static + Send + Sync + ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>>,
	<RuntimeApi as ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>>>::RuntimeApi:
		RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<LightBackend, Block>>,
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

	let (client, backend, keystore_container, mut task_manager, on_demand) =
		sc_service::new_light_parts::<Block, RuntimeApi, Executor>(
			&config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
		)?;

	let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
		on_demand.clone(),
	));

	let mut telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let (grandpa_block_import, _) = sc_finality_grandpa::block_import(
		client.clone(),
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;
	let justification_import = grandpa_block_import.clone();

	let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::Config::get_or_compute(&*client)?,
		grandpa_block_import,
		client.clone(),
	)?;

	let import_queue = sc_consensus_babe::import_queue(
		babe_link,
		babe_block_import,
		Some(Box::new(justification_import)),
		client.clone(),
		select_chain,
		InherentDataProviders::new(),
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
		sp_consensus::NeverCanAuthor,
		telemetry.as_ref().map(|x| x.handle()),
	)?;

	let (network, network_status_sinks, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: Some(on_demand.clone()),
			block_announce_validator_builder: None,
		})?;

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(&config, task_manager.spawn_handle(), client.clone(), network.clone());
	}

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		remote_blockchain: Some(backend.remote_blockchain()),
		transaction_pool,
		task_manager: &mut task_manager,
		on_demand: Some(on_demand),
		rpc_extensions_builder: Box::new(|_, _| ()),
		config,
		client,
		keystore: keystore_container.sync_keystore(),
		backend,
		network,
		network_status_sinks,
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	network_starter.start_network();

	Ok(task_manager)
}
