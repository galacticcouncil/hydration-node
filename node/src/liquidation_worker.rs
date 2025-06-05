use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use fp_self_contained::SelfContainedCall;
use frame_support::{BoundedVec, __private::sp_tracing::tracing};
use futures::{future::ready, StreamExt};
use hex_literal::hex;
use hydradx_runtime::{
	evm::{precompiles::erc20_mapping::Erc20MappingApi, EvmAddress},
	Block, Runtime, RuntimeCall,
};
use hyper::{body::Body, Client, StatusCode};
use hyperv14 as hyper;
use liquidation_worker_support::*;
use pallet_ethereum::Transaction;
use pallet_liquidation::LiquidationWorkerApi;
use parking_lot::Mutex;
use polkadot_primitives::EncodeAs;
use primitives::{AccountId, BlockNumber};
use sc_client_api::{Backend, BlockchainEvents, StorageProvider};
use sc_service::SpawnTaskHandle;
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_arithmetic::ArithmeticError;
use sp_blockchain::HeaderBackend;
use sp_core::{RuntimeDebug, H160};
use sp_offchain::OffchainWorkerApi;
use sp_runtime::{traits::Header, transaction_validity::TransactionSource};
use std::{cmp::Ordering, marker::PhantomData, sync::Arc};
use threadpool::ThreadPool;
use xcm_runtime_apis::dry_run::runtime_decl_for_dry_run_api::DryRunApiV1;

const LOG_TARGET: &str = "liquidation-worker";

// Address of the pool address provider contract.
const PAP_CONTRACT: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));

