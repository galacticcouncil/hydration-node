//! Node-mode orchestration: LiquidationTask, LiquidationWorkerConfig, borrower fetching.
//!
//! This is the full async orchestration layer moved from `node/src/liquidation_worker.rs`.

use super::*;
use crate::config;
use crate::oracle;
use crate::traits::*;
use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use frame_support::{dispatch::GetDispatchInfo, BoundedVec};
use futures::StreamExt;
use hydradx_runtime::{evm::precompiles::erc20_mapping::Erc20MappingApi, OriginCaller, RuntimeCall, RuntimeEvent};
use liquidation_worker_support::*;
use pallet_currencies_rpc_runtime_api::CurrenciesApi;
use sc_client_api::{Backend, BlockchainEvents, StorageProvider};
use sc_service::SpawnTaskHandle;
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::{traits::Header, Percent, SaturatedConversion};
use std::{
	cmp::Ordering,
	collections::HashMap,
	marker::PhantomData,
	sync::{mpsc, Arc},
};

const LOG_TARGET: &str = "liquidation-worker";

type HttpClient = Arc<
	hyperv14::Client<
		hyper_rustls::HttpsConnector<hyperv14::client::HttpConnector>,
		hyperv14::body::Body,
	>,
>;

/// The configuration for the liquidation worker (CLI arguments).
#[derive(Clone, Debug, clap::Parser)]
pub struct LiquidationWorkerConfig {
	/// Enable/disable execution of the liquidation worker.
	#[clap(long)]
	pub liquidation_worker: Option<bool>,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pap_contract: Option<primitives::EvmAddress>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub runtime_api_caller: Option<primitives::EvmAddress>,

	/// EVM address of the account that signs DIA oracle update.
	#[clap(long)]
	pub oracle_update_signer: Option<Vec<primitives::EvmAddress>>,

	/// EVM address of the DIA oracle update call address.
	#[clap(long)]
	pub oracle_update_call_address: Option<Vec<primitives::EvmAddress>>,

	/// Target health factor.
	#[clap(long, default_value_t = config::DEFAULT_TARGET_HF)]
	pub target_hf: u128,

	/// URL to fetch initial borrowers data from.
	#[clap(long, default_value = config::DEFAULT_OMNIWATCH_URL)]
	pub omniwatch_url: String,

	/// Percentage of the block weight reserved for other transactions.
	#[clap(long, default_value_t = config::DEFAULT_WEIGHT_RESERVE)]
	pub weight_reserve: u8,
}

pub struct LiquidationTask<B, C, BE, P>(PhantomData<(B, C, BE, P)>);

