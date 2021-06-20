//! HydraDX Client abstractions

#![allow(clippy::upper_case_acronyms)]

pub use crate::service::{FullBackend, FullClient, HydraExecutor, LightBackend, LightClient, TestingHydraExecutor};
use common_runtime::{AccountId, AssetId, Balance, BlockNumber, Hash, Index, Block, Header};
use sc_client_api::{Backend as BackendT, BlockchainEvents, KeyIterator};
use sp_api::{CallApiAt, NumberFor, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus::BlockStatus;
use sp_runtime::{
	generic::{BlockId, SignedBlock},
	traits::{BlakeTwo256, Block as BlockT},
	Justifications,
};
use sp_storage::{ChildInfo, PrefixedStorageKey, StorageData, StorageKey};
use std::sync::Arc;

/// A set of APIs that HydraDX-like runtimes must implement.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_consensus_babe::BabeApi<Block>
	+ pallet_grandpa::fg_primitives::GrandpaApi<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ sp_authority_discovery::AuthorityDiscoveryApi<Block>
	+ pallet_xyk_rpc_runtime_api::XYKApi<Block, AccountId, AssetId, Balance>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

impl<Api> RuntimeApiCollection for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_consensus_babe::BabeApi<Block>
		+ pallet_grandpa::fg_primitives::GrandpaApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ sp_authority_discovery::AuthorityDiscoveryApi<Block>
		+ pallet_xyk_rpc_runtime_api::XYKApi<Block, AccountId, AssetId, Balance>,
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

/// Trait that abstracts over all available client implementations.
///
/// For a concrete type there exists [`Client`].
pub trait AbstractClient<Block, Backend>:
	BlockchainEvents<Block>
	+ Sized
	+ Send
	+ Sync
	+ ProvideRuntimeApi<Block>
	+ HeaderBackend<Block>
	+ CallApiAt<Block, StateBackend = Backend::State>
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Self::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

impl<Block, Backend, Client> AbstractClient<Block, Backend> for Client
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Client: BlockchainEvents<Block>
		+ ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ Sized
		+ Send
		+ Sync
		+ CallApiAt<Block, StateBackend = Backend::State>,
	Client::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

/// Execute something with the client instance.
///
/// As there exist multiple chains inside HydraDX, like HydraDX itself and testing runtime,
/// there can exist different kinds of client types. As these client types differ in the generics
/// that are being used, we can not easily return them from a function. For returning them from a
/// function there exists [`Client`]. However, the problem on how to use this client instance still
/// exists. This trait "solves" it in a dirty way. It requires a type to implement this trait and
/// than the [`execute_with_client`](ExecuteWithClient::execute_with_client) function can be called
/// with any possible client instance.
///
/// In a perfect world, we could make a closure work in this way.
pub trait ExecuteWithClient {
	/// The return type when calling this instance.
	type Output;

	/// Execute whatever should be executed with the given client instance.
	fn execute_with_client<Client, Api, Backend>(self, client: Arc<Client>) -> Self::Output
	where
		<Api as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
		Backend: sc_client_api::Backend<Block> + 'static,
		Backend::State: sp_api::StateBackend<BlakeTwo256>,
		Api: RuntimeApiCollection<StateBackend = Backend::State>,
		Client: AbstractClient<Block, Backend, Api = Api> + 'static;
}

/// A handle to a HydraDX client instance.
///
/// The HydraDX service supports multiple different runtimes (HydraDX itself or testing runtime). As each runtime has a
/// specialized client, we need to hide them behind a trait. This is this trait.
///
/// When wanting to work with the inner client, you need to use `execute_with`.
///
/// See [`ExecuteWithClient`](trait.ExecuteWithClient.html) for more information.
pub trait ClientHandle {
	/// Execute the given something with the client.
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output;
}

/// A client instance of HydraDX.
///
/// See [`ExecuteWithClient`] for more information.
#[derive(Clone)]
pub enum Client {
	HydraDX(Arc<FullClient<hydra_dx_runtime::RuntimeApi, HydraExecutor>>),
	TestingHydraDX(Arc<FullClient<testing_hydra_dx_runtime::RuntimeApi, TestingHydraExecutor>>),
}

impl ClientHandle for Client {
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output {
		match self {
			Self::HydraDX(client) => T::execute_with_client::<_, _, FullBackend>(t, client.clone()),
			Self::TestingHydraDX(client) => T::execute_with_client::<_, _, FullBackend>(t, client.clone()),
		}
	}
}

impl sc_client_api::UsageProvider<Block> for Client {
	fn usage_info(&self) -> sc_client_api::ClientInfo<Block> {
		match self {
			Self::HydraDX(client) => client.usage_info(),
			Self::TestingHydraDX(client) => client.usage_info(),
		}
	}
}

impl sc_client_api::BlockBackend<Block> for Client {
	fn block_body(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
		match self {
			Self::HydraDX(client) => client.block_body(id),
			Self::TestingHydraDX(client) => client.block_body(id),
		}
	}

	fn block(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
		match self {
			Self::HydraDX(client) => client.block(id),
			Self::TestingHydraDX(client) => client.block(id),
		}
	}

	fn block_status(&self, id: &BlockId<Block>) -> sp_blockchain::Result<BlockStatus> {
		match self {
			Self::HydraDX(client) => client.block_status(id),
			Self::TestingHydraDX(client) => client.block_status(id),
		}
	}

	fn justifications(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Justifications>> {
		match self {
			Self::HydraDX(client) => client.justifications(id),
			Self::TestingHydraDX(client) => client.justifications(id),
		}
	}

	fn block_hash(&self, number: NumberFor<Block>) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.block_hash(number),
			Self::TestingHydraDX(client) => client.block_hash(number),
		}
	}

	fn indexed_transaction(&self, id: &<Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Vec<u8>>> {
		match self {
			Self::HydraDX(client) => client.indexed_transaction(id),
			Self::TestingHydraDX(client) => client.indexed_transaction(id),
		}
	}
}

impl sc_client_api::StorageProvider<Block, FullBackend> for Client {
	fn storage(&self, id: &BlockId<Block>, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			Self::HydraDX(client) => client.storage(id, key),
			Self::TestingHydraDX(client) => client.storage(id, key),
		}
	}

	fn storage_keys(&self, id: &BlockId<Block>, key_prefix: &StorageKey) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			Self::HydraDX(client) => client.storage_keys(id, key_prefix),
			Self::TestingHydraDX(client) => client.storage_keys(id, key_prefix),
		}
	}

	fn storage_hash(
		&self,
		id: &BlockId<Block>,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.storage_hash(id, key),
			Self::TestingHydraDX(client) => client.storage_hash(id, key),
		}
	}

	fn storage_pairs(
		&self,
		id: &BlockId<Block>,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<(StorageKey, StorageData)>> {
		match self {
			Self::HydraDX(client) => client.storage_pairs(id, key_prefix),
			Self::TestingHydraDX(client) => client.storage_pairs(id, key_prefix),
		}
	}

	fn storage_keys_iter<'a>(
		&self,
		id: &BlockId<Block>,
		prefix: Option<&'a StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<KeyIterator<'a, <FullBackend as sc_client_api::Backend<Block>>::State, Block>> {
		match self {
			Self::HydraDX(client) => client.storage_keys_iter(id, prefix, start_key),
			Self::TestingHydraDX(client) => client.storage_keys_iter(id, prefix, start_key),
		}
	}

	fn child_storage(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			Self::HydraDX(client) => client.child_storage(id, child_info, key),
			Self::TestingHydraDX(client) => client.child_storage(id, child_info, key),
		}
	}

	fn child_storage_keys(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			Self::HydraDX(client) => client.child_storage_keys(id, child_info, key_prefix),
			Self::TestingHydraDX(client) => client.child_storage_keys(id, child_info, key_prefix),
		}
	}

	fn child_storage_hash(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.child_storage_hash(id, child_info, key),
			Self::TestingHydraDX(client) => client.child_storage_hash(id, child_info, key),
		}
	}

	fn max_key_changes_range(
		&self,
		first: NumberFor<Block>,
		last: BlockId<Block>,
	) -> sp_blockchain::Result<Option<(NumberFor<Block>, BlockId<Block>)>> {
		match self {
			Self::HydraDX(client) => client.max_key_changes_range(first, last),
			Self::TestingHydraDX(client) => client.max_key_changes_range(first, last),
		}
	}

	fn key_changes(
		&self,
		first: NumberFor<Block>,
		last: BlockId<Block>,
		storage_key: Option<&PrefixedStorageKey>,
		key: &StorageKey,
	) -> sp_blockchain::Result<Vec<(NumberFor<Block>, u32)>> {
		match self {
			Self::HydraDX(client) => client.key_changes(first, last, storage_key, key),
			Self::TestingHydraDX(client) => client.key_changes(first, last, storage_key, key),
		}
	}
}

impl sp_blockchain::HeaderBackend<Block> for Client {
	fn header(&self, id: BlockId<Block>) -> sp_blockchain::Result<Option<Header>> {
		match self {
			Self::HydraDX(client) => client.header(&id),
			Self::TestingHydraDX(client) => client.header(&id),
		}
	}

	fn info(&self) -> sp_blockchain::Info<Block> {
		match self {
			Self::HydraDX(client) => client.info(),
			Self::TestingHydraDX(client) => client.info(),
		}
	}

	fn status(&self, id: BlockId<Block>) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
		match self {
			Self::HydraDX(client) => client.status(id),
			Self::TestingHydraDX(client) => client.status(id),
		}
	}

	fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
		match self {
			Self::HydraDX(client) => client.number(hash),
			Self::TestingHydraDX(client) => client.number(hash),
		}
	}

	fn hash(&self, number: BlockNumber) -> sp_blockchain::Result<Option<Hash>> {
		match self {
			Self::HydraDX(client) => client.hash(number),
			Self::TestingHydraDX(client) => client.hash(number),
		}
	}
}
