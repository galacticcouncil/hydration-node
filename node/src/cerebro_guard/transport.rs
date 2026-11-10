//! cerebro's `Transport` over the node: the block under construction (storage read natively from
//! the builder's overlay, runtime calls in a rolled-back nested transaction) and the client at an
//! imported block.

use super::events::{RawRead, RawStorage};
use cerebro_core::BlockHash;
use cerebro_hydration::balance::call_output;
use cerebro_hydration::Transport;
use fp_rpc::EthereumRuntimeRPCApi;
use hydradx_runtime::opaque::Block;
use sc_block_builder::Isolated;
use sc_client_api::{Backend, StorageProvider};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::{H160, H256, U256};
use sp_storage::StorageKey;
use std::marker::PhantomData;

/// Gas for read-only EVM calls (aToken `balanceOf`, oracle adapter).
const VIEW_GAS: u64 = 10_000_000;

/// The block under construction. `at` is ignored: there is only the current state.
pub struct ApiTransport<'a, 'b, Api> {
	iso: &'a Isolated<'b, Block, Api>,
	parent: H256,
	number: u32,
}

impl<'a, 'b, Api> ApiTransport<'a, 'b, Api> {
	pub fn new(iso: &'a Isolated<'b, Block, Api>, parent: H256, number: u32) -> Self {
		Self { iso, parent, number }
	}
}

impl<Api> RawStorage for ApiTransport<'_, '_, Api>
where
	Api: ApiExt<Block>,
{
	fn read(&self, reads: Vec<RawRead>) -> Result<Vec<Option<Vec<u8>>>, String> {
		reads
			.into_iter()
			.map(|(key, offset, max_len)| {
				self.iso
					.storage(&key, |v| v.map(|v| slice(v, offset, max_len).to_vec()))
					.map_err(|e| format!("storage read: {e}"))
			})
			.collect()
	}
}

/// The builder's per-transaction changes in cerebro's shape. Appended keys (event lists) carry no
/// earlier value and no balance, so they are left out.
pub fn changes(changes: sp_api::TransactionChanges) -> Vec<cerebro_hydration::state::Change> {
	changes
		.changes
		.into_iter()
		.map(|c| cerebro_hydration::state::Change {
			key: c.key,
			pre: c.pre,
			post: c.post,
		})
		.collect()
}

fn slice(v: &[u8], offset: u32, max_len: u32) -> &[u8] {
	let start = (offset as usize).min(v.len());
	&v[start..start.saturating_add(max_len as usize).min(v.len())]
}

impl<Api> Transport for ApiTransport<'_, '_, Api>
where
	Api: ApiExt<Block> + EthereumRuntimeRPCApi<Block>,
{
	fn storage_many(&self, keys: &[Vec<u8>], _at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		RawStorage::read(self, keys.iter().map(|k| (k.clone(), 0, u32::MAX)).collect())
	}

	fn evm_view(&self, to: [u8; 20], data: &[u8], _at: BlockHash) -> Result<Vec<u8>, String> {
		let info = self
			.iso
			.read(|api| {
				api.call(
					self.parent,
					H160::zero(),
					H160(to),
					data.to_vec(),
					U256::zero(),
					U256::from(VIEW_GAS),
					None,
					None,
					None,
					true,
					None,
					None,
				)
			})
			.map_err(|e| format!("EthereumRuntimeRPCApi_call: {e}"))?
			.map_err(|e| format!("EthereumRuntimeRPCApi_call: {e:?}"))?;
		call_output(info)
	}

	fn storage_keys(&self, _prefix: &[u8], _at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
		Err("storage_keys is not available while building a block".into())
	}

	fn block_number(&self, _at: BlockHash) -> Result<u32, String> {
		Ok(self.number)
	}
}

/// An imported block, through the client. Used to refresh the shared caches.
pub struct ClientTransport<'a, C, B> {
	client: &'a C,
	_backend: PhantomData<B>,
}

impl<'a, C, B> ClientTransport<'a, C, B> {
	pub fn new(client: &'a C) -> Self {
		Self {
			client,
			_backend: PhantomData,
		}
	}
}

impl<C, B> Transport for ClientTransport<'_, C, B>
where
	B: Backend<Block>,
	C: StorageProvider<Block, B> + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
{
	fn storage_many(&self, keys: &[Vec<u8>], at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		keys.iter()
			.map(|k| {
				self.client
					.storage(H256(at), &StorageKey(k.clone()))
					.map(|v| v.map(|d| d.0))
					.map_err(|e| e.to_string())
			})
			.collect()
	}

	fn evm_view(&self, to: [u8; 20], data: &[u8], at: BlockHash) -> Result<Vec<u8>, String> {
		let info = self
			.client
			.runtime_api()
			.call(
				H256(at),
				H160::zero(),
				H160(to),
				data.to_vec(),
				U256::zero(),
				U256::from(VIEW_GAS),
				None,
				None,
				None,
				true,
				None,
				None,
			)
			.map_err(|e| format!("EthereumRuntimeRPCApi_call: {e}"))?
			.map_err(|e| format!("EthereumRuntimeRPCApi_call: {e:?}"))?;
		call_output(info)
	}

	fn storage_keys(&self, prefix: &[u8], at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
		Ok(self
			.client
			.storage_keys(H256(at), Some(&StorageKey(prefix.to_vec())), None)
			.map_err(|e| e.to_string())?
			.map(|k| k.0)
			.collect())
	}

	fn block_number(&self, at: BlockHash) -> Result<u32, String> {
		self.client
			.number(H256(at))
			.map_err(|e| e.to_string())?
			.ok_or_else(|| "unknown block".into())
	}
}

#[cfg(test)]
mod tests {
	use super::slice;

	#[test]
	fn slice_should_cap_at_max_len_when_value_is_longer() {
		assert_eq!(slice(&[1, 2, 3, 4, 5], 1, 2), &[2, 3]);
	}

	#[test]
	fn slice_should_return_tail_when_max_len_exceeds_value() {
		assert_eq!(slice(&[1, 2, 3], 1, u32::MAX), &[2, 3]);
	}

	#[test]
	fn slice_should_return_empty_when_offset_is_past_end() {
		assert_eq!(slice(&[1, 2, 3], 9, u32::MAX), &[] as &[u8]);
	}
}
