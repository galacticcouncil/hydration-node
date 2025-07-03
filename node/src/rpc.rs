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

use std::sync::Arc;

use cumulus_primitives_core::PersistedValidationData;
use cumulus_primitives_parachain_inherent::ParachainInherentData;
use cumulus_test_relay_sproof_builder::RelayStateSproofBuilder;
use fc_db::kv::Backend as FrontierBackend;
pub use fc_rpc::{EthBlockDataCacheTask, StorageOverride, StorageOverrideHandler};
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use fp_rpc::{ConvertTransaction, ConvertTransactionRuntimeApi, EthereumRuntimeRPCApi};
use hydradx_runtime::{opaque::Block, AccountId, Balance, Index};
use pallet_ismp_rpc::{IsmpApiServer, IsmpRpcHandler};
use sc_client_api::{
	backend::{Backend, StateBackend, StorageProvider},
	client::BlockchainEvents,
	BlockBackend, ProofProvider,
};
use sc_network::service::traits::NetworkService;
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;
use sc_transaction_pool::{ChainApi, Pool};
use sc_transaction_pool_api::TransactionPool;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

pub struct HydraDxEthConfig<C, BE>(std::marker::PhantomData<(C, BE)>);

impl<C, BE> fc_rpc::EthConfig<Block, C> for HydraDxEthConfig<C, BE>
where
	C: sc_client_api::StorageProvider<Block, BE> + Sync + Send + 'static,
	BE: Backend<Block> + 'static,
{
	type EstimateGasAdapter = ();
	type RuntimeStorageOverride = fc_rpc::frontier_backend_client::SystemAccountId20StorageOverride<Block, C, BE>;
}

/// Full client dependencies.
pub struct FullDeps<C, P, B> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Backend used by the node.
	pub backend: Arc<B>,
}

/// Extra dependencies for Ethereum compatibility.
pub struct Deps<C, P, A: ChainApi, CT> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Graph pool instance.
	pub graph: Arc<Pool<A>>,
	/// Ethereum transaction converter.
	pub converter: Option<CT>,
	/// The Node authority flag
	pub is_authority: bool,
	/// Whether to enable dev signer
	pub enable_dev_signer: bool,
	/// Network service
	pub network: Arc<dyn NetworkService>,
	/// Chain syncing service
	pub sync: Arc<SyncingService<Block>>,
	/// Frontier Backend.
	pub frontier_backend: Arc<FrontierBackend<Block, C>>,
	/// Ethereum data access overrides.
	pub overrides: Arc<dyn StorageOverride<Block>>,
	/// Cache for Ethereum block data.
	pub block_data_cache: Arc<EthBlockDataCacheTask<Block>>,
	/// EthFilterApi pool.
	pub filter_pool: FilterPool,
	/// Maximum number of logs in a query.
	pub max_past_logs: u32,
	/// Fee history cache.
	pub fee_history_cache: FeeHistoryCache,
	/// Maximum fee history cache size.
	pub fee_history_cache_limit: FeeHistoryCacheLimit,
	/// Maximum allowed gas limit will be ` block.gas_limit *
	/// execute_gas_limit_multiplier` when using eth_call/eth_estimateGas.
	pub execute_gas_limit_multiplier: u64,
}

