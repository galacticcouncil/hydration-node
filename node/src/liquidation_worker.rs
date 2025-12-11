use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use fp_self_contained::SelfContainedCall;
use frame_support::{dispatch::GetDispatchInfo, BoundedVec, __private::sp_tracing::tracing};
use futures::StreamExt;
use hex_literal::hex;
use hydradx_runtime::{
	evm::{precompiles::erc20_mapping::Erc20MappingApi, EvmAddress},
	OriginCaller, RuntimeCall, RuntimeEvent,
};
use hyper::{body::Body, Client, StatusCode};
use hyperv14 as hyper;
pub use liquidation_worker_support::*;
use pallet_currencies_rpc_runtime_api::CurrenciesApi;
use pallet_ethereum::Transaction;
use polkadot_primitives::EncodeAs;
use primitives::AccountId;
use sc_client_api::{Backend, BlockchainEvents, StorageKey, StorageProvider};
use sc_service::SpawnTaskHandle;
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool};
use sp_api::{ApiError, ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::{RuntimeDebug, H160, H256};
use sp_offchain::OffchainWorkerApi;
use sp_runtime::{traits::Header, transaction_validity::TransactionSource, Percent};
use std::{
	cmp::Ordering,
	collections::{HashMap, HashSet},
	marker::PhantomData,
	ops::Deref,
	sync::{mpsc, Arc, Mutex},
};
use threadpool::ThreadPool;
use xcm_runtime_apis::dry_run::{CallDryRunEffects, DryRunApi};

const LOG_TARGET: &str = "liquidation-worker";

// Address of the pool address provider contract.
const PAP_CONTRACT: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));

