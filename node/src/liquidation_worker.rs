use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use fp_self_contained::SelfContainedCall;
use frame_support::BoundedVec;
use frame_support::__private::sp_tracing::tracing;
use futures::{future::ready, StreamExt};
use hex_literal::hex;
use hydradx_runtime::{evm::precompiles::erc20_mapping::HydraErc20Mapping, Block, Runtime, RuntimeCall};
use hydradx_traits::evm::{Erc20Encoding, EvmAddress};
use hyper::{body::Body, Client, StatusCode};
use hyperv14 as hyper;
use liquidation_worker_support::*;
use pallet_ethereum::Transaction;
use parking_lot::Mutex;
use polkadot_primitives::EncodeAs;
use primitives::AccountId;
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

pub const LOG_TARGET: &str = "liquidation-worker";
const PAP_CONTRACT: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691")); // TODO: verify
const RUNTIME_API_CALLER: EvmAddress = H160(hex!("82db570265c37be24caf5bc943428a6848c3e9a6")); // TODO: verify
const ORACLE_UPDATE_CALLER: EvmAddress = H160(hex!("ff0c624016c873d359dde711b42a2f475a5a07d3"));
const ORACLE_UPDATE_CALL_ADDRESS: EvmAddress = H160(hex!("48ae7803cd09c48434e3fc5629f15fb76f0b5ce5"));
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

pub type HttpClient = Arc<Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>, Body>>;

/// The configuration for the liquidation worker.
/// By default, the worker is enabled and uses `MAX_LIQUIDATIONS`, `PAP_CONTRACT`,
/// `RUNTIME_API_CALLER` and `TARGET_HF` values.
#[derive(Clone, Copy, Debug, clap::Parser)]
pub struct LiquidationWorkerConfig {
	/// Maximum number of accounts we try to liquidate.
	#[clap(long, default_value = "5")]
	pub max_liquidations: u8,

	/// Disable liquidation worker.
	#[clap(long, default_value = "false")]
	pub disable_liquidation_worker: bool,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pap_contract: Option<EvmAddress>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub runtime_api_caller: Option<EvmAddress>,

	/* TODO: would be nice if we can also configure oracle_update_caller and oracle_update_call_address,
	 * but these are hardcoded in the runtime in validate_self_contained.
	 */
	// /// EVM address of the account that signs DIA oracle update.
	// #[clap(long)]
	// pub oracle_update_caller: Option<EvmAddress>,
	//
	// /// EVM address of the DIA oracle update call address.
	// #[clap(long)]
	// pub oracle_update_call_address: Option<EvmAddress>,
	/// Target health factor
	#[clap(long, default_value_t = TARGET_HF)]
	pub target_hf: u128,
}

// TODO: maybe use struct with data:
// pub struct LiquidationTask<B, C, BE, P> {
//     client: Arc<C>,
//     transaction_pool: Arc<P>,
//     spawner: SpawnTaskHandle,
//     _phantom: PhantomData<(B, BE)>,
// }
pub struct LiquidationTask<B, C, BE, P>(PhantomData<(B, C, BE, P)>);

