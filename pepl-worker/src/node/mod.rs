//! Node-mode implementations of worker traits.
//!
//! These adapters wrap the Substrate client, transaction pool, and RPC APIs
//! to provide the trait implementations needed by the generic worker loop.

pub mod block_source;
pub mod mempool;
pub mod rpc;
pub mod runner;
pub mod tx_submitter;

pub use block_source::NodeBlockSource;
pub use mempool::NodeMempoolMonitor;
pub use runner::{LiquidationTask, LiquidationWorkerConfig};
pub use tx_submitter::{NodeTxSubmitter, ReportOnlySubmitter};

use crate::config;
use crate::traits::*;
use codec::Decode;
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use fp_self_contained::SelfContainedCall;
use frame_support::BoundedVec;
use hydradx_runtime::{evm::precompiles::erc20_mapping::Erc20MappingApi, OriginCaller, RuntimeCall, RuntimeEvent};
use liquidation_worker_support::*;
use pallet_currencies_rpc_runtime_api::CurrenciesApi;
use pallet_ethereum::Transaction;
use primitives::{AccountId, EvmAddress};
use sc_client_api::{Backend, StorageKey, StorageProvider};
use sp_api::{ApiError, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;
use xcm_runtime_apis::dry_run::{CallDryRunEffects, DryRunApi};

/// Known event topic hashes.
mod events {
	use hex_literal::hex;
	use sp_core::H256;

	pub const BORROW: H256 = H256(hex!("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0"));
	pub const COLLATERAL_CONFIGURATION_CHANGED: H256 =
		H256(hex!("637febbda9275aea2e85c0ff690444c8d87eb2e8339bbede9715abcc89cb0995"));
}

/// Wraps a Substrate client as a `RuntimeApiProvider`.
///
/// Creates a fresh `runtime_api()` instance for every call to avoid the
/// `UsingSameInstanceForDifferentBlocks` error that occurs when reusing a
/// single runtime API instance across multiple block hashes.
#[derive(Clone)]
pub struct ApiProvider<C> {
	pub client: C,
}

impl<C> ApiProvider<C> {
	pub fn new(client: C) -> Self {
		Self { client }
	}
}

impl<Block, C> RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> for ApiProvider<C>
where
	Block: BlockT,
	C: std::ops::Deref,
	C::Target: ProvideRuntimeApi<Block>,
	<C::Target as ProvideRuntimeApi<Block>>::Api: EthereumRuntimeRPCApi<Block>
		+ Erc20MappingApi<Block>
		+ DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller>
		+ CurrenciesApi<Block, AssetId, AccountId, Balance>,
{
	fn current_timestamp(&self, hash: Block::Hash) -> Option<u64> {
		let api = self.client.runtime_api();
		let block = api.current_block(hash).ok()??;
		block.header.timestamp.checked_div(1_000)
	}

	fn call(
		&self,
		hash: Block::Hash,
		caller: EvmAddress,
		contract_address: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<Result<fp_evm::ExecutionInfoV2<Vec<u8>>, sp_runtime::DispatchError>, ApiError> {
		let api = self.client.runtime_api();
		api.call(
			hash,
			caller,
			contract_address,
			data,
			U256::zero(),
			gas_limit,
			None,
			None,
			None,
			true,
			None,
			None,
		)
	}

	fn address_to_asset(&self, hash: Block::Hash, address: AssetAddress) -> Result<Option<AssetId>, ApiError> {
		let api = self.client.runtime_api();
		api.address_to_asset(hash, address)
	}

	fn dry_run_call(
		&self,
		hash: Block::Hash,
		origin: OriginCaller,
		call: RuntimeCall,
	) -> Result<Result<CallDryRunEffects<RuntimeEvent>, xcm_runtime_apis::dry_run::Error>, ApiError> {
		let api = self.client.runtime_api();
		api.dry_run_call(hash, origin, call, 5)
	}

	fn minimum_balance(&self, hash: Block::Hash, asset_id: AssetId) -> Result<Balance, ApiError> {
		let api = self.client.runtime_api();
		api.minimum_balance(hash, asset_id)
	}
}

/// Wraps a Substrate RuntimeApi as a `DryRunner` for the worker.
pub struct NodeDryRunner<Block, C> {
	pub client: Arc<C>,
	pub _phantom: std::marker::PhantomData<Block>,
}

impl<Block, C> crate::traits::DryRunner for NodeDryRunner<Block, C>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block>,
	C::Api: DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller>,
	C: HeaderBackend<Block>,
{
	fn dry_run(&self, tx: &LiquidationTx, _block_hash: [u8; 32]) -> bool {
		let hash = self.client.info().best_hash;
		let liquidation_call = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
			collateral_asset: tx.collateral_asset,
			debt_asset: tx.debt_asset,
			user: tx.user,
			debt_to_cover: tx.debt_to_cover,
			route: BoundedVec::new(),
		});

		let dry_run_result = self
			.client
			.runtime_api()
			.dry_run_call(hash, hydradx_runtime::RuntimeOrigin::none().caller, liquidation_call, 5);

		match dry_run_result {
			Ok(Ok(call_result)) => call_result.execution_result.is_ok(),
			_ => false,
		}
	}
}