/// RPC Extension Builder
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Instantiate all full RPC extensions.
pub fn create_full<C, P, B>(deps: FullDeps<C, P, B>) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block> + BlockBackend<Block> + ProofProvider<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: pallet_ismp_runtime_api::IsmpRuntimeApi<Block, sp_core::H256>,
	C::Api: BlockBuilderApi<Block>,
	P: TransactionPool + Sync + Send + 'static,
	B: sc_client_api::Backend<Block> + Send + Sync + 'static,
	B::State: sc_client_api::StateBackend<sp_runtime::traits::HashingFor<Block>>,
{
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
	use substrate_frame_rpc_system::{System, SystemApiServer};
	use substrate_state_trie_migration_rpc::{StateMigration, StateMigrationApiServer};

	let mut module = RpcExtension::new(());
	let FullDeps { client, pool, backend } = deps;

	module.merge(System::new(client.clone(), pool).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
	module.merge(StateMigration::new(client.clone(), backend.clone()).into_rpc())?;

	module.merge(IsmpRpcHandler::new(client, backend)?.into_rpc())?;

	Ok(module)
}

/// Instantiate Ethereum-compatible RPC extensions.
pub fn create<C, BE, P, A, CT>(
	mut io: RpcExtension,
	deps: Deps<C, P, A, CT>,
	subscription_task_executor: SubscriptionTaskExecutor,
	pubsub_notification_sinks: Arc<
		fc_mapping_sync::EthereumBlockNotificationSinks<fc_mapping_sync::EthereumBlockNotification<Block>>,
	>,
) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block>,
	C::Api: BlockBuilderApi<Block> + EthereumRuntimeRPCApi<Block> + ConvertTransactionRuntimeApi<Block>,
	C: BlockchainEvents<Block> + 'static,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + StorageProvider<Block, BE>,
	C: CallApiAt<Block>,
	BE: Backend<Block> + 'static,
	BE::State: StateBackend<BlakeTwo256>,
	P: TransactionPool<Block = Block> + 'static,
	A: ChainApi<Block = Block> + 'static,
	CT: ConvertTransaction<<Block as BlockT>::Extrinsic> + Send + Sync + 'static,
{
	use fc_rpc::{
		Eth, EthApiServer, EthDevSigner, EthFilter, EthFilterApiServer, EthPubSub, EthPubSubApiServer, EthSigner, Net,
		NetApiServer, Web3, Web3ApiServer,
	};

	let Deps {
		client,
		pool,
		graph,
		converter,
		is_authority,
		enable_dev_signer,
		network,
		sync,
		frontier_backend,
		overrides,
		block_data_cache,
		filter_pool,
		max_past_logs,
		fee_history_cache,
		fee_history_cache_limit,
		execute_gas_limit_multiplier,
	} = deps;

	let mut signers = Vec::new();
	if enable_dev_signer {
		signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);
	}

	let pending_create_inherent_data_providers = move |_, _| async move {
		let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
		// Create a dummy parachain inherent data provider which is required to pass
		// the checks by the para chain system. We use dummy values because in the 'pending context'
		// neither do we have access to the real values nor do we need them.
		let (relay_parent_storage_root, relay_chain_state) =
			RelayStateSproofBuilder::default().into_state_root_and_proof();
		let vfp = PersistedValidationData {
			// This is a hack to make `cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases`
			// happy. Relay parent number can't be bigger than u32::MAX.
			relay_parent_number: u32::MAX,
			relay_parent_storage_root,
			..Default::default()
		};
		let parachain_inherent_data = ParachainInherentData {
			validation_data: vfp,
			relay_chain_state,
			downward_messages: Default::default(),
			horizontal_messages: Default::default(),
		};
		Ok((timestamp, parachain_inherent_data))
	};

	io.merge(
		Eth::<_, _, _, _, _, _, _, HydraDxEthConfig<_, _>>::new(
			client.clone(),
			pool.clone(),
			graph.clone(),
			converter,
			sync.clone(),
			vec![],
			overrides.clone(),
			frontier_backend.clone(),
			is_authority,
			block_data_cache.clone(),
			fee_history_cache,
			fee_history_cache_limit,
			execute_gas_limit_multiplier,
			None,
			pending_create_inherent_data_providers,
			None,
		)
		.replace_config::<HydraDxEthConfig<C, BE>>()
		.into_rpc(),
	)?;

	io.merge(
		EthFilter::new(
			client.clone(),
			frontier_backend,
			graph,
			filter_pool,
			500_usize, // max stored filters
			max_past_logs,
			block_data_cache,
		)
		.into_rpc(),
	)?;

	io.merge(
		EthPubSub::new(
			pool,
			client.clone(),
			sync,
			subscription_task_executor,
			overrides,
			pubsub_notification_sinks,
		)
		.into_rpc(),
	)?;

	io.merge(
		Net::new(
			client.clone(),
			network,
			// Whether to format the `peer_count` response as Hex (default) or not.
			true,
		)
		.into_rpc(),
	)?;

	io.merge(Web3::new(client).into_rpc())?;

	Ok(io)
}

impl<C, P, A: ChainApi, CT: Clone> Clone for Deps<C, P, A, CT> {
	fn clone(&self) -> Self {
		Self {
			client: self.client.clone(),
			pool: self.pool.clone(),
			graph: self.graph.clone(),
			converter: self.converter.clone(),
			is_authority: self.is_authority,
			enable_dev_signer: self.enable_dev_signer,
			network: self.network.clone(),
			sync: self.sync.clone(),
			frontier_backend: self.frontier_backend.clone(),
			overrides: self.overrides.clone(),
			block_data_cache: self.block_data_cache.clone(),
			filter_pool: self.filter_pool.clone(),
			max_past_logs: self.max_past_logs,
			fee_history_cache: self.fee_history_cache.clone(),
			fee_history_cache_limit: self.fee_history_cache_limit,
			execute_gas_limit_multiplier: self.execute_gas_limit_multiplier,
		}
	}
}
