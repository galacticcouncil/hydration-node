// This file is part of Basilisk-node.

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

#![allow(clippy::upper_case_acronyms)]

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use pallet_xyk_rpc_runtime_api::BalanceInfo;
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};
use std::sync::Arc;

pub use self::gen_client::Client as XYKClient;
pub use pallet_xyk_rpc_runtime_api::XYKApi as XYKRuntimeApi;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct BalanceRequest<Balance> {
	amount: Balance,
}

#[rpc]
pub trait XYKApi<BlockHash, AccountId, AssetId, Balance, ResponseType> {
	#[rpc(name = "xyk_getPoolBalances")]
	fn get_pool_balances(&self, pool_address: AccountId, at: Option<BlockHash>) -> Result<Vec<ResponseType>>;

	#[rpc(name = "xyk_getPoolAccount")]
	fn get_pool_id(&self, asset_a: AssetId, asset_b: AssetId) -> Result<AccountId>;
}

/// A struct that implements the [`XYKApi`].
pub struct XYK<C, B> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<B>,
}

impl<C, B> XYK<C, B> {
	/// Create new `XYK` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		XYK {
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
	XYKApi<<Block as BlockT>::Hash, AccountId, AssetId, Balance, BalanceInfo<AssetId, Balance>> for XYK<C, Block>
where
	Block: BlockT,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: XYKRuntimeApi<Block, AccountId, AssetId, Balance>,
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

	fn get_pool_id(&self, asset_a: AssetId, asset_b: AssetId) -> Result<AccountId> {
		let api = self.client.runtime_api();
		let at = BlockId::hash(self.client.info().best_hash);

		api.get_pool_id(&at, asset_a, asset_b).map_err(|e| RpcError {
			code: ErrorCode::ServerError(Error::RuntimeError.into()),
			message: "Unable to retrieve pool account address.".into(),
			data: Some(format!("{:?}", e).into()),
		})
	}
}