// Account that calls the runtime API. Needs to have enough of WETH to pay for the runtime API call.
const RUNTIME_API_CALLER: EvmAddress = H160(hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e"));

// Money market address
const BORROW_CALL_ADDRESS: EvmAddress = H160(hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38"));
const POOL_CONFIGURATOR_ADDRESS: EvmAddress = H160(hex!("e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4"));
mod events {
	use super::{hex, H256};

	pub const BORROW: H256 = H256(hex!("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0"));
	pub const COLLATERAL_CONFIGURATION_CHANGED: H256 =
		H256(hex!("637febbda9275aea2e85c0ff690444c8d87eb2e8339bbede9715abcc89cb0995"));
}

// Account that signs the DIA oracle update transactions.
const ORACLE_UPDATE_SIGNER: &[EvmAddress] = &[
	H160(hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e")),
	H160(hex!("ff0c624016c873d359dde711b42a2f475a5a07d3")),
];
// Address of the DIA oracle contract.
const ORACLE_UPDATE_CALL_ADDRESS: &[EvmAddress] = &[
	H160(hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e")),
	H160(hex!("48ae7803cd09c48434e3fc5629f15fb76f0b5ce5")),
];

// Target value of HF we try to liquidate to.
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

// Percentage of the block weight reserved for other transactions.
const WEIGHT_RESERVE: u8 = 10u8;

const OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

type HttpClient = Arc<Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>, Body>>;

/// The configuration for the liquidation worker.
/// By default, the worker is enabled and uses `PAP_CONTRACT`, `RUNTIME_API_CALLER`, `ORACLE_UPDATE_SIGNER`, `ORACLE_UPDATE_CALL_ADDRESS` and `TARGET_HF` values if not specified.
#[derive(Clone, Debug, clap::Parser)]
pub struct LiquidationWorkerConfig {
	/// Enable/disable execution of the liquidation worker.
	#[clap(long)]
	pub liquidation_worker: Option<bool>,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pap_contract: Option<EvmAddress>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub runtime_api_caller: Option<EvmAddress>,

	/// EVM address of the account that signs DIA oracle update.
	#[clap(long)]
	pub oracle_update_signer: Option<Vec<EvmAddress>>,

	/// EVM address of the DIA oracle update call address.
	#[clap(long)]
	pub oracle_update_call_address: Option<Vec<EvmAddress>>,

	/// Target health factor.
	#[clap(long, default_value_t = TARGET_HF)]
	pub target_hf: u128,

	/// URL to fetch initial borrowers data from.
	#[clap(long, default_value = OMNIWATCH_URL)]
	pub omniwatch_url: String,

	/// Percentage of the block weight reserved for other transactions.
	#[clap(long, default_value_t = WEIGHT_RESERVE)]
	pub weight_reserve: u8,
}

struct ApiProvider<C>(C);
impl<Block, C> RuntimeApiProvider<Block, OriginCaller, RuntimeCall, RuntimeEvent> for ApiProvider<&C>
where
	Block: BlockT,
	C: EthereumRuntimeRPCApi<Block>
		+ Erc20MappingApi<Block>
		+ DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller>
		+ CurrenciesApi<Block, AssetId, AccountId, Balance>,
{
	fn current_timestamp(&self, hash: Block::Hash) -> Option<u64> {
		let block = self.0.current_block(hash).ok()??;
		// milliseconds to seconds
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
		self.0.call(
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
		)
	}

	fn address_to_asset(&self, hash: Block::Hash, address: AssetAddress) -> Result<Option<AssetId>, ApiError> {
		self.0.address_to_asset(hash, address)
	}

	fn dry_run_call(
		&self,
		hash: Block::Hash,
		origin: OriginCaller,
		call: RuntimeCall,
	) -> Result<Result<CallDryRunEffects<RuntimeEvent>, xcm_runtime_apis::dry_run::Error>, ApiError> {
		self.0.dry_run_call(hash, origin, call)
	}

	fn minimum_balance(&self, hash: Block::Hash, asset_id: AssetId) -> Result<Balance, ApiError> {
		self.0.minimum_balance(hash, asset_id)
	}
}

type UserAddress = EvmAddress;
type AssetAddress = EvmAddress;
type Price = U256;
pub type AssetSymbol = Vec<u8>;

/// Messages that are sent to the liquidation worker.
#[derive(Clone, RuntimeDebug)]
enum MessageType<B: BlockT> {
	Block(
		sc_client_api::client::BlockImportNotification<B>,
		Vec<UserAddress>,
		Vec<UserAddress>,
	), // (block, new_borrows, liquidated_users_in_previous_block)
	Transaction(TransactionType),
}

/// Messages that are sent to the liquidation worker.
#[derive(Clone, RuntimeDebug)]
enum TransactionType {
	OracleUpdate(Vec<(AssetAddress, Option<Price>)>),
}

/// State of the liquidation worker.
#[derive(Clone, RuntimeDebug)]
enum LiquidationWorkerTask {
	LiquidateAll,
	OracleUpdate(Vec<(AssetAddress, Option<Price>)>),
	WaitForNewTransaction,
}

/// Provides some state of the liquidation worker. Used to provide data for RPC API.
/// The struct uses its own copy of the borrowers list to not hide the existing one behind mutex.
/// `ThreadPool` is used to determine if the worker thread is running. Ideally, we would use
/// `TaskManager` for that, but the implementation of it doesn't provide a public API to get the list
/// of running tasks.
pub struct LiquidationTaskData {
	pub borrowers_list: Arc<Mutex<Vec<Borrower>>>,
	pub max_transactions: Arc<Mutex<usize>>,
	pub thread_pool: Arc<Mutex<ThreadPool>>,
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

pub struct LiquidationTask<B, C, BE, P>(PhantomData<(B, C, BE, P)>);

impl<B, C, BE, P> LiquidationTask<B, C, BE, P>
where
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B>
		+ Erc20MappingApi<B>
		+ DryRunApi<B, RuntimeCall, RuntimeEvent, OriginCaller>
		+ CurrenciesApi<B, AssetId, AccountId, Balance>,
	C: BlockchainEvents<B> + 'static,
	C: HeaderBackend<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
{
	/// Starting point for the liquidation worker.
	/// Executes `on_block_imported` on every block.
	/// The initial list of borrowers is fetched and sorted by the HF.
	pub async fn run(
		client: Arc<C>,
		config: LiquidationWorkerConfig,
		transaction_pool: Arc<P>,
		spawner: SpawnTaskHandle,
		liquidation_task_data: Arc<LiquidationTaskData>,
	) {
		tracing::info!("liquidation-worker: starting");

		let runtime_api = client.runtime_api();
		let mut current_hash = client.info().best_hash;
		let Ok(Some(header)) = client.header(current_hash) else {
			return;
		};

		let has_api_v2 = runtime_api.has_api_with::<dyn OffchainWorkerApi<B>, _>(current_hash, |v| v == 2);
		if let Ok(true) = has_api_v2 {
		} else {
			tracing::error!(
				target: LOG_TARGET,
				"liquidation-worker: Unsupported Offchain Worker API version. Consider turning off offchain workers if they are not part of your runtime.",
			);
			return;
		};

		// Fetch and sort the list of borrowers.
		let Some(borrowers_data) = Self::fetch_borrowers_data(config.omniwatch_url.clone()).await else {
			tracing::error!("liquidation-worker: fetch_borrowers_data failed");
			return;
		};
		let Some(sorted_borrowers) = Self::process_borrowers_data(borrowers_data) else {
			tracing::error!("liquidation-worker: process_borrowers_data failed");
			return;
		};

		// Use `MoneyMarketData` to get the list of reserves.
		let Ok(money_market) = MoneyMarketData::<B, OriginCaller, RuntimeCall, RuntimeEvent>::new::<ApiProvider<&C::Api>>(
			ApiProvider::<&C::Api>(runtime_api.deref()),
			current_hash,
			config.pap_contract.unwrap_or(PAP_CONTRACT),
			config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
		) else {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: MoneyMarketData initialization failed");
			return;
		};

		let mut reserves: HashMap<AssetAddress, AssetSymbol> = money_market
			.reserves()
			.iter()
			.map(|r| (r.asset_address, r.symbol.clone()))
			.collect();

		// Channel used to communicate new blocks and transactions to the liquidation worker thread.
		let (worker_channel_tx, worker_channel_rx) = mpsc::channel();

		let client_c = client.clone();
		let spawner_c = spawner.clone();
		let transaction_pool_c = transaction_pool.clone();
		let config_c = config.clone();

		// Start the liquidation worker thread.
		if let Ok(thread_pool) = liquidation_task_data.clone().thread_pool.lock() {
			thread_pool.execute(move || {
				Self::liquidation_worker(
					client_c,
					config_c,
					transaction_pool_c,
					spawner_c,
					header,
					sorted_borrowers.clone(),
					money_market.clone(),
					worker_channel_rx,
					liquidation_task_data,
				)
			});
		}

		// Accounts that sign the DIA oracle update transactions.
		let allowed_signers = config
			.clone()
			.oracle_update_signer
			.unwrap_or(ORACLE_UPDATE_SIGNER.to_vec());
		// Addresses of the DIA oracle contract.
		let allowed_oracle_call_addresses = config
			.clone()
			.oracle_update_call_address
			.unwrap_or(ORACLE_UPDATE_CALL_ADDRESS.to_vec());

		// Combine block and transaction notifications and process them sequentially.
		let mut block_notification_stream = client.import_notification_stream();
		let mut transaction_notification_stream = transaction_pool.import_notification_stream();

		loop {
			tokio::select! {
				Some(new_block) = block_notification_stream.next() => {
					if new_block.is_new_best {
						let mut borrows: Vec<UserAddress> = Vec::new();
						let mut liquidated_users_in_last_block: Vec<UserAddress> = Vec::new();

						// Get events from the previous block.
						if let Ok(events) = Self::get_events(client.clone(), current_hash) {
							if let Some((new_borrows, new_assets, liquidated_users)) = Self::filter_events(events) {
								for new_asset_address in new_assets {
									let Ok(symbol) = MoneyMarketData::<B, OriginCaller, RuntimeCall, RuntimeEvent>::fetch_asset_symbol::<ApiProvider<&C::Api>>(
										&ApiProvider::<&C::Api>(runtime_api.deref()),
										current_hash,
										&new_asset_address,
										config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
									) else { continue };

									reserves.insert(new_asset_address, symbol);
								}
								borrows.extend(new_borrows);
								liquidated_users_in_last_block.extend(liquidated_users);
							}
						}

						current_hash = new_block.hash;

						// Send the message to the liquidation worker thread.
						let _ = worker_channel_tx.send(MessageType::Block(new_block, borrows, liquidated_users_in_last_block));
					} else {
						tracing::info!(target: LOG_TARGET, "liquidation-worker: Skipping liquidation worker for non-canon block.")
					}
				},
				Some(new_transaction_notification) = transaction_notification_stream.next() => {
					let Some(pool_tx) = transaction_pool.clone().ready_transaction(&new_transaction_notification) else {
						continue
					};

					let opaque_tx_encoded = pool_tx.data().encode();
					let tx = hydradx_runtime::UncheckedExtrinsic::decode(&mut &*opaque_tx_encoded);

					let Ok(transaction) = tx else {
						tracing::error!(target: LOG_TARGET, "liquidation-worker: transaction decoding failed");
						continue
					};

					if let Some(TransactionType::OracleUpdate(oracle_data)) = Self::get_transaction_type(
						transaction.0,
						&allowed_signers,
						&allowed_oracle_call_addresses,
						&reserves,
					) {
						// Send the message to the liquidation worker thread.
						let _ = worker_channel_tx.send(MessageType::Transaction(TransactionType::OracleUpdate(oracle_data)));
					}
				},
				// Streams are "fused" and return `None` when they are exhausted.
				// We don't expect to get here, but if we do, we want to exit the loop and prevent panic.
				else => break
			}
		}
	}

	fn get_transaction_type(
		transaction: sp_runtime::generic::UncheckedExtrinsic<
			hydradx_runtime::Address,
			RuntimeCall,
			hydradx_runtime::Signature,
			hydradx_runtime::SignedExtra,
		>,
		allowed_signers: &[EvmAddress],
		allowed_oracle_call_addresses: &[EvmAddress],
		reserves: &HashMap<AssetAddress, AssetSymbol>,
	) -> Option<TransactionType> {
		if let Some(transaction) =
			Self::verify_oracle_update_transaction(&transaction, allowed_signers, allowed_oracle_call_addresses)
		{
			if let Some(oracle_data) = Self::process_new_oracle_update(&transaction, reserves) {
				return Some(TransactionType::OracleUpdate(oracle_data));
			}
		}

		None
	}

	/// Parse a new DIA oracle update transaction and return the list of oracle updates.
	/// Updates of assets that are not used by the MM are omitted from the returned list.
	fn process_new_oracle_update(
		transaction: &ethereum::TransactionV2,
		reserves: &HashMap<AssetAddress, AssetSymbol>,
	) -> Option<Vec<(AssetAddress, Option<Price>)>> {
		let Some(oracle_data) = parse_oracle_transaction(transaction) else {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: parse_oracle_transaction failed");
			return None;
		};

		// One DIA oracle update transaction can update the price of multiple assets.
		// Create a list of (asset_address, price) pairs from the oracle update.
		let oracle_data: Vec<(AssetAddress, Option<Price>)> = oracle_data
			.iter()
			.filter_map(
				|OracleUpdataData {
				     base_asset_name, price, ..
				 }| {
					if let Ok(base_asset_str) = String::from_utf8(base_asset_name.to_ascii_lowercase()) {
						let asset_reserves: Vec<(&AssetAddress, &AssetSymbol)> = reserves
							.iter()
							.filter(|(_asset_address, symbol)| {
								if let Ok(asset) = String::from_utf8(symbol.to_ascii_lowercase().to_vec()) {
									asset.contains(&base_asset_str)
								} else {
									false
								}
							})
							.collect();

						Some(
							asset_reserves
								.iter()
								.map(|(&asset_address, symbol)| {
									if *symbol == base_asset_name {
										(asset_address, Some(*price))
									} else {
										(asset_address, None)
									}
								})
								.collect::<Vec<_>>(),
						)
					} else {
						None
					}
				},
			)
			.flatten()
			.collect::<Vec<_>>();

		// Skip the execution if assets in the oracle update are not in the money market.
		if oracle_data.is_empty() {
			None
		} else {
			Some(oracle_data)
		}
	}

	#[allow(clippy::too_many_arguments)]
	#[allow(clippy::type_complexity)]
	/// Main liquidation logic of the worker.
	/// Submits unsigned liquidation transactions for validated liquidation opportunities.
	fn try_liquidate(
		client: Arc<C>,
		config: LiquidationWorkerConfig,
		transaction_pool: Arc<P>,
		spawner: SpawnTaskHandle,
		header: B::Header,
		current_evm_timestamp: u64,
		borrowers: &mut [Borrower],
		borrower: &Borrower,
		updated_assets: Option<&Vec<AssetAddress>>,
		money_market: &mut MoneyMarketData<B, OriginCaller, RuntimeCall, RuntimeEvent>,
		liquidated_users: &mut Vec<UserAddress>,
		max_liquidations: usize,
		tx_waitlist: &mut HashSet<EvmAddress>,
	) -> Result<(), ()> {
		let hash = header.hash();

		let Some(ref mut borrower) = borrowers
			.iter_mut()
			.find(|element| element.user_address == borrower.user_address)
		else {
			return Ok(());
		};

		// Skip if the user has been already liquidated in this block.
		if liquidated_users.contains(&borrower.user_address) {
			tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} user {} has already been liquidated in this block.", header.number(), borrower.user_address);
			return Ok(());
		};

		// Skip if there is an oracle update and the user has been placed on the waitlist.
		if updated_assets.is_some() && tx_waitlist.contains(&borrower.user_address) {
			tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} skipping try_liquidate for user {}. The user is in tx_waitlist and the block contains price update.", header.number(), borrower.user_address);
			return Ok(());
		}

		// Get `UserData` based on updated price.
		let Ok(user_data) = UserData::new(
			ApiProvider::<&C::Api>(client.clone().runtime_api().deref()),
			header.hash(),
			money_market,
			borrower.user_address,
			current_evm_timestamp,
			config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
		) else {
			tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} failed to get user data for user {:?}", header.number(), borrower.user_address);
			return Ok(());
		};

		if let Ok(current_hf) =
			user_data.health_factor::<B, ApiProvider<&C::Api>, OriginCaller, RuntimeCall, RuntimeEvent>(money_market)
		{
			// Update user's HF.
			borrower.health_factor = current_hf;

			let hf_one = U256::from(10u128.pow(18));
			if current_hf > hf_one {
				tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} HF of user {:?} above one, skipping execution", header.number(), borrower.user_address);
				return Ok(());
			}
		} else {
			// We were unable to get user's HF. Skip the execution for this user.
			tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} failed to get HF for user {:?}", header.number(), borrower.user_address);
			return Ok(());
		}

		if let Ok(Some(liquidation_option)) = money_market.get_best_liquidation_option::<ApiProvider<&C::Api>>(
			&user_data,
			config.target_hf.into(),
			updated_assets,
		) {
			let (Some(collateral_asset_id), Some(debt_asset_id)) = (
				money_market.address_to_asset(liquidation_option.collateral_asset),
				money_market.address_to_asset(liquidation_option.debt_asset),
			) else {
				tracing::error!(target: LOG_TARGET, "liquidation-worker: address_to_asset conversion failed");
				return Ok(());
			};

			let Ok(debt_to_liquidate) = liquidation_option.debt_to_liquidate.try_into() else {
				return Ok(());
			};

			let liquidation_tx = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
				collateral_asset: collateral_asset_id,
				debt_asset: debt_asset_id,
				user: borrower.user_address,
				debt_to_cover: debt_to_liquidate,
				route: BoundedVec::new(),
			});

			let encoded_tx: fp_self_contained::UncheckedExtrinsic<
				hydradx_runtime::Address,
				RuntimeCall,
				hydradx_runtime::Signature,
				hydradx_runtime::SignedExtra,
			> = fp_self_contained::UncheckedExtrinsic::new_unsigned(liquidation_tx.clone());
			let encoded = encoded_tx.encode();
			let opaque_tx =
				sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid");

			// There is no guarantee that the TX will be executed and with the result we expect. The HF after the execution can be slightly different from what we can predict.
			// Reset the HF to 0 so it will be recalculated again.
			borrower.health_factor = U256::zero();

			if liquidated_users.len() >= max_liquidations {
				// Don't submit new transactions if the transaction limit per block has been reached.
				return Ok(());
			}

			// Dry run if there is no oracle update and the user has been placed on the waitlist.
			// We do not dry run if there is an oracle update because this would provide incorrect result due to the incorrect state of the MoneyMarketData struct.
			if updated_assets.is_none() && tx_waitlist.contains(&borrower.user_address) {
				// dry run to prevent spamming with extrinsic that will fail (e.g. because of not being profitable)
				let dry_run_result = ApiProvider::<&C::Api>(client.runtime_api().deref()).dry_run_call(
					hash,
					hydradx_runtime::RuntimeOrigin::none().caller,
					liquidation_tx,
				);

				if let Ok(Ok(call_result)) = dry_run_result {
					if let Err(error) = call_result.execution_result {
						tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} Dry running liquidation failed for user {:?}, assets {:?} {:?} and debt amount {:?} with reason: {:?}", header.number(), borrower.user_address, collateral_asset_id, debt_asset_id, debt_to_liquidate, error);
						return Ok(());
					}
				} else {
					return Ok(());
				}
			}

			// add user to the list of borrowers that are liquidated in this run.
			liquidated_users.push(borrower.user_address);

			let _ = tx_waitlist.insert(borrower.user_address);

			let tx_pool_c = transaction_pool.clone();
			let borrower_c = borrower.clone();
			// `tx_pool::submit_one()` returns a Future type, so we need to spawn a new task
			spawner.spawn("liquidation-worker-on-submit", Some("liquidation-worker"), async move {
				let submit_result = tx_pool_c
					.submit_one(hash, TransactionSource::Local, opaque_tx.into())
					.await;
				tracing::info!(target: LOG_TARGET, "liquidation-worker: {:?} Submit result for user {:?}: {:?}", header.number(), borrower_c.user_address, submit_result);
			});
		} else {
			tracing::debug!(target: LOG_TARGET, "liquidation-worker: {:?} failed to get liquidation option for user {:?}", header.number(), borrower.user_address);
		}

		Ok(())
	}

	#[allow(clippy::too_many_arguments)]
	fn liquidation_worker(
		client: Arc<C>,
		config: LiquidationWorkerConfig,
		transaction_pool: Arc<P>,
		spawner: SpawnTaskHandle,
		header: B::Header,
		sorted_borrowers: Vec<Borrower>,
		money_market: MoneyMarketData<B, OriginCaller, RuntimeCall, RuntimeEvent>,
		worker_channel_rx: mpsc::Receiver<MessageType<B>>,
		liquidation_task_data: Arc<LiquidationTaskData>,
	) {
		let mut header = header;
		let mut borrowers = sorted_borrowers;
		// We need two lists of borrowers. One mutable that we can update and one we can iterate over.
		let mut borrowers_c = borrowers.clone();
		let mut money_market = money_market;
		let mut tx_waitlist = HashSet::<EvmAddress>::new();

		let mut max_transactions =
			Self::calculate_max_number_of_liquidations_in_block(config.clone()).unwrap_or_default();

		let runtime_api = client.runtime_api();
		let Some(mut current_evm_timestamp) =
			ApiProvider::<&C::Api>(runtime_api.deref()).current_timestamp(header.hash())
		else {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: fetch_current_evm_block_timestamp failed");
			return;
		};

		// List of liquidated users in this block.
		// We don't try to liquidate a user more than once in a block.
		let mut liquidated_users = Vec::<UserAddress>::new();

		let mut current_task = LiquidationWorkerTask::WaitForNewTransaction;

		'main_loop: loop {
			match worker_channel_rx.try_recv() {
				Ok(MessageType::Block(block, new_borrowers, liquidated_users_in_last_block)) => {
					tracing::info!(target: LOG_TARGET, "\nliquidation-worker-state: {:?} received NewBlock", block.header.number());

					Self::process_new_block(
						&block,
						&mut header,
						client.clone(),
						&config,
						&mut money_market,
						&mut borrowers,
						&mut borrowers_c,
						&mut liquidated_users,
						&mut current_evm_timestamp,
						new_borrowers,
						liquidated_users_in_last_block,
						&mut tx_waitlist,
						&mut max_transactions,
						liquidation_task_data.clone(),
					);

					current_task = LiquidationWorkerTask::LiquidateAll;
				}
				Ok(MessageType::Transaction(TransactionType::OracleUpdate(oracle_update_data))) => {
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} received OracleUpdate", header.number());
					current_task = LiquidationWorkerTask::OracleUpdate(oracle_update_data);
				}
				Err(mpsc::TryRecvError::Empty) => {}
				Err(mpsc::TryRecvError::Disconnected) => {
					// disconnected, we will not receive any new messages from the channel.
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: exiting worker thread");
					return;
				}
			};

			match current_task {
				LiquidationWorkerTask::LiquidateAll => {
					let now = std::time::Instant::now();
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} starting LiquidateAll", header.number());

					// Iterate over all borrowers and try to liquidate them.
					for (index, borrower) in borrowers_c.iter().enumerate() {
						match worker_channel_rx.try_recv() {
							Ok(MessageType::Block(block, new_borrowers, liquidated_users_in_last_block)) => {
								tracing::info!(target: LOG_TARGET, "\nliquidation-worker-state: {:?} received NewBlock", block.header.number());

								Self::process_new_block(
									&block,
									&mut header,
									client.clone(),
									&config,
									&mut money_market,
									&mut borrowers,
									&mut borrowers_c,
									&mut liquidated_users,
									&mut current_evm_timestamp,
									new_borrowers,
									liquidated_users_in_last_block,
									&mut tx_waitlist,
									&mut max_transactions,
									liquidation_task_data.clone(),
								);

								// Restart `LiquidateAll` task.
								continue 'main_loop;
							}
							Ok(MessageType::Transaction(TransactionType::OracleUpdate(oracle_update_data))) => {
								tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} received OracleUpdate", header.number());
								tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} LiquidateAll execution time: {:?}, borrowers processed: {:?}", header.number(), now.elapsed().as_millis(), index);
								current_task = LiquidationWorkerTask::OracleUpdate(oracle_update_data);
								continue 'main_loop;
							}
							_ => (),
						}

						match Self::try_liquidate(
							client.clone(),
							config.clone(),
							transaction_pool.clone(),
							spawner.clone(),
							header.clone(),
							current_evm_timestamp,
							&mut borrowers,
							borrower,
							None,
							&mut money_market,
							&mut liquidated_users,
							max_transactions,
							&mut tx_waitlist,
						) {
							Ok(()) => (),
							Err(()) => return,
						}
					}

					// We iterated over all borrowers, wait for a new oracle update.
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} LiquidateAll processed all borrowers. Execution time: {:?}", header.number(), now.elapsed().as_millis());
					current_task = LiquidationWorkerTask::WaitForNewTransaction;
				}
				LiquidationWorkerTask::OracleUpdate(ref oracle_update_data) => {
					let now = std::time::Instant::now();
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} starting OracleUpdate", header.number());

					let mut updated_assets = Vec::new();
					// Iterate over all price updates and aggregate all price updates first.
					// All oracle updates we use are quoted in USD.
					for (base_asset_address, maybe_new_price) in oracle_update_data.iter() {
						if let Some(new_price) = maybe_new_price {
							money_market.update_reserve_price(*base_asset_address, new_price);
						}
						updated_assets.push(*base_asset_address);
					}

					// Iterate over all borrowers and try to liquidate them.
					for (index, borrower) in borrowers_c.iter().enumerate() {
						match worker_channel_rx.try_recv() {
							Ok(MessageType::Block(block, new_borrowers, liquidated_users_in_last_block)) => {
								tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} received NewBlock", block.header.number());

								Self::process_new_block(
									&block,
									&mut header,
									client.clone(),
									&config,
									&mut money_market,
									&mut borrowers,
									&mut borrowers_c,
									&mut liquidated_users,
									&mut current_evm_timestamp,
									new_borrowers,
									liquidated_users_in_last_block,
									&mut tx_waitlist,
									&mut max_transactions,
									liquidation_task_data.clone(),
								);

								// Restart `LiquidateAll` task.
								current_task = LiquidationWorkerTask::LiquidateAll;
								continue 'main_loop;
							}
							Ok(MessageType::Transaction(TransactionType::OracleUpdate(oracle_update_data))) => {
								tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} received OracleUpdate", header.number());
								tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} OracleUpdate execution time: {:?}, borrowers processed: {:?}", header.number(), now.elapsed().as_millis(), index);
								// New oracle update received. Skip the execution and process a new oracle update.
								current_task = LiquidationWorkerTask::OracleUpdate(oracle_update_data);
								continue 'main_loop;
							}
							_ => (),
						}

						match Self::try_liquidate(
							client.clone(),
							config.clone(),
							transaction_pool.clone(),
							spawner.clone(),
							header.clone(),
							current_evm_timestamp,
							&mut borrowers,
							borrower,
							Some(&updated_assets),
							&mut money_market,
							&mut liquidated_users,
							max_transactions,
							&mut tx_waitlist,
						) {
							Ok(()) => (),
							Err(()) => return,
						}
					}

					// We iterated over all borrowers, wait for a new oracle update.
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} OracleUpdate processed all borrowers. Execution time: {:?}", header.number(), now.elapsed().as_millis());
					current_task = LiquidationWorkerTask::LiquidateAll;
				}
				LiquidationWorkerTask::WaitForNewTransaction => {
					tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} starting WaitForNewTransaction", header.number());

					match worker_channel_rx.recv() {
						Ok(MessageType::Block(block, new_borrowers, liquidated_users_in_last_block)) => {
							tracing::info!(target: LOG_TARGET, "\n\nliquidation-worker-state: {:?} received NewBlock", block.header.number());

							Self::process_new_block(
								&block,
								&mut header,
								client.clone(),
								&config,
								&mut money_market,
								&mut borrowers,
								&mut borrowers_c,
								&mut liquidated_users,
								&mut current_evm_timestamp,
								new_borrowers,
								liquidated_users_in_last_block,
								&mut tx_waitlist,
								&mut max_transactions,
								liquidation_task_data.clone(),
							);

							current_task = LiquidationWorkerTask::LiquidateAll;
						}
						Ok(MessageType::Transaction(TransactionType::OracleUpdate(oracle_update_data))) => {
							tracing::info!(target: LOG_TARGET, "liquidation-worker-state: {:?} received OracleUpdate", header.number());
							current_task = LiquidationWorkerTask::OracleUpdate(oracle_update_data);
						}
						Err(mpsc::RecvError) => {
							// disconnected, we will not receive any new messages from the channel.
							return;
						}
					}
				}
			}
		}
	}

	fn calculate_max_number_of_liquidations_in_block(config: LiquidationWorkerConfig) -> Option<usize> {
		let max_block_weight = hydradx_runtime::BlockWeights::get()
			.get(frame_support::dispatch::DispatchClass::Normal)
			.max_total
			.unwrap_or_default();

		let liquidation_weight = pallet_liquidation::Call::<hydradx_runtime::Runtime>::liquidate {
			collateral_asset: Default::default(),
			debt_asset: Default::default(),
			user: Default::default(),
			debt_to_cover: Default::default(),
			route: BoundedVec::new(),
		};
		let liquidation_weight = liquidation_weight.get_dispatch_info().weight;

		let allowed_weight = 100u8.saturating_sub(config.weight_reserve);
		let max_block_weight = Percent::from_percent(allowed_weight) * max_block_weight;

		max_block_weight
			.checked_div_per_component(&liquidation_weight)
			.map(|limit| limit as usize)
	}

	fn get_events(
		client: Arc<C>,
		block_hash: B::Hash,
	) -> Result<Vec<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>, ()> {
		if let Ok(Some(encoded_events)) = client.storage(
			block_hash,
			&StorageKey(hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec()), // System::events storage key
		) {
			if let Ok(events) = Vec::<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>::decode(
				&mut encoded_events.0.as_slice(),
			) {
				return Ok(events);
			}
		}

		Err(())
	}

	fn filter_events(
		events: Vec<frame_system::EventRecord<RuntimeEvent, hydradx_runtime::Hash>>,
	) -> Option<(Vec<UserAddress>, Vec<AssetAddress>, Vec<UserAddress>)> {
		let mut new_borrows: Vec<UserAddress> = Vec::new();
		let mut new_assets = Vec::<AssetAddress>::new();
		let mut liquidated_users = Vec::<AssetAddress>::new();

		for event in events {
			match &event.event {
				RuntimeEvent::EVM(pallet_evm::Event::Log { log }) => {
					if log.address == BORROW_CALL_ADDRESS && log.topics[0] == events::BORROW {
						if let Some(&borrower) = log.topics.get(2) {
							new_borrows.push(UserAddress::from(borrower));
						}
					} else if log.address == POOL_CONFIGURATOR_ADDRESS
						&& log.topics[0] == events::COLLATERAL_CONFIGURATION_CHANGED
					{
						if let Some(&asset) = log.topics.get(1) {
							new_assets.push(AssetAddress::from(asset));
						}
					}
				}
				RuntimeEvent::Liquidation(pallet_liquidation::Event::Liquidated { user, .. }) => {
					liquidated_users.push(user.clone());
				}
				_ => {}
			}
		}

		Some((new_borrows, new_assets, liquidated_users))
	}

	#[allow(clippy::too_many_arguments)]
	fn process_new_block(
		new_block: &sc_client_api::client::BlockImportNotification<B>,
		header: &mut B::Header,
		client: Arc<C>,
		config: &LiquidationWorkerConfig,
		money_market: &mut MoneyMarketData<B, OriginCaller, RuntimeCall, RuntimeEvent>,
		borrowers: &mut Vec<Borrower>,
		borrowers_c: &mut Vec<Borrower>,
		liquidated_users: &mut Vec<UserAddress>,
		current_evm_timestamp: &mut u64,
		new_borrowers: Vec<UserAddress>,
		liquidated_users_in_last_block: Vec<UserAddress>,
		tx_waitlist: &mut HashSet<UserAddress>,
		max_transactions: &mut usize,
		liquidation_task_data: Arc<LiquidationTaskData>,
	) {
		// Update variables.
		*header = new_block.header.clone();

		*max_transactions = Self::calculate_max_number_of_liquidations_in_block(config.clone()).unwrap_or_default();
		// Set the number of max liquidation calls in a block for the liquidation RPC API.
		if let Ok(mut max_txs) = liquidation_task_data.max_transactions.lock() {
			*max_txs = max_transactions.clone();
		}

		let _ = Self::add_new_borrowers(new_borrowers, borrowers);

		tracing::debug!(target: LOG_TARGET, "liquidation-worker: liquidated_users_in_last_block: {:?}", liquidated_users_in_last_block);
		for liquidated_user in liquidated_users_in_last_block {
			let _ = tx_waitlist.remove(&liquidated_user);
		}

		let runtime_api = client.runtime_api();

		let Ok(new_money_market) =
			MoneyMarketData::<B, OriginCaller, RuntimeCall, RuntimeEvent>::new::<ApiProvider<&C::Api>>(
				ApiProvider::<&C::Api>(runtime_api.deref()),
				header.hash(),
				config.pap_contract.unwrap_or(PAP_CONTRACT),
				config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
			)
		else {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: MoneyMarketData initialization failed");
			return;
		};

		*money_market = new_money_market;

		*borrowers_c = borrowers.to_owned();

		// Update the copy of the borrowers list for the liquidation RPC API.
		if let Ok(mut borrowers_ext) = liquidation_task_data.borrowers_list.lock() {
			*borrowers_ext = borrowers_c.clone();
		}

		liquidated_users.clear();

		let Some(new_evm_timestamp) = ApiProvider::<&C::Api>(runtime_api.deref()).current_timestamp(header.hash())
		else {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: fetch_current_evm_block_timestamp failed");
			return;
		};
		*current_evm_timestamp = new_evm_timestamp;
	}

	/// Fetch the preprocessed data used to evaluate possible candidates for liquidation.
	async fn fetch_borrowers_data(url: String) -> Option<BorrowersData<AccountId>> {
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_native_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();
		let http_client: HttpClient = Arc::new(Client::builder().build(https));

		let url = url.parse().ok()?;
		let res = http_client.get(url).await.ok()?;
		if res.status() != StatusCode::OK {
			tracing::error!(target: LOG_TARGET, "liquidation-worker: failed to fetch borrowers data");
			return None;
		}

		let bytes = hyper::body::to_bytes(res.into_body()).await.ok()?;

		let data = String::from_utf8(bytes.to_vec()).ok()?;
		let data = data.as_str();
		let data = serde_json::from_str::<BorrowersData<AccountId>>(data);
		data.ok()
	}

	/// Returns borrowers sorted by HF.
	/// The list is sorted in ascending order, starting with borrowers whose HF has not yet been
	/// calculated (HF==0).
	fn process_borrowers_data(oracle_data: BorrowersData<AccountId>) -> Option<Vec<Borrower>> {
		let one = U256::from(10u128.pow(18));
		let fractional_multiplier = U256::from(10u128.pow(12));

		let mut borrowers = oracle_data
			.borrowers
			.iter()
			.map(|(user_address, borrower_data_details)| {
				// I'm not aware of a better way to convert f32 to U256. Use this naive approach and
				// take the first 6 decimals. That should be enough for our purpose.
				let integer_part = U256::from(borrower_data_details.health_factor.trunc() as u128).checked_mul(one);
				let fractional_part = U256::from((borrower_data_details.health_factor.fract() * 1_000_000f32) as u128)
					.checked_mul(fractional_multiplier);

				// return 0 if the computation failed and recalculate the HF later.
				let health_factor = integer_part
					.zip(fractional_part)
					.and_then(|(i, f)| i.checked_add(f))
					.unwrap_or_default();

				Borrower {
					user_address: *user_address,
					health_factor,
				}
			})
			.collect::<Vec<_>>();

		// sort by HF
		borrowers.sort_by(|a, b| a.health_factor.partial_cmp(&b.health_factor).unwrap_or(Ordering::Equal));

		Some(borrowers)
	}

	/// Check if the provided transaction is a valid DIA oracle update.
	/// All Ethereum transaction types are supported.
	fn verify_oracle_update_transaction(
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
				Transaction::Legacy(legacy_transaction) => legacy_transaction.action,
				Transaction::EIP2930(eip2930_transaction) => eip2930_transaction.action,
				Transaction::EIP1559(eip1559_transaction) => eip1559_transaction.action,
			};

			// check if the transaction is DIA oracle update
			if let pallet_ethereum::TransactionAction::Call(call_address) = action {
				if allowed_oracle_call_addresses.contains(&call_address) {
					// additional check to prevent running the worker for DIA oracle updates signed by invalid address
					if let Some(Ok(signer)) = extrinsic.function.check_self_contained() {
						if allowed_signers.contains(&signer) {
							return Some(transaction);
						};
					}
				};
			};
		};

		None
	}

	/// Adds a new borrower to the borrower list.
	/// If the borrower is already in the list, invalidates the HF by setting it to 0 so the HF will be recalculated.
	/// We don't try to liquidate on new borrows.
	fn add_new_borrowers(new_borrowers: Vec<UserAddress>, borrowers: &mut Vec<Borrower>) -> Result<(), ()> {
		for user_address in new_borrowers {
			match borrowers.iter_mut().find(|b| b.user_address == user_address) {
				Some(b) => {
					// Borrower is already on the list. Invalidate the HF by setting it to 0 and adding an asset to the list.
					b.health_factor = U256::zero();
				}
				None => {
					// add new borrower to the list. HF is set to 0, so we can place it at the beginning and the list will remain sorted.
					borrowers.insert(
						0,
						Borrower {
							user_address,
							health_factor: U256::zero(),
						},
					);
				}
			}
		}

		// Sort the borrowers by the health factor.
		borrowers.sort_by(|a, b| a.health_factor.partial_cmp(&b.health_factor).unwrap_or(Ordering::Equal));

		Ok(())
	}
}