impl<B, C, BE, P> LiquidationTask<B, C, BE, P>
where
	B: BlockT,
	C: ProvideRuntimeApi<B>,
	C::Api: EthereumRuntimeRPCApi<B>,
	C: BlockchainEvents<B> + 'static,
	C: HeaderBackend<B> + StorageProvider<B, BE>,
	BE: Backend<B> + 'static,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
{
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

		// We store the last best block. We use it to stop older tasks.
		let best_block = Arc::new(std::sync::Mutex::from(B::Hash::default()));

		// new block imported
		client
			.import_notification_stream()
			.for_each(move |n| {
				if n.is_new_best {
					spawner.spawn("liquidation-worker-on-block", Some("liquidation-worker"), {
						{
							let Ok(mut m_best_block) = best_block.lock() else {
								return ready(());
							}; // return if the mutex is poisoned
							*m_best_block = n.hash;
						}
						let client_c = client.clone();
						Self::on_block_imported(
							client_c.clone(),
							spawner.clone(),
							n.hash,
							n.header,
							best_block.clone(),
							http_client.clone(),
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
	async fn on_block_imported(
		client: Arc<C>,
		spawner: SpawnTaskHandle,
		current_block_hash: B::Hash,
		header: B::Header,
		best_block_hash: Arc<std::sync::Mutex<B::Hash>>,
		http_client: HttpClient,
		transaction_pool: Arc<P>,
		thread_pool: Arc<Mutex<ThreadPool>>,
		config: LiquidationWorkerConfig,
	) {
		// We can ignore the result, because it's not important for us.
		// All we want is to have some upper bound for execution time of this task.
		let _ = tokio::time::timeout(std::time::Duration::from_secs(4), async {
            // Fetch the data with borrowers info.
            // We don't need to set a deadline, because it's wrapped in a task with a deadline.
            let Some(borrowers_data) = Self::fetch_borrowers_data(http_client.clone()).await else {
				tracing::error!(target: LOG_TARGET, "fetch_borrowers_data failed");
				return
			};
            let sorted_borrowers_data = Self::process_borrowers_data(borrowers_data, config.max_liquidations);

            // New transaction in the transaction pool
            let mut notification_st = transaction_pool.clone().import_notification_stream();
            while let Some(notification) = notification_st.next().await {
				let now = std::time::Instant::now();

				// If `current_block_hash != best_block_hash`, this task is most probably from previous block.
				let Ok(m_best_block_hash) = best_block_hash.lock() else { return }; // return if the mutex is poisoned
				if current_block_hash != *m_best_block_hash {
					// Break from the loop and end the task.
					break
				}

                // Variables used in tasks are captured by the value, so we need to clone them.
                let tx_pool = transaction_pool.clone();
                let spawner_c = Arc::new(spawner.clone());
                let header_c = Arc::new(header.clone());
				let client_c = client.clone();
				let sorted_borrowers_data_c = sorted_borrowers_data.clone();

                let Some(pool_tx) = transaction_pool.clone().ready_transaction(&notification) else { return };
				let opaque_tx_encoded = pool_tx.data().encode();
				let tx = hydradx_runtime::UncheckedExtrinsic::decode(&mut &*opaque_tx_encoded);

				let Ok(transaction) = tx else { return };

				let tx_pool_c = tx_pool.clone();

				let Some(transaction) = Self::verify_oracle_update_transaction(transaction.0)
					else {
						tracing::debug!(target: LOG_TARGET, "parse_oracle_transaction failed");
						return
					};

				Self::spawn_worker(thread_pool.clone(), move || {
					let runtime_api = client_c.runtime_api();
					let hash = header_c.hash();
					let has_api_v2 = runtime_api.has_api_with::<dyn OffchainWorkerApi<B>, _>(hash, |v| v == 2);

					if let Ok(true) = has_api_v2 {} else {
						tracing::error!(
							target: LOG_TARGET,
							"Unsupported Offchain Worker API version. Consider turning off offchain workers if they are not part of your runtime.",
						);
						return
					};

					let Some(oracle_data) = parse_oracle_transaction(transaction) else {
						tracing::debug!(target: LOG_TARGET, "parse_oracle_transaction failed");
						return
					};

					// iterate over all price updates
					// all oracle updates we are interested in are quoted in USD
					for OracleUpdataData{base_asset, quote_asset: _, price, timestamp: _} in oracle_data.iter() {
						// TODO: maybe we can use `price` to determine if HF will increase or decrease
						let Ok(mut money_market_data) = MoneyMarketData::<Block, Runtime>::new(config.pap_contract.unwrap_or(PAP_CONTRACT), config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER))
							else { return };

						// our calculations "happen" in the next block
						let Ok(current_evm_timestamp) = fetch_current_evm_block_timestamp::<Block, Runtime>()
							.and_then(|timestamp| timestamp.checked_add(primitives::constants::time::SECS_PER_BLOCK)
							.ok_or(ArithmeticError::Overflow.into())) else { return };

						// get address of the asset whose price is about to be updated
						let Some(asset_reserve) = money_market_data.reserves().iter().find(|asset| *asset.symbol().to_ascii_lowercase() == *base_asset.to_ascii_lowercase())
							else { return };

						let base_asset_address = asset_reserve.asset_address();

						// iterate over all borrowers
						for borrower in sorted_borrowers_data_c.iter() {
							let Ok(user_data) = UserData::new(&money_market_data, borrower.0, current_evm_timestamp, config.runtime_api_caller.unwrap_or(RUNTIME_API_CALLER))
								else { return };

							if let Ok(Some(liquidation_option)) = money_market_data
								.get_best_liquidation_option(&user_data, config.target_hf.into(), (base_asset_address, price.into())) {

								let (Some(collateral_asset_id), Some(debt_asset_id)) =
									(HydraErc20Mapping::decode_evm_address(liquidation_option.collateral_asset),
									 HydraErc20Mapping::decode_evm_address(liquidation_option.debt_asset))
									else {
										continue
									};

								let Ok(debt_to_liquidate) = liquidation_option.debt_to_liquidate.try_into()
									else {
										continue
									};

								let liquidation_tx = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_unsigned {
									collateral_asset: collateral_asset_id,
									debt_asset: debt_asset_id,
									user: borrower.0,
									debt_to_cover: debt_to_liquidate,
									route: BoundedVec::new(),
								});


								// dry run to prevent spamming with extrinsics that will fail (e.g. because of not being profitable)
								let dry_run_result = Runtime::dry_run_call(
									hydradx_runtime::RuntimeOrigin::none().caller,
									liquidation_tx.clone());

								if let Ok(call_result) = dry_run_result {
									if call_result.execution_result.is_err() {
										tracing::debug!(target: LOG_TARGET, "Dry running liquidation failed: {:?}", call_result.execution_result);
										continue
									}
								}

								let encoded_tx: fp_self_contained::UncheckedExtrinsic<hydradx_runtime::Address, RuntimeCall, hydradx_runtime::Signature, hydradx_runtime::SignedExtra> =
									fp_self_contained::UncheckedExtrinsic::new_unsigned(liquidation_tx);
								let encoded = encoded_tx.encode();
								let opaque_tx = sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid");

								let tx_pool_cc = tx_pool_c.clone();
								// `tx_pool::submit_one()` returns a Future type, so we need to spawn a new task
								spawner_c.spawn(
									"liquidation-worker-on-submit",
									Some("liquidation-worker"),
									async move {
										tracing::debug!(target: LOG_TARGET, "Submitting liquidation extrinsic {opaque_tx:?}");
										let _ = tx_pool_cc.submit_one(current_block_hash, TransactionSource::Local, opaque_tx.into()).await;
										tracing::debug!(target: LOG_TARGET, "Liquidation execution time: {:?}", now.elapsed().as_millis());
									}
								);
							}
						}
					}
				});
			}
		}).await;
	}

	// TODO: return Result type
	/// Fetch the preprocessed data used to evaluate possible candidates for liquidation.
	async fn fetch_borrowers_data(http_client: HttpClient) -> Option<BorrowerData<AccountId>> {
		let url = ("https://omniwatch.play.hydration.cloud/api/borrowers/by-health")
			.parse()
			.ok()?;
		let res = http_client.get(url).await.ok()?;
		assert_eq!(res.status(), StatusCode::OK);

		let bytes = hyper::body::to_bytes(res.into_body()).await.ok()?;

		let data = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");
		let data = data.as_str();
		let data = serde_json::from_str::<BorrowerData<AccountId>>(data);
		data.ok()
	}

	/// Returns borrowers sorted by HF.
	/// Maximum size of the returned list is `MAX_LIQUIDATIONS`.
	pub fn process_borrowers_data(
		oracle_data: BorrowerData<AccountId>,
		max_liquidations: u8,
	) -> Vec<(H160, BorrowerDataDetails<AccountId>)> {
		let mut borrowers = oracle_data.borrowers.clone();
		// remove elements with HF == 0
		borrowers.retain(|b| b.1.health_factor > 0.0);
		borrowers.sort_by(|a, b| {
			a.1.health_factor
				.partial_cmp(&b.1.health_factor)
				.unwrap_or(Ordering::Equal)
		});
		borrowers.truncate(borrowers.len().min(max_liquidations as usize));
		borrowers
	}

	/// Spawns a new offchain worker.
	///
	/// We spawn offchain workers for each block in a separate thread,
	/// since they can run for a significant amount of time
	/// in a blocking fashion and we don't want to block the runtime.
	///
	/// Note that we should avoid that if we switch to future-based runtime in the future,
	/// alternatively:
	fn spawn_worker(thread_pool: Arc<Mutex<ThreadPool>>, f: impl FnOnce() + Send + 'static) {
		thread_pool.lock().execute(f);
	}

	/// Check if the provided transaction is valid DIA oracle update.
	fn verify_oracle_update_transaction(
		extrinsic: sp_runtime::generic::UncheckedExtrinsic<
			hydradx_runtime::Address,
			RuntimeCall,
			hydradx_runtime::Signature,
			hydradx_runtime::SignedExtra,
		>,
	) -> Option<Transaction> {
		if let RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = extrinsic.function.clone() {
			if let Transaction::Legacy(legacy) = transaction.clone() {
				// check if the transaction is DIA oracle update
				if let pallet_ethereum::TransactionAction::Call(call_address) = legacy.action {
					if call_address == ORACLE_UPDATE_CALL_ADDRESS {
						// additional check to prevent running the worker for DIA oracle updates signed by invalid address
						if extrinsic.function.check_self_contained() == Some(Ok(ORACLE_UPDATE_CALLER)) {
							return Some(transaction);
						};
					};
				};
			};
		};

		None
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
/// Return a list of `OracleUpdateData`.
pub fn parse_oracle_transaction(eth_tx: Transaction) -> Option<Vec<OracleUpdataData>> {
	let legacy_transaction = match eth_tx {
		Transaction::Legacy(legacy_transaction) => legacy_transaction,
		_ => return None,
	};

	let mut dia_oracle_data = Vec::new();

	let fn_selector = &legacy_transaction.input[0..4];
	// setValue
	if fn_selector == hex!("7898e0c2") {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::String,
				ethabi::ParamType::Uint(16),
				ethabi::ParamType::Uint(16),
			],
			&legacy_transaction.input[4..], // first 4 bytes are function selector
		)
		.ok()?;

		dia_oracle_data.push((
			decoded[0].clone().into_string()?,
			decoded[1].clone().into_uint()?,
			decoded[2].clone().into_uint()?,
		));
	}
	// setMultipleValues
	else if fn_selector == hex!("8d241526") {
		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
			],
			&legacy_transaction.input[4..], // first 4 bytes are function selector
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
		assert_eq!(expected, parse_oracle_transaction(tx).unwrap());

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
		assert_eq!(expected, parse_oracle_transaction(tx).unwrap());
	}
}
