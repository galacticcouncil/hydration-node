use codec::{Decode, Encode};
use jsonrpsee::{
	core::client::{Client, ClientT},
	rpc_params,
	types::ParamsSer,
	ws_client::{WsClient, WsClientBuilder},
};
use serde::de::DeserializeOwned;
use sp_runtime::{generic::SignedBlock, traits::Block as BlockT, traits::NumberFor};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

#[allow(clippy::enum_variant_names)]
enum RpcCall {
	GetHeader,
	GetFinalizedHead,
	GetBlock,
	GetBlockHash,
}

impl RpcCall {
	fn as_str(&self) -> &'static str {
		match self {
			RpcCall::GetHeader => "chain_getHeader",
			RpcCall::GetFinalizedHead => "chain_getFinalizedHead",
			RpcCall::GetBlock => "chain_getBlock",
			RpcCall::GetBlockHash => "chain_getBlockHash",
		}
	}
}

/// General purpose method for making RPC calls.
async fn make_request<'a, T: DeserializeOwned>(
	client: &Arc<Client>,
	call: RpcCall,
	params: Option<ParamsSer<'a>>,
) -> Result<T, String> {
	client
		.request::<T>(call.as_str(), params)
		.await
		.map_err(|e| format!("{} request failed: {:?}", call.as_str(), e))
}

/// Simple RPC service that is capable of keeping the connection.
///
/// Service will connect to `uri` for the first time already during initialization.
pub struct RpcService {
	uri: String,
}

impl RpcService {
	/// Creates a new RPC service.
	pub async fn new<S: AsRef<str>>(uri: S) -> Result<Self, String> {
		Ok(Self {
			uri: uri.as_ref().to_string(),
		})
	}

	/// Returns the address at which requests are sent.
	pub fn uri(&self) -> String {
		self.uri.clone()
	}

	/// Build a websocket client that connects to `self.uri`.
	async fn build_client<S: AsRef<str>>(uri: S) -> Result<WsClient, String> {
		WsClientBuilder::default()
			.max_request_body_size(u32::MAX)
			.build(uri)
			.await
			.map_err(|e| format!("`WsClientBuilder` failed to build: {e:?}"))
	}

	/// Generic method for making RPC requests.
	async fn make_request<'a, T: DeserializeOwned>(
		&self,
		call: RpcCall,
		params: Option<ParamsSer<'a>>,
	) -> Result<T, String> {
		let client = Arc::new(Self::build_client(&self.uri).await?);
		make_request(&client, call, params).await
	}

	/// Get the header of the block identified by `at`.
	pub async fn get_header<Block>(&self, at: Block::Hash) -> Result<Block::Header, String>
	where
		Block: BlockT,
		Block::Header: DeserializeOwned,
	{
		self.make_request(RpcCall::GetHeader, rpc_params!(at)).await
	}

	/// Get the finalized head.
	pub async fn get_finalized_head<Block: BlockT>(&self) -> Result<Block::Hash, String> {
		self.make_request(RpcCall::GetFinalizedHead, None).await
	}

	/// Get the signed block identified by `at`.
	pub async fn get_block<Block: BlockT + DeserializeOwned>(&self, at: Block::Hash) -> Result<Block, String> {
		Ok(self
			.make_request::<SignedBlock<Block>>(RpcCall::GetBlock, rpc_params!(at))
			.await?
			.block)
	}

	/// Get the hash of a block.
	pub async fn get_block_hash<Block: BlockT + DeserializeOwned>(
		&self,
		at: NumberFor<Block>,
	) -> Result<Block::Hash, String> {
		self.make_request(RpcCall::GetBlockHash, rpc_params!(at)).await
	}
}

pub fn save_blocks_snapshot<Block: Encode>(data: &Vec<Block>, path: &Path) -> Result<(), &'static str> {
	let mut path = path.to_path_buf();
	let encoded = data.encode();
	path.set_extension("blocks");
	fs::write(path, encoded).map_err(|_| "fs::write failed.")?;
	Ok(())
}

pub fn load_blocks_snapshot<Block: Decode>(path: &Path) -> Result<Vec<Block>, &'static str> {
	let mut path = path.to_path_buf();
	path.set_extension("blocks");
	let bytes = fs::read(path).map_err(|_| "fs::read failed.")?;
	Decode::decode(&mut &*bytes).map_err(|_| "decode failed")
}

pub fn hash_of<Block: BlockT>(hash_str: &str) -> Result<Block::Hash, &'static str>
where
	Block::Hash: FromStr,
	<Block::Hash as FromStr>::Err: std::fmt::Debug,
{
	hash_str
		.parse::<<Block as BlockT>::Hash>()
		.map_err(|_| "Could not parse block hash")
}