/// The data from DIA oracle update transaction.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
struct OracleUpdataData {
	base_asset_name: AssetSymbol,
	quote_asset: AssetSymbol,
	price: Price,
	timestamp: U256,
}
impl OracleUpdataData {
	pub fn new(base_asset_name: AssetSymbol, quote_asset: AssetSymbol, price: Price, timestamp: U256) -> Self {
		Self {
			base_asset_name,
			quote_asset,
			price,
			timestamp,
		}
	}
}

/// Parse DIA oracle update transaction.
/// All Ethereum transaction types are supported.
/// Returns a list of `OracleUpdateData`.
fn parse_oracle_transaction(eth_tx: &Transaction) -> Option<Vec<OracleUpdataData>> {
	let transaction_input = match eth_tx {
		Transaction::Legacy(legacy_transaction) => &legacy_transaction.input,
		Transaction::EIP2930(eip2930_transaction) => &eip2930_transaction.input,
		Transaction::EIP1559(eip1559_transaction) => &eip1559_transaction.input,
	};

	let mut dia_oracle_data = Vec::new();

	let fn_selector = &transaction_input[0..4];

	if fn_selector == Into::<u32>::into(Function::SetValue).to_be_bytes() {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::String,
				ethabi::ParamType::Uint(16),
				ethabi::ParamType::Uint(16),
			],
			&transaction_input[4..], // first 4 bytes are function selector
		)
		.ok()?;

		dia_oracle_data.push((
			decoded[0].clone().into_string()?,
			decoded[1].clone().into_uint()?,
			decoded[2].clone().into_uint()?,
		));
	} else if fn_selector == Into::<u32>::into(Function::SetMultipleValues).to_be_bytes() {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
			],
			&transaction_input[4..], // first 4 bytes are function selector
		)
		.ok()?;

		if decoded.len() == 2 {
			for (asset_str, price_and_timestamp) in sp_std::iter::zip(
				decoded[0].clone().into_array()?.iter(),
				decoded[1].clone().into_array()?.iter(),
			) {
				let price_and_timestamp = price_and_timestamp.clone().into_uint()?;
				let price = Price::from_little_endian(&price_and_timestamp.encode_as()[16..32]);
				let timestamp = U256::from_little_endian(&price_and_timestamp.encode_as()[0..16]);
				dia_oracle_data.push((asset_str.clone().into_string()?, price, timestamp));
			}
		};
	}

	let mut result = Vec::new();
	for (asset_str, price, timestamp) in dia_oracle_data.iter() {
		// we expect the asset string to be in the format of "DOT/USD"
		let mut assets = asset_str
			.split("/")
			.map(|s| s.as_bytes().to_vec())
			.collect::<Vec<AssetSymbol>>();
		if assets.len() != 2 {
			continue;
		};

		// remove null terminator from the second asset string
		if assets[1].last().cloned() == Some(0) {
			let quote_asset_len = assets[1].len().saturating_sub(1);
			assets[1].truncate(quote_asset_len);
		}

		result.push(OracleUpdataData::new(
			assets[0].clone(),
			assets[1].clone(),
			*price,
			*timestamp,
		));
	}

	Some(result)
}