impl<B, C, BE, P> LiquidationTask<B, C, BE, P>
where
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B>
		+ Erc20MappingApi<B>
		+ xcm_runtime_apis::dry_run::DryRunApi<B, RuntimeCall, RuntimeEvent, OriginCaller>
		+ CurrenciesApi<B, AssetId, AccountId, Balance>,
	C: BlockchainEvents<B> + 'static,
	C: HeaderBackend<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
	B::Hash: From<sp_core::H256> + Into<sp_core::H256>,
{
	/// Starting point for the liquidation worker.
	/// Executes the worker loop on every block.
	/// The initial list of borrowers is fetched and sorted by the HF.
	pub async fn run(
		client: Arc<C>,
		cli_config: LiquidationWorkerConfig,
		transaction_pool: Arc<P>,
		spawner: SpawnTaskHandle,
		liquidation_task_data: Arc<LiquidationTaskData>,
	) {
		log::info!(target: LOG_TARGET, "liquidation-worker: starting");

		let runtime_api = client.runtime_api();
		let current_hash = client.info().best_hash;
		let Ok(Some(_header)) = client.header(current_hash) else {
			return;
		};

		let has_api_v2 =
			runtime_api.has_api_with::<dyn OffchainWorkerApi<B>, _>(current_hash, |v| v == 2);
		if let Ok(true) = has_api_v2 {
		} else {
			log::error!(
				target: LOG_TARGET,
				"liquidation-worker: Unsupported Offchain Worker API version. Consider turning off offchain workers if they are not part of your runtime.",
			);
			return;
		};

		// Fetch and sort the list of borrowers.
		let Some(borrowers_data) =
			Self::fetch_borrowers_data(cli_config.omniwatch_url.clone()).await
		else {
			log::error!(target: LOG_TARGET, "liquidation-worker: fetch_borrowers_data failed");
			return;
		};
		let Some(sorted_borrowers) = Self::process_borrowers_data(borrowers_data) else {
			log::error!(target: LOG_TARGET, "liquidation-worker: process_borrowers_data failed");
			return;
		};

		let pap_contract = cli_config.pap_contract.unwrap_or(config::DEFAULT_PAP_CONTRACT);
		let runtime_api_caller = cli_config
			.runtime_api_caller
			.unwrap_or(config::DEFAULT_RUNTIME_API_CALLER);

		// Use `MoneyMarketData` to get the list of reserves.
		let api_provider = ApiProvider::new(client.clone());
		let Ok(money_market) =
			MoneyMarketData::<B, OriginCaller, RuntimeCall, RuntimeEvent>::new(
				api_provider.clone(),
				current_hash,
				pap_contract,
				runtime_api_caller,
			)
		else {
			log::error!(target: LOG_TARGET, "liquidation-worker: MoneyMarketData initialization failed");
			return;
		};

		let mut reserves: HashMap<AssetAddress, AssetSymbol> = money_market
			.reserves()
			.iter()
			.map(|r| (r.asset_address, r.symbol.clone()))
			.collect();

		// Channels for block events and oracle updates.
		let (block_tx, block_rx) = mpsc::channel::<BlockEvent>();
		let (oracle_tx, oracle_rx) = mpsc::channel::<Vec<OracleUpdate>>();

		// Calculate max transactions per block.
		let max_transactions =
			Self::calculate_max_number_of_liquidations_in_block(&cli_config).unwrap_or(10);

		// Set the number of max liquidation calls in a block for the liquidation RPC API.
		if let Ok(mut max_txs) = liquidation_task_data.max_transactions.lock() {
			*max_txs = max_transactions;
		}

		// Build the WorkerConfig.
		let worker_config = crate::WorkerConfig {
			pap_contract,
			runtime_api_caller,
			target_hf: cli_config.target_hf,
			max_liquidations_per_block: max_transactions,
			dry_run: false,
			hf_scan_threshold: None,  // node mode: scan all
			no_interrupt: false,      // node mode: interrupt on new block
			oracle_persist: false,    // node mode: no persistent overrides
		};

		// Get initial EVM timestamp.
		let initial_timestamp = api_provider
			.current_timestamp(current_hash)
			.unwrap_or(0);

		// Oracle signers and addresses.
		let allowed_signers = cli_config
			.oracle_update_signer
			.unwrap_or(config::DEFAULT_ORACLE_UPDATE_SIGNERS.to_vec());
		let allowed_oracle_call_addresses = cli_config
			.oracle_update_call_address
			.unwrap_or(config::DEFAULT_ORACLE_UPDATE_CALL_ADDRESSES.to_vec());

		let liquidation_task_data_worker = liquidation_task_data.clone();
		let transaction_pool_for_worker = transaction_pool.clone();
		let api_provider_for_worker = api_provider.clone();

		// Spawn the worker thread (blocking — uses Handle::block_on internally via RuntimeApiProvider).
		if let Ok(thread_pool) = liquidation_task_data.thread_pool.lock() {
			thread_pool.execute(move || {
				let mut block_source =
					crate::node::block_source::NodeBlockSource::new(block_rx);
				let mut oracle_source =
					crate::node::mempool::NodeMempoolMonitor::new(oracle_rx);
				let submitter = crate::node::tx_submitter::NodeTxSubmitter::new(
					transaction_pool_for_worker,
					spawner,
				);
				let dry_runner = NodeDryRunner {
					client: api_provider_for_worker.client.clone(),
					_phantom: PhantomData,
				};

				let mut money_market = money_market;
				let mut borrowers = sorted_borrowers;

				// Update borrowers list for RPC.
				if let Ok(mut borrowers_ext) =
					liquidation_task_data_worker.borrowers_list.lock()
				{
					*borrowers_ext = borrowers.clone();
				}

				log::info!(
					target: LOG_TARGET,
					"liquidation-worker: starting worker loop ({} borrowers, {} reserves)",
					borrowers.len(),
					money_market.reserves().len()
				);

				crate::run_worker::<
					B,
					OriginCaller,
					RuntimeCall,
					RuntimeEvent,
					_,
					_,
					_,
					_,
					ApiProvider<Arc<C>>,
				>(
					&mut block_source,
					&submitter,
					&mut oracle_source,
					&dry_runner,
					&api_provider_for_worker,
					&worker_config,
					&mut money_market,
					&mut borrowers,
					initial_timestamp,
				);
			});
		}

		// Async event loop: process block and transaction notifications.
		let mut block_notification_stream = client.import_notification_stream();
		let mut transaction_notification_stream =
			transaction_pool.import_notification_stream();

		let mut current_hash = current_hash;

		loop {
			tokio::select! {
				Some(new_block) = block_notification_stream.next() => {
					if new_block.is_new_best {
						let mut borrows: Vec<UserAddress> = Vec::new();
						let mut liquidated_users_in_last_block: Vec<UserAddress> = Vec::new();

						// Get events from the previous block.
						if let Ok(events) = get_events::<B, C, BE>(client.clone(), current_hash) {
							let (new_borrows, new_assets, liquidated_users) =
								filter_events(events);
							for new_asset_address in new_assets {
								let Ok(symbol) = MoneyMarketData::<
									B,
									OriginCaller,
									RuntimeCall,
									RuntimeEvent,
								>::fetch_asset_symbol::<ApiProvider<Arc<C>>>(
									&api_provider,
									current_hash,
									&new_asset_address,
									runtime_api_caller,
								) else {
									continue;
								};

								reserves.insert(new_asset_address, symbol);
							}
							borrows.extend(new_borrows);
							liquidated_users_in_last_block.extend(liquidated_users);
						}

						current_hash = new_block.hash;

						let event = BlockEvent {
							block_number: (*new_block.header.number())
								.saturated_into::<u32>(),
							block_hash: {
								let h: sp_core::H256 = new_block.hash.into();
								h.0
							},
							new_borrowers: borrows,
							liquidated_users: liquidated_users_in_last_block,
							new_assets: vec![],
						};

						let _ = block_tx.send(event);
					} else {
						log::info!(target: LOG_TARGET, "liquidation-worker: Skipping non-canon block.")
					}
				},
				Some(new_transaction_notification) = transaction_notification_stream.next() => {
					let Some(pool_tx) = transaction_pool
						.clone()
						.ready_transaction(&new_transaction_notification)
					else {
						continue;
					};

					let opaque_tx_encoded = pool_tx.data().encode();
					let tx = hydradx_runtime::HydraUncheckedExtrinsic::decode(
						&mut &*opaque_tx_encoded,
					);

					let Ok(transaction) = tx else {
						log::error!(target: LOG_TARGET, "liquidation-worker: transaction decoding failed");
						continue;
					};

					if let Some(oracle_updates) = Self::process_oracle_transaction(
						transaction.0,
						&allowed_signers,
						&allowed_oracle_call_addresses,
						&reserves,
					) {
						let _ = oracle_tx.send(oracle_updates);
					}
				},
				else => break
			}
		}
	}

	/// Process an oracle update transaction from the mempool.
	fn process_oracle_transaction(
		extrinsic: sp_runtime::generic::UncheckedExtrinsic<
			hydradx_runtime::Address,
			RuntimeCall,
			hydradx_runtime::Signature,
			hydradx_runtime::SignedExtra,
		>,
		allowed_signers: &[primitives::EvmAddress],
		allowed_oracle_call_addresses: &[primitives::EvmAddress],
		reserves: &HashMap<AssetAddress, AssetSymbol>,
	) -> Option<Vec<OracleUpdate>> {
		let transaction = verify_oracle_update_transaction(
			&extrinsic,
			allowed_signers,
			allowed_oracle_call_addresses,
		)?;

		let input = get_transaction_input(&transaction)?;
		let oracle_data = oracle::parse_oracle_input(input)?;
		let matched = oracle::match_oracle_to_reserves(&oracle_data, reserves);

		if matched.is_empty() {
			return None;
		}

		// Convert (AssetAddress, Option<Price>) to Vec<OracleUpdate>.
		let updates: Vec<OracleUpdate> = matched
			.into_iter()
			.map(|(addr, price)| OracleUpdate {
				asset_address: addr,
				price,
			})
			.collect();

		Some(updates)
	}

	fn calculate_max_number_of_liquidations_in_block(
		config: &LiquidationWorkerConfig,
	) -> Option<usize> {
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
		let liquidation_weight = liquidation_weight.get_dispatch_info().call_weight;

		let allowed_weight = 100u8.saturating_sub(config.weight_reserve);
		let max_block_weight = Percent::from_percent(allowed_weight) * max_block_weight;

		max_block_weight
			.checked_div_per_component(&liquidation_weight)
			.map(|limit| limit as usize)
	}

	/// Fetch the preprocessed data used to evaluate possible candidates for liquidation.
	async fn fetch_borrowers_data(
		url: String,
	) -> Option<BorrowersData<primitives::AccountId>> {
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_webpki_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();
		let http_client: HttpClient =
			Arc::new(hyperv14::Client::builder().build(https));

		let url = url.parse().ok()?;
		let res = http_client.get(url).await.ok()?;
		if res.status() != hyperv14::StatusCode::OK {
			log::error!(target: LOG_TARGET, "liquidation-worker: failed to fetch borrowers data");
			return None;
		}

		let bytes = hyperv14::body::to_bytes(res.into_body()).await.ok()?;
		let data = String::from_utf8(bytes.to_vec()).ok()?;
		let data = serde_json::from_str::<BorrowersData<primitives::AccountId>>(&data);
		data.ok()
	}

	/// Returns borrowers sorted by HF.
	fn process_borrowers_data(
		oracle_data: BorrowersData<primitives::AccountId>,
	) -> Option<Vec<Borrower>> {
		let one = U256::from(10u128.pow(18));
		let fractional_multiplier = U256::from(10u128.pow(12));

		let mut borrowers = oracle_data
			.borrowers
			.iter()
			.map(|(user_address, borrower_data_details)| {
				let integer_part = U256::from(
					borrower_data_details.health_factor.trunc() as u128,
				)
				.checked_mul(one);
				let fractional_part = U256::from(
					(borrower_data_details.health_factor.fract() * 1_000_000f32) as u128,
				)
				.checked_mul(fractional_multiplier);

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

		borrowers.sort_by(|a, b| {
			a.health_factor
				.partial_cmp(&b.health_factor)
				.unwrap_or(Ordering::Equal)
		});

		Some(borrowers)
	}
}
