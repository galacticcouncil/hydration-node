use std::marker::PhantomData;
use std::sync::{Arc};
use std::hash::Hash;
use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use ethabi::ethereum_types::U256;
use fp_rpc::EthereumRuntimeRPCApi;
use frame_support::__private::sp_tracing::tracing;
use futures::future::ready;
use sc_client_api::{Backend, BlockchainEvents, StorageProvider};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use futures::StreamExt;
use sc_transaction_pool_api::{TransactionPool, InPoolTransaction};
use hex_literal::hex;
use sp_core::{offchain, H160, H256};
use hyperv14 as hyper;
use hyper::{body::Body, Client, StatusCode};
use pallet_ethereum::Transaction;
use sc_service::SpawnTaskHandle;
use threadpool::ThreadPool;
use parking_lot::Mutex;
use polkadot_primitives::EncodeAs;
use sp_offchain::OffchainWorkerApi;
use sp_runtime::transaction_validity::TransactionSource;
use sp_runtime::traits::{CheckedConversion, Header};
use hydradx_traits::router::AssetPair;
use pallet_liquidation::{AssetId, Config};
use pallet_liquidation_rpc_runtime_api::LiquidationApi;

const LOG_TARGET: &str = "offchain-worker";

pub type HttpClient = Arc<Client<hyper_rustls::HttpsConnector<hyper::client::HttpConnector>, Body>>;
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
        transaction_pool: Arc<P>,
        spawner: SpawnTaskHandle,
    ) {
        // liquidation calculations are performed in a separate thread.
        let thread_pool = Arc::new(Mutex::new(ThreadPool::with_name(
        "offchain-worker".into(),
        num_cpus::get(), // TODO
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
                    log::info!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - 1 new best block");
                    spawner.spawn(
                        "offchain-on-block",
                        Some("offchain-worker"),
                        {
                            {
                                let mut m_best_block = best_block.lock().unwrap();
                                *m_best_block = n.hash;
                            }
                            let client_c = client.clone();
                            Self::on_block_imported(client_c.clone(), spawner.clone(), n.hash, n.header, best_block.clone(), http_client.clone(), transaction_pool.clone(), thread_pool.clone())
                        }
                    );


                } else {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "Skipping offchain workers for non-canon block: {:?}",
                        n.header,
                    )
                }

                ready(())
        })
        .await;
    }

    async fn on_block_imported(client: Arc<C>, spawner: SpawnTaskHandle, current_block_hash: B::Hash, header: B::Header, best_block_hash: Arc<std::sync::Mutex<B::Hash>>, http_client: HttpClient, transaction_pool: Arc<P>, thread_pool: Arc<Mutex<ThreadPool>>) {


        // We can ignore the result, because it's not important for us.
        // All we want is to have some upper bound for execution time of this task.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(8), async {
            log::info!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - 2 on block imported");
            // Fetch the data with borrowers info.
            // We don't need to set a deadline, because it's wrapped in a task with a deadline.
            let Some(data) = Self::fetch_data(http_client.clone()).await else { return };
            let processed_data = pallet_liquidation::Pallet::<hydradx_runtime::Runtime>::process_borrowers_data(data);

            // New transaction in the transaction pool
            let mut notification_st = transaction_pool.clone().import_notification_stream();
            while let Some(notification) = notification_st.next().await {
                log::info!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - 3 new transaction");
                {
                    // If `current_block_hash != best_block_hash`, this task is most probably from previous block.
                    let m_best_block_hash = best_block_hash.lock().unwrap();
                    if current_block_hash != *m_best_block_hash {
                        log::info!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - OLD BLOCK, BYE BYE");
                        // Break from the loop and end the task.
                        break
                    }
                }

                // Variables used in tasks are captured by the value, so we need to clone them.
                let tx_pool = transaction_pool.clone();
                let spawnerc = Arc::new(spawner.clone());
                let spawnerc_c = Arc::new(spawner.clone());
                let hashc = Arc::new(current_block_hash.clone());
                let headerc = Arc::new(header.clone());

                let maybe_transaction = transaction_pool.clone().ready_transaction(&notification);
                if let Some(tx) = maybe_transaction {
                    let opaque_tx_encoded = tx.data().encode();
                    let tx = hydradx_runtime::UncheckedExtrinsic::decode(&mut &*opaque_tx_encoded);

                    if let Ok(transaction) = tx {
                        let transaction = transaction.0;
                        let tx_pool_c = tx_pool.clone();

                        if let hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::dummy_received { debt_to_cover }) = transaction.function {

                            log::info!("liquidation worker transaction inner: {:?}", transaction);
                            let client_c = client.clone();
                            Self::spawn_worker(thread_pool.clone(), move || {
                                log::info!("- - - - - - - - - - - - - - -liquidation worker THREAD START");


                                let runtime_api = client_c.runtime_api();
                                let hash = headerc.hash();
                                let has_api_v2 = runtime_api.has_api_with::<dyn OffchainWorkerApi<B>, _>(hash, |v| v == 2);
                                if let Ok(true) = has_api_v2 {} else {
                                    tracing::error!(
                                        target: LOG_TARGET,
                                        "Unsupported Offchain Worker API version. Consider turning off offchain workers if they are not part of your runtime.",
                                    );
                                    return
                                };

                                let transaction = dummy_dia_tx();
                                let Some(signer) = Self::recover_signer(&transaction) else {
                                    log::info!("- - - - - - - - - - - - - - -liquidation worker recover signer failed");
                                    return };

                                log::info!("- - - - - - - - - - - - - - -liquidation worker SIGNER {:?}", signer);

                                let Some(oracle_data) = parse_oracle_transaction(transaction) else {
                                    log::info!("- - - - - - - - - - - - - - -liquidation worker parse_oracle_transaction failed");
                                    return };
                                log::info!("- - - - - - - - - - - - - - -liquidation worker DIA DATA {:?}", oracle_data);

                                for (asset_pair_str, price, timestamp) in oracle_data.iter() {
                                    let base_fee = runtime_api.gas_price(hash).unwrap_or_default();

                                    // let from = H160::default();
                                    // let to = H160::default();
                                    // let data = Vec::new();
                                    // let value = U256::default();
                                    // let gas_limit = U256::default();
                                    // let res = runtime_api.call(hash, from, to, data, value, gas_limit, None, None, None, false, None);
                                    // log::info!("- - - - - - - - - - - - - - -liquidation worker EVM CALL {:?}", res);

                                    // let liquidation_tx = hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_unsigned {collateral_asset: 0, debt_asset: 1, user: H160::zero(), debt_to_cover: 1_000_000, route: vec![]});
                                    let liquidation_tx = hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::dummy_send { debt_to_cover });
                                    let encoded_tx: fp_self_contained::UncheckedExtrinsic<hydradx_runtime::Address, hydradx_runtime::RuntimeCall, hydradx_runtime::Signature, hydradx_runtime::SignedExtra> = fp_self_contained::UncheckedExtrinsic::new_unsigned(liquidation_tx);
                                    let encoded = encoded_tx.encode();
                                    let opaque_tx = sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid");

                                    let tx_pool_cc = tx_pool_c.clone();
                                    spawnerc_c.spawn(
                                        "offchain-on-block",
                                        Some("offchain-worker"),
                                        async move {
                                            let res = tx_pool_cc.submit_one(current_block_hash, TransactionSource::Local, opaque_tx.into()).await;
                                            log::info!("- - - - - - - - - - - - - - -liquidation worker SUBMIT {:?}", res);
                                        }
                                    );
                                }

                                log::info!("- - - - - - - - - - - - - - -liquidation worker THREAD END");
                            });
                        }


                        // if let hydradx_runtime::RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = transaction.function {
                        //     if let Transaction::Legacy(legacy) = transaction.clone() {
                        //         if let pallet_ethereum::TransactionAction::Call(call_address) = legacy.action {
                        //             if call_address == H160::from_slice(hex!("3cd0a705a2dc65e5b1e1205896baa2be8a07c6e0").as_slice())
                        //                 || call_address
                        //                 == H160::from_slice(hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice())
                        //                 || call_address
                        //                 == H160::from_slice(hex!("0000000000000000000000000000000100000000").as_slice())
                        //             {
                        //                 log::info!("liquidation worker transaction inner: {:?}", transaction);
                        //                 Self::spawn_worker(thread_pool.clone(), move || {
                        //                     log::info!("- - - - - - - - - - - - - - -liquidation worker THREAD START");
                        //                     let Some(signer) = Self::recover_signer(&transaction) else {
                        //                         log::info!("- - - - - - - - - - - - - - -liquidation worker recover signer failed");
                        //                         return };
                        //
                        //                     log::info!("- - - - - - - - - - - - - - -liquidation worker SIGNER {:?}", signer);
                        //                     let transaction = dummy_dia_tx();
                        //                     let Some(oracle_data) = pallet_liquidation::Pallet::<hydradx_runtime::Runtime>::parse_oracle_transaction(transaction) else {
                        //                         log::info!("- - - - - - - - - - - - - - -liquidation worker parse_oracle_transaction failed");
                        //                         return };
                        //                     log::info!("- - - - - - - - - - - - - - -liquidation worker DIA DATA {:?}", oracle_data);
                        //
                        //                     let liquidation_tx = hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_unsigned {collateral_asset: 0, debt_asset: 1, user: H160::zero(), debt_to_cover: 1_000_000, route: vec![]});
                        //                     let encoded_tx: fp_self_contained::UncheckedExtrinsic<hydradx_runtime::Address, hydradx_runtime::RuntimeCall, hydradx_runtime::Signature, hydradx_runtime::SignedExtra> = fp_self_contained::UncheckedExtrinsic::new_unsigned(liquidation_tx);
                        //                     let encoded = encoded_tx.encode();
                        //                     let opaque_tx = sp_runtime::OpaqueExtrinsic::decode(&mut &encoded[..]).expect("Encoded extrinsic is always valid");
                        //
                        //                     spawnerc_c.spawn(
                        //                         "offchain-on-block",
                        //                         Some("offchain-worker"),
                        //                         async move {
                        //                             let res = tx_pool_c.submit_one(current_block_hash, TransactionSource::Local, opaque_tx.into()).await;
                        //                             log::info!("- - - - - - - - - - - - - - -liquidation worker SUBMIT {:?}", res);
                        //                         }
                        //                     );
                        //                     log::info!("- - - - - - - - - - - - - - -liquidation worker THREAD END");
                        //                 });
                        //             }
                        //         };
                        //     };
                        // };

                    };
                };
            }
        }).await;
        log::info!("- - - - - - - - - - - - - - - - - - - - - - - - - - - - 9999 BYE BYE end on block imported");
    }

    async fn fetch_data(http_client: HttpClient) -> Option<pallet_liquidation::BorrowerData<hydradx_runtime::AccountId>> {

        let url = ("https://omniwatch.play.hydration.cloud/api/borrowers/by-health").parse().unwrap();
        let res = http_client.get(url).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let data = String::from_utf8(bytes.to_vec()).expect("response was not valid utf-8");
        let data = data.as_str();
        let data = serde_json::from_str::<pallet_liquidation::BorrowerData<hydradx_runtime::AccountId>>(&data);
        data.ok()
    }

    /// Spawns a new offchain worker.
    ///
    /// We spawn offchain workers for each block in a separate thread,
    /// since they can run for a significant amount of time
    /// in a blocking fashion and we don't want to block the runtime.
    ///
    /// Note that we should avoid that if we switch to future-based runtime in the future,
    /// alternatively:
    fn spawn_worker(thread_pool: Arc<Mutex<ThreadPool>>, f: impl FnOnce() -> () + Send + 'static) {
        thread_pool.lock().execute(f);
    }

    fn recover_signer(transaction: &Transaction) -> Option<H160> {
        let mut sig = [0u8; 65];
        let mut msg = [0u8; 32];
        match transaction {
            Transaction::Legacy(t) => {
                sig[0..32].copy_from_slice(&t.signature.r()[..]);
                sig[32..64].copy_from_slice(&t.signature.s()[..]);
                sig[64] = t.signature.standard_v();
                msg.copy_from_slice(
                    &pallet_ethereum::LegacyTransactionMessage::from(t.clone()).hash()[..],
                );
            }
            _ => return None,
        }
        let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg).ok()?;
        Some(H160::from(H256::from(sp_io::hashing::keccak_256(&pubkey))))
    }
}