pub mod rpc {
	use crate::liquidation_worker::LiquidationTaskData;
	use jsonrpsee::{
		core::{async_trait, RpcResult},
		proc_macros::rpc,
		types::error::ErrorObject,
	};
	use liquidation_worker_support::Borrower;
	use std::sync::Arc;

	#[rpc(client, server)]
	pub trait LiquidationWorkerApi {
		#[method(name = "liquidation_getBorrowers")]
		async fn get_borrowers(&self) -> RpcResult<Vec<Borrower>>;

		#[method(name = "liquidation_isRunning")]
		async fn is_running(&self) -> RpcResult<bool>;

		#[method(name = "liquidation_maxTransactionsPerBlock")]
		async fn max_transactions_per_block(&self) -> RpcResult<usize>;
	}

	/// Error type of this RPC api.
	pub enum Error {
		/// Getting the lock failed.
		LockError,
	}

	impl From<Error> for i32 {
		fn from(e: Error) -> i32 {
			match e {
				Error::LockError => 1,
			}
		}
	}

	/// Provides RPC methods.
	pub struct LiquidationWorker {
		pub liquidation_task_data: Arc<LiquidationTaskData>,
	}

	impl LiquidationWorker {
		pub fn new(liquidation_task_data: Arc<LiquidationTaskData>) -> Self {
			Self { liquidation_task_data }
		}
	}

