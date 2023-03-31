//! Client abstractions

#![allow(clippy::upper_case_acronyms)]

use crate::service::{FullBackend, FullClient, HydraDXExecutorDispatch, TestingHydraDXExecutorDispatch};
use common_runtime::{AccountId, Balance, Block, BlockNumber, Hash, Header, Index};
use sc_client_api::{Backend as BackendT, BlockchainEvents, KeyIterator};
use sp_api::{CallApiAt, NumberFor, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus::BlockStatus;
use sp_runtime::{
	generic::SignedBlock,
	traits::{BlakeTwo256, Block as BlockT},
	Justifications,
};
use sp_storage::{ChildInfo, StorageData, StorageKey};
use std::sync::Arc;

/// A set of APIs that HydraDX-like runtimes must implement.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

impl<Api> RuntimeApiCollection for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>,
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
	HydraDX(Arc<FullClient<hydradx_runtime::RuntimeApi, HydraDXExecutorDispatch>>),
	TestingHydraDX(Arc<FullClient<testing_hydradx_runtime::RuntimeApi, TestingHydraDXExecutorDispatch>>),
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
	fn block_body(
		&self,
		hash: <Block as BlockT>::Hash,
	) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
		match self {
			Self::HydraDX(client) => client.block_body(hash),
			Self::TestingHydraDX(client) => client.block_body(hash),
		}
	}

	fn block(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
		match self {
			Self::HydraDX(client) => client.block(hash),
			Self::TestingHydraDX(client) => client.block(hash),
		}
	}

	fn block_status(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<BlockStatus> {
		match self {
			Self::HydraDX(client) => client.block_status(hash),
			Self::TestingHydraDX(client) => client.block_status(hash),
		}
	}

	fn justifications(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Justifications>> {
		match self {
			Self::HydraDX(client) => client.justifications(hash),
			Self::TestingHydraDX(client) => client.justifications(hash),
		}
	}

	fn block_hash(&self, number: NumberFor<Block>) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.block_hash(number),
			Self::TestingHydraDX(client) => client.block_hash(number),
		}
	}

	fn indexed_transaction(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Vec<u8>>> {
		match self {
			Self::HydraDX(client) => client.indexed_transaction(hash),
			Self::TestingHydraDX(client) => client.indexed_transaction(hash),
		}
	}

	fn block_indexed_body(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
		match self {
			Self::HydraDX(client) => client.block_indexed_body(hash),
			Self::TestingHydraDX(client) => client.block_indexed_body(hash),
		}
	}

	fn requires_full_sync(&self) -> bool {
		match self {
			Self::HydraDX(client) => client.requires_full_sync(),
			Self::TestingHydraDX(client) => client.requires_full_sync(),
		}
	}
}

impl sc_client_api::StorageProvider<Block, FullBackend> for Client {
	fn storage(&self, hash: <Block as BlockT>::Hash, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			Self::HydraDX(client) => client.storage(hash, key),
			Self::TestingHydraDX(client) => client.storage(hash, key),
		}
	}

	fn storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			Self::HydraDX(client) => client.storage_keys(hash, key_prefix),
			Self::TestingHydraDX(client) => client.storage_keys(hash, key_prefix),
		}
	}

	fn storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.storage_hash(hash, key),
			Self::TestingHydraDX(client) => client.storage_hash(hash, key),
		}
	}

	fn storage_pairs(
		&self,
		hash: <Block as BlockT>::Hash,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<(StorageKey, StorageData)>> {
		match self {
			Self::HydraDX(client) => client.storage_pairs(hash, key_prefix),
			Self::TestingHydraDX(client) => client.storage_pairs(hash, key_prefix),
		}
	}

	fn storage_keys_iter<'a>(
		&self,
		hash: <Block as BlockT>::Hash,
		prefix: Option<&'a StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<KeyIterator<<FullBackend as sc_client_api::Backend<Block>>::State, Block>> {
		match self {
			Self::HydraDX(client) => client.storage_keys_iter(hash, prefix, start_key),
			Self::TestingHydraDX(client) => client.storage_keys_iter(hash, prefix, start_key),
		}
	}

	fn child_storage(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			Self::HydraDX(client) => client.child_storage(hash, child_info, key),
			Self::TestingHydraDX(client) => client.child_storage(hash, child_info, key),
		}
	}

	fn child_storage_keys(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			Self::HydraDX(client) => client.child_storage_keys(hash, child_info, key_prefix),
			Self::TestingHydraDX(client) => client.child_storage_keys(hash, child_info, key_prefix),
		}
	}

	fn child_storage_keys_iter<'a>(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: ChildInfo,
		prefix: Option<&'a StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<KeyIterator<<FullBackend as sc_client_api::Backend<Block>>::State, Block>> {
		match self {
			Self::HydraDX(client) => client.child_storage_keys_iter(hash, child_info, prefix, start_key),
			Self::TestingHydraDX(client) => client.child_storage_keys_iter(hash, child_info, prefix, start_key),
		}
	}

	fn child_storage_hash(
		&self,
		hash: <Block as BlockT>::Hash,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			Self::HydraDX(client) => client.child_storage_hash(hash, child_info, key),
			Self::TestingHydraDX(client) => client.child_storage_hash(hash, child_info, key),
		}
	}
}

impl sp_blockchain::HeaderBackend<Block> for Client {
	fn header(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Header>> {
		match self {
			Self::HydraDX(client) => client.header(hash),
			Self::TestingHydraDX(client) => client.header(hash),
		}
	}

	fn info(&self) -> sp_blockchain::Info<Block> {
		match self {
			Self::HydraDX(client) => client.info(),
			Self::TestingHydraDX(client) => client.info(),
		}
	}

	fn status(&self, hash: <Block as BlockT>::Hash) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
		match self {
			Self::HydraDX(client) => client.status(hash),
			Self::TestingHydraDX(client) => client.status(hash),
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