// Account that calls the runtime API. Needs to have enough WETH balance to pay for the runtime API call.
const RUNTIME_API_CALLER: EvmAddress = H160(hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e"));

// Money market address
const BORROW_CALL_ADDRESS: EvmAddress = H160(hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38"));

// Target value of HF we try to liquidate to.
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

// Failed liquidations are suspended for this number of blocks before we try to execute them again.
const WAIT_PERIOD: BlockNumber = 10;

type HttpClient = Arc<Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>, Body>>;

/// The configuration for the liquidation worker.
/// By default, the worker is enabled and uses `PAP_CONTRACT`, `RUNTIME_API_CALLER` and `TARGET_HF` values.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct LiquidationWorkerConfig {
	/// Disable liquidation worker.
	#[clap(long, default_value = "false")]
	pub disable_liquidation_worker: bool,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pap_contract: Option<EvmAddress>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub runtime_api_caller: Option<EvmAddress>,

	/// Target health factor
	#[clap(long, default_value_t = TARGET_HF)]
	pub target_hf: u128,
}

pub struct LiquidationTask<B, C, BE, P>(PhantomData<(B, C, BE, P)>);

impl<B, C, BE, P> LiquidationTask<B, C, BE, P>
where
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B> + Erc20MappingApi<B> + LiquidationWorkerApi<B>,
	C: BlockchainEvents<B> + 'static,
	C: HeaderBackend<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
{
	/// Starting point for the liquidation worker.
	/// Executes `on_block_imported` on every block.
	/// Initial list of borrowers is fetched and sorted by the HF.
	/// `tx_waitlist` is initialized here because it's persistent between liquidation runs.
	pub async fn run(
		client: Arc<C>,
		config: LiquidationWorkerConfig,
		transaction_pool: Arc<P>,
		spawner: SpawnTaskHandle,
	) {
		// liquidation calculations are performed in a separate thread.
		let thread_pool = Arc::new(Mutex::new(ThreadPool::with_name(
			"liquidation-worker".into(),
			num_cpus::get(),
		)));

		// initialize the client once and reuse it.
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_native_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();
		let http_client: HttpClient = Arc::new(Client::builder().build(https));

		// Fetch and sort the data with borrowers info.
		let Some(borrowers_data) = Self::fetch_borrowers_data(http_client.clone()).await else {
			tracing::error!(target: LOG_TARGET, "fetch_borrowers_data failed");
			return;
		};
		let sorted_borrowers_data = Self::process_borrowers_data(borrowers_data);
		let borrowers = Arc::new(std::sync::Mutex::from(sorted_borrowers_data));

		// We store the last best block. We use it to stop older tasks.
		let best_block = Arc::new(std::sync::Mutex::from(B::Hash::default()));

		// List of liquidations that failed and are postponed to not block other possible liquidations.
		// Stored as a list of tuples: (tx_hash, block_number_when_tx_failed).
		let tx_waitlist = Arc::new(std::sync::Mutex::from(Vec::<(
			[u8; 8],
			<<B as BlockT>::Header as Header>::Number,
		)>::new()));

		// new block imported
		client
			.import_notification_stream()
			.for_each(move |n| {
				if n.is_new_best {
					spawner.spawn("liquidation-worker-on-block", Some("liquidation-worker"), {
						{
							let Ok(mut m_best_block) = best_block.lock() else {
								tracing::debug!(target: LOG_TARGET, "best_block mutex is poisoned");
								// return if the mutex is poisoned
								return ready(());
							};
							*m_best_block = n.hash;
						}
						let client_c = client.clone();
						Self::on_block_imported(
							client_c.clone(),
							spawner.clone(),
							n.hash,
							n.header,
							best_block.clone(),
							borrowers.clone(),
							tx_waitlist.clone(),
							transaction_pool.clone(),
							thread_pool.clone(),
							config,
						)
					});
				} else {
					tracing::debug!(
						target: LOG_TARGET,
						"Skipping liquidation worker for non-canon block: {:?}",
						n.header,
					)
				}

				ready(())
			})
			.await;
	}

	#[allow(clippy::too_many_arguments)]
	#[allow(clippy::type_complexity)]
	/// Main function of the liquidation worker, executed on each new block.
	/// The execution time of this function is limited to 4 seconds.
	/// Listens to new transactions and executes `on_new_transaction` on each new transaction.
	async fn on_block_imported(
		client: Arc<C>,
		spawner: SpawnTaskHandle,
		current_block_hash: B::Hash,
		header: B::Header,
		best_block_hash: Arc<std::sync::Mutex<B::Hash>>,
		borrowers: Arc<std::sync::Mutex<Vec<(H160, U256)>>>,
		tx_waitlist: Arc<std::sync::Mutex<Vec<([u8; 8], <<B as BlockT>::Header as Header>::Number)>>>,
		transaction_pool: Arc<P>,
		thread_pool: Arc<Mutex<ThreadPool>>,
		config: LiquidationWorkerConfig,
	) {
		let now = std::time::Instant::now();

		// We can ignore the result, because it's not important for us.
		// All we want is to have some upper bound for execution time of this task.
		let _ = tokio::time::timeout(std::time::Duration::from_secs(4), async {
			let runtime_api = client.runtime_api();
			let hash = header.hash();
			let has_api_v2 = runtime_api.has_api_with::<dyn OffchainWorkerApi<B>, _>(hash, |v| v == 2);

			if let Ok(true) = has_api_v2 {} else {
				tracing::error!(
							target: LOG_TARGET,
							"Unsupported Offchain Worker API version. Consider turning off offchain workers if they are not part of your runtime.",
						);
				return
			};

			let current_block_number = *header.number();

			// `tx_waitlist` maintenance.
			// Remove all transactions that are older than WAIT_PERIOD blocks and can be executed again.
			{
				let Ok(mut waitlist) = tx_waitlist.lock() else {
					tracing::debug!(target: LOG_TARGET, "tx_waitlist mutex is poisoned");
					// return if the mutex is poisoned
					return
				};

				waitlist.retain(|(_, block_num)| {
					current_block_number < *block_num + WAIT_PERIOD.into()
				});
			}

			// Get allowed signers and allowed oracle call addresses.
			// These values can be changed in the runtime, so get them on every block.
			let runtime_api = client.runtime_api();
			let hash = header.hash();
			// Accounts that sign the DIA oracle update transactions.
			let maybe_allowed_signers = runtime_api.oracle_signers(hash);
			// Addresses of the DIA oracle contract.
			let maybe_allowed_oracle_call_addresses = runtime_api.oracle_call_addresses(hash);
			let (Ok(allowed_signers), Ok(allowed_oracle_call_addresses)) = (maybe_allowed_signers, maybe_allowed_oracle_call_addresses) else { return };

            // New transaction in the transaction pool
            let mut notification_st = transaction_pool.clone().import_notification_stream();
            while let Some(notification) = notification_st.next().await {
				// If `current_block_hash != best_block_hash`, this task is most probably from previous block.
				let Ok(m_best_block_hash) = best_block_hash.lock() else {
					tracing::debug!(target: LOG_TARGET, "best_block_hash mutex is poisoned");
					// return if the mutex is poisoned
					return
				};
				if current_block_hash != *m_best_block_hash {
					// Break from the loop and end the task.
					return
				}

				match Self::on_new_transaction(
					notification,
					client.clone(),
					spawner.clone(),
					header.clone(),
					current_block_hash,
					borrowers.clone(),
					tx_waitlist.clone(),
					transaction_pool.clone(),
					thread_pool.clone(),
					allowed_signers.clone(),
					allowed_oracle_call_addresses.clone(),
					config,
				) {
					Ok(()) => continue,
					Err(()) => return,
				}
			}
		}).await;

		tracing::debug!(target: LOG_TARGET, "on_block_imported execution time: {:?}", now.elapsed().as_millis());
	}

	#[allow(clippy::too_many_arguments)]
	#[allow(clippy::type_complexity)]
	/// Executes when a new transaction is added to the transaction pool.
	/// Listens to borrow and DIA oracle update transactions.
	fn on_new_transaction(
		notification: <P as TransactionPool>::Hash,
		client: Arc<C>,
		spawner: SpawnTaskHandle,
		header: B::Header,
		current_block_hash: B::Hash,
		borrowers: Arc<std::sync::Mutex<Vec<(H160, U256)>>>,
		tx_waitlist: Arc<std::sync::Mutex<Vec<([u8; 8], <<B as BlockT>::Header as Header>::Number)>>>,
		transaction_pool: Arc<P>,
		thread_pool: Arc<Mutex<ThreadPool>>,
		allowed_signers: Vec<EvmAddress>,
		allowed_oracle_call_addresses: Vec<EvmAddress>,
		config: LiquidationWorkerConfig,
	) -> Result<(), ()> {
		// Variables used in tasks are captured by the value, so we need to clone them.
		let sorted_borrowers_data_c = borrowers.clone();

		let Some(pool_tx) = transaction_pool.clone().ready_transaction(&notification) else {
			return Err(());
		};
		let opaque_tx_encoded = pool_tx.data().encode();
		let tx = hydradx_runtime::UncheckedExtrinsic::decode(&mut &*opaque_tx_encoded);

		let Ok(transaction) = tx else { return Err(()) };

		// Listen to `borrow` transactions and add new borrowers to the list. If the borrower is already in the list, invalidate the HF by setting it to 0.
		let maybe_borrower = Self::is_borrow_transaction(transaction.0.clone());
		if let Some(borrower) = maybe_borrower {
			// Add new borrower to the list if needed.
			// If the borrower is already in the list, invalidate the HF by setting it to 0.
			match Self::process_new_borrow(borrower, sorted_borrowers_data_c.clone()) {
				// skip the execution and wait for another TX
				Ok(()) => return Ok(()),
				Err(()) => return Err(()),
			};
		}

		// Mainly listen to DIA oracle update transactions and verify the signer.
		let Some(transaction) =
			Self::verify_oracle_update_transaction(transaction.0, &allowed_signers, &allowed_oracle_call_addresses)
		else {
			tracing::debug!(target: LOG_TARGET, "verify_oracle_update_transaction failed");
			return Ok(());
		};

		Self::spawn_worker(thread_pool.clone(), move || {
			Self::process_new_oracle_update(
				transaction,
				client.clone(),
				spawner.clone(),
				header.clone(),
				current_block_hash,
				borrowers.clone(),
				tx_waitlist.clone(),
				transaction_pool.clone(),
				config,
			)
		});

		Ok(())
	}

	#[allow(clippy::too_many_arguments)]
	#[allow(clippy::type_complexity)]
	/// Executes when a new DIA oracle update transaction is added to the transaction pool.
	/// Tries to find liquidation opportunities and execute them.
	fn process_new_oracle_update(
		transaction: ethereum::TransactionV2,
		client: Arc<C>,
		spawner: SpawnTaskHandle,
		header: B::Header,
		current_block_hash: B::Hash,
		borrowers: Arc<std::sync::Mutex<Vec<(H160, U256)>>>,
		tx_waitlist: Arc<std::sync::Mutex<Vec<([u8; 8], <<B as BlockT>::Header as Header>::Number)>>>,
		transaction_pool: Arc<P>,
		config: LiquidationWorkerConfig,
	) {
		let Some(oracle_data) = parse_oracle_transaction(&transaction) else {
			tracing::debug!(target: LOG_TARGET, "parse_oracle_transaction failed");
			return;
		};

		// our calculations "happen" in the next block
		let Ok(current_evm_timestamp) = fetch_current_evm_block_timestamp::<Block, Runtime>().and_then(|timestamp| {
			timestamp
				.checked_add(primitives::constants::time::SECS_PER_BLOCK)
				.ok_or(ArithmeticError::Overflow.into())
		}) else {
			tracing::debug!(target: LOG_TARGET, "fetch_current_evm_block_timestamp failed");
			return;
		};

		// List of liquidated users in this block.
		// We don't try to liquidate a user more than once in a block.
		let mut liquidated_users: Vec<EvmAddress> = Vec::new();

		// iterate over all price updates
		// all oracle updates we are interested in are quoted in USD
		for OracleUpdataData {
			base_asset,
			quote_asset: _,
			price,
			timestamp: _,
		} in oracle_data.iter()
		{
			// TODO: maybe we can use `price` to determine if HF will increase or decrease
			let Ok(mut money_market_data) = MoneyMarketData::<Block, Runtime>::new(
				config.pap_contract.unwrap_or(PAP_CONTRACT),
				config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
			) else {
				continue;
			};

			let Ok(mut borrowers_data) = borrowers.lock() else {
				tracing::debug!(target: LOG_TARGET, "borrowers_data mutex is poisoned");
				// return if the mutex is poisoned
				return;
			};
			// Iterate over all borrowers. Borrowers are sorted by their HF, in ascending order.
			for borrower in borrowers_data.iter_mut() {
				match Self::try_liquidate(
					borrower,
					&mut liquidated_users,
					&mut money_market_data,
					current_evm_timestamp,
					base_asset,
					price,
					client.clone(),
					spawner.clone(),
					header.clone(),
					current_block_hash,
					tx_waitlist.clone(),
					transaction_pool.clone(),
					config,
				) {
					Ok(()) => (),
					Err(()) => return,
				}
			}
		}
	}

	#[allow(clippy::too_many_arguments)]
	#[allow(clippy::type_complexity)]
	/// Main liquidation logic of the worker.
	/// Submits unsigned liquidation transactions for validated liquidation opportunities.
	fn try_liquidate(
		borrower: &mut (EvmAddress, U256),
		liquidated_users: &mut Vec<EvmAddress>,
		money_market_data: &mut MoneyMarketData<Block, Runtime>,
		current_evm_timestamp: u64,
		base_asset: &[u8],
		price: &U256,
		client: Arc<C>,
		spawner: SpawnTaskHandle,
		header: B::Header,
		current_block_hash: B::Hash,
		tx_waitlist: Arc<std::sync::Mutex<Vec<([u8; 8], <<B as BlockT>::Header as Header>::Number)>>>,
		transaction_pool: Arc<P>,
		config: LiquidationWorkerConfig,
	) -> Result<(), ()> {
		let now = std::time::Instant::now();
		let current_block_number = *header.number();

		let runtime_api = client.runtime_api();
		let hash = header.hash();

		// get address of the asset whose price is about to be updated
		let Some(asset_reserve) = money_market_data
			.reserves()
			.iter()
			.find(|asset| *asset.symbol().to_ascii_lowercase() == *base_asset.to_ascii_lowercase())
		else {
			return Ok(());
		};

		let base_asset_address = asset_reserve.asset_address();

		// skip if the user has been already liquidated in this block
		if liquidated_users.contains(&borrower.0) {
			return Ok(());
		};

		let Ok(user_data) = UserData::new(
			money_market_data,
			borrower.0,
			current_evm_timestamp,
			config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER),
		) else {
			return Ok(());
		};

		if let Ok(current_hf) = user_data.health_factor(money_market_data) {
			// update user's HF
			borrower.1 = current_hf;

			if current_hf > U256::one() {
				return Ok(());
			}
		} else {
			// we were unable to get user's HF. Skip the execution for this user.
			return Ok(());
		}

		if let Ok(Some(liquidation_option)) = money_market_data.get_best_liquidation_option(
			&user_data,
			config.target_hf.into(),
			(base_asset_address, price.into()),
		) {
			let (Ok(Some(collateral_asset_id)), Ok(Some(debt_asset_id))) = (
				runtime_api.address_to_asset(hash, liquidation_option.collateral_asset),
				runtime_api.address_to_asset(hash, liquidation_option.debt_asset),
			) else {
				return Ok(());
			};

			let Ok(debt_to_liquidate) = liquidation_option.debt_to_liquidate.try_into() else {
				return Ok(());
			};

			let liquidation_tx = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
				collateral_asset: collateral_asset_id,
				debt_asset: debt_asset_id,
				user: borrower.0,
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

			let tx_hash = sp_core::blake2_64(&encoded);

			let Ok(mut waitlist) = tx_waitlist.lock() else {
				tracing::debug!(target: LOG_TARGET, "tx_waitlist mutex is poisoned");
				// return if the mutex is poisoned
				return Err(());
			};

			// skip the execution if the transaction is in the waitlist
			if waitlist.iter().find(|tx| tx.0 == tx_hash).is_some() {
				// TX is still on hold, skip the execution
				return Ok(());
			};

			// dry run to prevent spamming with extrinsics that will fail (e.g. because of not being profitable)
			let dry_run_result = Runtime::dry_run_call(hydradx_runtime::RuntimeOrigin::none().caller, liquidation_tx);

			if let Ok(call_result) = dry_run_result {
				if call_result.execution_result.is_err() {
					tracing::debug!(target: LOG_TARGET, "Dry running liquidation failed: {:?}", call_result.execution_result);

					// put the failed tx on hold for `WAIT_PERIOD` number of blocks
					waitlist.push((tx_hash, current_block_number));
					return Ok(());
				}
			}

			// There is no guarantee that the TX will be executed and with the result we expect. The HF after the execution can be slightly different than what we can predict.
			// Reset the HF to 0 so it will be recalculated again.
			borrower.1 = U256::zero();

			// add user to the list of borrowers that are liquidated in this run.
			liquidated_users.push(borrower.0);

			let tx_pool_cc = transaction_pool.clone();
			// `tx_pool::submit_one()` returns a Future type, so we need to spawn a new task
			spawner.spawn("liquidation-worker-on-submit", Some("liquidation-worker"), async move {
				tracing::debug!(target: LOG_TARGET, "Submitting liquidation extrinsic {opaque_tx:?}");
				let _ = tx_pool_cc
					.submit_one(current_block_hash, TransactionSource::Local, opaque_tx.into())
					.await;
				tracing::debug!(target: LOG_TARGET, "try_liquidate execution time: {:?}", now.elapsed().as_millis());
			});
		}

		Ok(())
	}

	// TODO: return Result type
	/// Fetch the preprocessed data used to evaluate possible candidates for liquidation.
	async fn fetch_borrowers_data(http_client: HttpClient) -> Option<BorrowerData<AccountId>> {
		let url = ("https://omniwatch.play.hydration.cloud/api/borrowers/by-health")
			.parse()
			.ok()?;
		let res = http_client.get(url).await.ok()?;
		if res.status() != StatusCode::OK {
			tracing::debug!(target: LOG_TARGET, "failed to fetch borrowers data");
			return None;
		}

		let bytes = hyper::body::to_bytes(res.into_body()).await.ok()?;

		let data = String::from_utf8(bytes.to_vec()).ok()?;
		let data = data.as_str();
		let data = serde_json::from_str::<BorrowerData<AccountId>>(data);
		data.ok()
	}

	/// Returns borrowers sorted by HF.
	/// The list ir sorted in ascending order, starting with borrowers whose HF has not yet been
	/// calculated (HF==0).
	pub fn process_borrowers_data(oracle_data: BorrowerData<AccountId>) -> Vec<(H160, U256)> {
		let one = U256::from(10u128.pow(18));
		let fractional_multiplier = U256::from(10u128.pow(12));
		let mut borrowers = oracle_data
			.borrowers
			.iter()
			.map(|b| {
				// I'm not aware of a better way to convert f32 to U256. Use this naive approach and
				// take first 6 decimals. That should be enough for our purpose.
				let integer_part = U256::from(b.1.health_factor.trunc() as u128).checked_mul(one);
				let fractional_part =
					U256::from((b.1.health_factor.fract() * 1_000_000f32) as u128).checked_mul(fractional_multiplier);
				// return 0 if the computation failed, and recalculate the HF later.
				let hf = integer_part
					.zip(fractional_part)
					.and_then(|(i, f)| i.checked_add(f))
					.unwrap_or_default();
				(b.0, hf)
			})
			.collect::<Vec<_>>();
		// sort by HF
		borrowers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

		borrowers
	}

	/// Spawns a new thread for liquidation worker.
	///
	/// We spawn liquidation workers for each oracle update in a separate thread,
	/// since they can run for a significant amount of time
	/// in a blocking fashion and we don't want to block the runtime.
	fn spawn_worker(thread_pool: Arc<Mutex<ThreadPool>>, f: impl FnOnce() + Send + 'static) {
		thread_pool.lock().execute(f);
	}

	/// Check if the provided transaction is valid DIA oracle update.
	/// All Ethereum transaction types are supported.
	fn verify_oracle_update_transaction(
		extrinsic: sp_runtime::generic::UncheckedExtrinsic<
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

	/// Check if the provided transaction is money market borrow.
	/// All Ethereum transaction types are supported.
	fn is_borrow_transaction(
		extrinsic: sp_runtime::generic::UncheckedExtrinsic<
			hydradx_runtime::Address,
			RuntimeCall,
			hydradx_runtime::Signature,
			hydradx_runtime::SignedExtra,
		>,
	) -> Option<H160> {
		if let RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = extrinsic.function.clone() {
			let action = match transaction {
				Transaction::Legacy(legacy_transaction) => legacy_transaction.action,
				Transaction::EIP2930(eip2930_transaction) => eip2930_transaction.action,
				Transaction::EIP1559(eip1559_transaction) => eip1559_transaction.action,
			};

			// check if the transaction is MM borrow
			if let pallet_ethereum::TransactionAction::Call(call_address) = action {
				if call_address == BORROW_CALL_ADDRESS {
					if let Some(Ok(borrower)) = extrinsic.function.check_self_contained() {
						return Some(borrower);
					};
				};
			};
		};

		None
	}

	/// Adds a new borrower to the borrowers list.
	/// If the borrower is already in the list, invalidates the HF by setting it to 0 so the HF will be recalculated.
	fn process_new_borrow(
		borrower: EvmAddress,
		borrowers_list_mutex: Arc<std::sync::Mutex<Vec<(H160, U256)>>>,
	) -> Result<(), ()> {
		// lock is automatically dropped at the end of this function
		let Ok(mut borrowers_data) = borrowers_list_mutex.lock() else {
			tracing::debug!(target: LOG_TARGET, "borrowers_data mutex is poisoned");
			// return if the mutex is poisoned
			return Err(());
		};

		let maybe_existing_borrower = borrowers_data.iter().position(|b| b.0 == borrower);
		if let Some(index) = maybe_existing_borrower {
			// borrower is already in the list. Invalidate the HF by setting it to 0.
			borrowers_data[index] = (borrower, U256::zero());
		} else {
			// add new borrower to the list. HF is set to 0, so we can place it at the beginning and the list will remain sorted.
			borrowers_data.insert(0, (borrower, U256::zero()));
		}

		Ok(())
	}
}

/// The data from DIA oracle update transaction.
#[derive(Eq, PartialEq, Clone, RuntimeDebug)]
pub struct OracleUpdataData {
	base_asset: Vec<u8>,
	quote_asset: Vec<u8>,
	price: U256,
	timestamp: U256,
}
impl OracleUpdataData {
	pub fn new(base_asset: Vec<u8>, quote_asset: Vec<u8>, price: U256, timestamp: U256) -> Self {
		Self {
			base_asset,
			quote_asset,
			price,
			timestamp,
		}
	}
}

/// Parse DIA oracle update transaction.
/// All Ethereum transaction types are supported.
/// Returns a list of `OracleUpdateData`.
pub fn parse_oracle_transaction(eth_tx: &Transaction) -> Option<Vec<OracleUpdataData>> {
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
				let price = U256::from_little_endian(&price_and_timestamp.encode_as()[16..32]);
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
			.collect::<Vec<Vec<u8>>>();
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

// setValue(string key, uint128 value, uint128 timestamp)
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