	#[async_trait]
	impl LiquidationWorkerApiServer for LiquidationWorker {
		async fn get_borrowers(&self) -> RpcResult<Vec<Borrower>> {
			if let Ok(borrowers) = self.liquidation_task_data.borrowers_list.lock() {
				Ok(borrowers.clone())
			} else {
				Ok(Vec::new())
			}
		}

		async fn is_running(&self) -> RpcResult<bool> {
			if let Ok(thread_pool) = self.liquidation_task_data.clone().thread_pool.lock() {
				if thread_pool.active_count() > 0 {
					return Ok(true);
				}
			}

			Ok(false)
		}

		async fn max_transactions_per_block(&self) -> RpcResult<usize> {
			if let Ok(max_transactions) = self.liquidation_task_data.max_transactions.lock() {
				Ok(*max_transactions)
			} else {
				Err(ErrorObject::owned(
					Error::LockError.into(),
					"Unable to acquire the max_transactions lock. PEPL probably not running.",
					None::<String>,
				))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::H256;

	fn dummy_dia_tx_single_value() -> Transaction {
		Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: U256::from(9264),
			gas_price: U256::from(5143629),
			gas_limit: U256::from(80674),
			action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
				hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
			)),
			value: U256::from(0),
			//
			input: hex!(
				"7898e0c2\
				0000000000000000000000000000000000000000000000000000000000000060\
				000000000000000000000000000000000000000000000000000007b205c4101d\
				0000000000000000000000000000000000000000000000000000000067fd2a55\
				0000000000000000000000000000000000000000000000000000000000000008\
				744254432f555344000000000000000000000000000000000000000000000000"
			)
			.encode_as(),
			signature: ethereum::TransactionSignature::new(
				444480,
				H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
				H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
			)
			.unwrap(),
		})
	}

	// setMultipleValues(string[] keys, uint256[] compressedValues)
	#[cfg(test)]
	fn dummy_dia_tx_multiple_values() -> Transaction {
		Transaction::Legacy(ethereum::LegacyTransaction {
			nonce: U256::from(9264),
			gas_price: U256::from(5143629),
			gas_limit: U256::from(80674),
			action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
				hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
			)),
			value: U256::from(0),
			//
			input: hex!(
				"8d241526\
                0000000000000000000000000000000000000000000000000000000000000040\
                0000000000000000000000000000000000000000000000000000000000000120\
                0000000000000000000000000000000000000000000000000000000000000002\
                0000000000000000000000000000000000000000000000000000000000000040\
                0000000000000000000000000000000000000000000000000000000000000080\
                0000000000000000000000000000000000000000000000000000000000000008\
                444f542f45544800000000000000000000000000000000000000000000000000\
                0000000000000000000000000000000000000000000000000000000000000008\
                4441492f45544800000000000000000000000000000000000000000000000000\
                0000000000000000000000000000000000000000000000000000000000000002\
                00000000000000000000000029b5c33700000000000000000000000067acbce5\
                000000000000000000000005939a32ea00000000000000000000000067acbce5"
			)
			.encode_as(),
			signature: ethereum::TransactionSignature::new(
				444480,
				H256::from_slice(hex!("6fd26272de1d95aea3df6d0a5eb554bb6a16bf2bff563e2216661f1a49ed3f8a").as_slice()),
				H256::from_slice(hex!("4bf0c9b80cc75a3860f0ae2fcddc9154366ddb010e6d70b236312299862e525c").as_slice()),
			)
			.unwrap(),
		})
	}

	#[test]
	fn parse_oracle_transaction_should_work() {
		// set single value
		let tx = dummy_dia_tx_single_value();
		let expected = vec![OracleUpdataData::new(
			"tBTC".as_bytes().to_vec(),
			"USD".as_bytes().to_vec(),
			U256::from(8461182308381u128),
			U256::from(1744644693u128),
		)];
		assert_eq!(expected, parse_oracle_transaction(&tx).unwrap());

		// set multiple values
		let tx = dummy_dia_tx_multiple_values();
		let expected = vec![
			OracleUpdataData::new(
				"DOT".as_bytes().to_vec(),
				"ETH".as_bytes().to_vec(),
				U256::from(699777847u128),
				U256::from(1739373797u128),
			),
			OracleUpdataData::new(
				"DAI".as_bytes().to_vec(),
				"ETH".as_bytes().to_vec(),
				U256::from(23951192810u128),
				U256::from(1739373797u128),
			),
		];
		assert_eq!(expected, parse_oracle_transaction(&tx).unwrap());
	}
}
