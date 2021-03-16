#![allow(clippy::upper_case_acronyms)]

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use module_amm_rpc_runtime_api::BalanceInfo;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};
use std::sync::Arc;

pub use self::gen_client::Client as AMMClient;
pub use module_amm_rpc_runtime_api::AMMApi as AMMRuntimeApi;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BalanceRequest<Balance> {
	amount: Balance,
}

#[rpc]
pub trait AMMApi<BlockHash, AccountId, AssetId, Balance, ResponseType> {
	#[rpc(name = "amm_getPoolBalances")]
	fn get_pool_balances(&self, pool_address: AccountId, at: Option<BlockHash>) -> Result<Vec<ResponseType>>;
}

/// A struct that implements the [`AMMApi`].
pub struct AMM<C, B> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> AMM<C, B> {
	/// Create new `AMM` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		AMM {
			client,
			_marker: Default::default(),
		}
	}
}

pub enum Error {
	/// The call to runtime failed.
	RuntimeError,
}

impl From<Error> for i64 {
	fn from(e: Error) -> i64 {
		match e {
			Error::RuntimeError => 1,
		}
	}
}

impl<C, Block, AccountId, AssetId, Balance>
	AMMApi<<Block as BlockT>::Hash, AccountId, AssetId, Balance, BalanceInfo<AssetId, Balance>> for AMM<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: AMMRuntimeApi<Block, AccountId, AssetId, Balance>,
	AccountId: Codec,
	AssetId: Codec,
	Balance: Codec + MaybeDisplay + MaybeFromStr,
{
	fn get_pool_balances(
		&self,
		pool_address: AccountId,
		at: Option<<Block as BlockT>::Hash>,
	) -> Result<Vec<BalanceInfo<AssetId, Balance>>> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(at.unwrap_or_else(||
			// If the block hash is not supplied assume the best block.
			self.client.info().best_hash));

		api.get_pool_balances(&at, pool_address).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to retrieve pool balances.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}
}