fn dummy_dia_tx() -> Transaction {
    Transaction::Legacy(ethereum::LegacyTransaction {
        nonce: U256::from(9264),
        gas_price: U256::from(5143629),
        gas_limit: U256::from(80674),
        action: pallet_ethereum::TransactionAction::Call(H160::from_slice(
            hex!("5d8320f3ced9575d8e25b6f437e610fc6a03bf52").as_slice(),
        )),
        value: U256::from(0), // 0x40	= 64	/ 120 = 288 / 80 = 128
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

pub fn parse_oracle_transaction(eth_tx: pallet_ethereum::Transaction) -> Option<Vec<(String, U256, U256)>> {
    let legacy_transaction = match eth_tx {
        pallet_ethereum::Transaction::Legacy(legacy_transaction) => legacy_transaction,
        _ => return None,
    };

    let decoded = ethabi::decode(
        &[
            ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
            ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
        ],
        &legacy_transaction.input[4..],// first 4 bytes are function selector
    ).ok()?;

    let mut dia_oracle_data = Vec::new();

    if decoded.len() == 2 {
        for (asset_str, price) in sp_std::iter::zip(decoded[0].clone().into_array()?.iter(), decoded[1].clone().into_array()?.iter()) {
            dia_oracle_data.push((asset_str.clone().into_string()?, price.clone().into_uint()?));

        }
    };

    let mut result = Vec::new();
    for (asset_str, data) in dia_oracle_data.iter() {
       let data_vec = data.clone().encode_as();
        let price = U256::from_little_endian(&data_vec[0..16]);
        let timestamp = U256::from_little_endian(&data_vec[16..32]);
        result.push((asset_str.clone(), price, timestamp));
    }

    Some(result)
}

#[test]
fn parse_oracle_transaction_should_work() {
    let tx = dummy_dia_tx();
    let result = parse_oracle_transaction(tx);
    println!("{:?}", result);
}

// pub struct LiquidationTask<B, C, BE, P> {
//     client: Arc<C>,
//     transaction_pool: Arc<P>,
//     spawner: SpawnTaskHandle,
//     _phantom: PhantomData<(B, BE)>,
// }
//
// impl<B, C, BE, P> LiquidationTask<B, C, BE, P>
// where
//     B: BlockT,
//     C: ProvideRuntimeApi<B> + 'static,
//     C::Api: EthereumRuntimeRPCApi<B>,
//     C: BlockchainEvents<B> + 'static,
//     C: HeaderBackend<B> + StorageProvider<B, BE>,
//     BE: Backend<B> + 'static,
//     P: TransactionPool,
// {
// }
