use std::sync::Arc;

use codec::Codec;
use jsonrpsee::{
	core::{async_trait, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

pub use pallet_staking_runtime_api::StakingApi as StakingRuntimeApi;

#[rpc(client, server)]
pub trait StakingApi<BlockHash, AccountId> {
	#[method(name = "staking_retrieveAccountPoints")]
	fn retrieve_account_points(&self, who: AccountId, at: Option<BlockHash>) -> RpcResult<u32>;
}

/// Provides RPC methods to query staking points.
pub struct Staking<C, B> {
	/// Shared reference to the client.
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> Staking<C, B> {
	/// Creates a new instance of the `Oracle` helper.
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

pub enum Error {
	RuntimeError,
}

impl From<Error> for i32 {
	fn from(e: Error) -> i32 {
		match e {
			Error::RuntimeError => 1,
		}
	}
}

#[async_trait]
impl<C, Block, AccountId> StakingApiServer<<Block as BlockT>::Hash, AccountId> for Staking<C, Block>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
	C::Api: StakingRuntimeApi<Block, AccountId>,
	AccountId: Codec + MaybeFromStr + MaybeDisplay,
{
	fn retrieve_account_points(&self, who: AccountId, at: Option<<Block as BlockT>::Hash>) -> RpcResult<u32> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

		api.retrieve_account_points(&at, who).map_err(|e| {
			CallError::Custom(ErrorObject::owned(
				Error::RuntimeError.into(),
				"Unable to retrieve user points.",
				Some(e.to_string()),
			))
			.into()
		})
	}
}