/// Extract events from a block (Borrow, CollateralConfigurationChanged, Liquidated).
pub fn filter_events(
	events: Vec<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>,
) -> (Vec<UserAddress>, Vec<AssetAddress>, Vec<UserAddress>) {
	let mut new_borrows = Vec::new();
	let mut new_assets = Vec::new();
	let mut liquidated_users = Vec::new();

	for event in events {
		match &event.event {
			RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => {
				if log.address == config::DEFAULT_BORROW_CALL_ADDRESS
					&& log.topics.first() == Some(&events::BORROW)
				{
					if let Some(&borrower) = log.topics.get(2) {
						new_borrows.push(UserAddress::from(borrower));
					}
				} else if log.address == config::DEFAULT_POOL_CONFIGURATOR_ADDRESS
					&& log.topics.first() == Some(&events::COLLATERAL_CONFIGURATION_CHANGED)
				{
					if let Some(&asset) = log.topics.get(1) {
						new_assets.push(AssetAddress::from(asset));
					}
				}
			}
			RuntimeEvent::Liquidation(pallet_liquidation::Event::Liquidated { user, .. }) => {
				liquidated_users.push(*user);
			}
			_ => {}
		}
	}

	(new_borrows, new_assets, liquidated_users)
}

/// Get events from a block by reading the System::events storage.
pub fn get_events<Block, C, BE>(
	client: Arc<C>,
	block_hash: Block::Hash,
) -> Result<Vec<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>, ()>
where
	Block: BlockT,
	C: StorageProvider<Block, BE>,
	BE: Backend<Block> + 'static,
{
	let events_key = StorageKey(
		hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec(),
	);

	if let Ok(Some(encoded_events)) = client.storage(block_hash, &events_key) {
		if let Ok(events) = Vec::<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>::decode(
			&mut encoded_events.0.as_slice(),
		) {
			return Ok(events);
		}
	}

	Err(())
}

/// Check if a transaction is a valid DIA oracle update.
pub fn verify_oracle_update_transaction(
	extrinsic: &sp_runtime::generic::UncheckedExtrinsic<
		hydradx_runtime::Address,
		RuntimeCall,
		hydradx_runtime::Signature,
		hydradx_runtime::SignedExtra,
	>,
	allowed_signers: &[EvmAddress],
	allowed_oracle_call_addresses: &[EvmAddress],
) -> Option<Transaction> {
	if let RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = extrinsic.function.clone() {
		let action = match transaction.clone() {
			Transaction::Legacy(t) => t.action,
			Transaction::EIP2930(t) => t.action,
			Transaction::EIP1559(t) => t.action,
			Transaction::EIP7702(_) => return None,
		};

		if let pallet_ethereum::TransactionAction::Call(call_address) = action {
			if allowed_oracle_call_addresses.contains(&call_address) {
				if let Some(Ok(signer)) = extrinsic.function.check_self_contained() {
					if allowed_signers.contains(&signer) {
						return Some(transaction);
					}
				}
			}
		}
	}

	None
}

/// Get the raw input bytes from an Ethereum transaction.
pub fn get_transaction_input(transaction: &Transaction) -> Option<&[u8]> {
	let input = match transaction {
		Transaction::Legacy(t) => &t.input,
		Transaction::EIP2930(t) => &t.input,
		Transaction::EIP1559(t) => &t.input,
		Transaction::EIP7702(_) => return None,
	};
	Some(input.as_ref())
}

/// Provides RPC-visible state data about the worker.
pub struct LiquidationTaskData {
	pub borrowers_list: Arc<Mutex<Vec<Borrower>>>,
	pub max_transactions: Arc<Mutex<usize>>,
	pub thread_pool: Arc<Mutex<ThreadPool>>,
}

impl Default for LiquidationTaskData {
	fn default() -> Self {
		Self::new()
	}
}

impl LiquidationTaskData {
	pub fn new() -> Self {
		Self {
			borrowers_list: Default::default(),
			max_transactions: Default::default(),
			thread_pool: Arc::new(Mutex::new(ThreadPool::with_name(
				"liquidation-worker".into(),
				num_cpus::get(),
			))),
		}
	}
}
