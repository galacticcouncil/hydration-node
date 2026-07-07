use crate::*;

// Live smoke test: hits the real omniwatch endpoint over the network, so it is
// #[ignore]d by default (flaky, and the borrower count is non-deterministic). Run it
// manually with `cargo test -p pepl-worker -- --ignored` to check omniwatch
// reachability. Deterministic failure-mode coverage is in the tests below, against a local mock.
#[tokio::test]
#[ignore = "hits live omniwatch over the network; run with --ignored"]
async fn fetch_borrowers_list_should_return_borrowers_when_omniwatch_reachable() {
	let https = https::new();

	let url = OMNIWATCH_URL.parse().expect("OMNIWATCH_URL to be valid");
	let borrowers = fetch_borrowers_list(&https, url, "test")
		.await
		.expect("fetch borrowers from omniwatch to work");

	assert!(!borrowers.is_empty());
}

// A down/unreachable omniwatch must return `None` (never panic). Port 1 is not listening, so
// the connection is refused immediately — deterministic, no network dependency.
#[tokio::test]
async fn fetch_borrowers_list_should_return_none_when_endpoint_unreachable() {
	let https = https::new();
	let url = "http://127.0.0.1:1/borrowers".parse().expect("valid uri");

	assert_eq!(fetch_borrowers_list(&https, url, "test").await, None);
}

// On total fetch failure the retry wrapper returns `None` (never panics), so the worker
// starts unseeded and keeps running — event discovery still adds borrowers and background
// re-seeding recovers the seed.
#[tokio::test]
async fn fetch_borrowers_list_with_retry_should_return_none_when_unreachable() {
	let https = https::new();
	let url = "http://127.0.0.1:1/borrowers".parse().expect("valid uri");

	let borrowers = fetch_borrowers_list_with_retry(
		&https,
		url,
		"test",
		2,
		std::time::Duration::from_millis(1),
		std::time::Duration::from_millis(200),
	)
	.await;

	assert_eq!(borrowers, None);
}

// An omniwatch that accepts the TCP connection but never sends an HTTP response must not hang
// the worker — every fetch attempt is bounded by a timeout. The listener is bound but never
// accepts: the kernel completes the handshake (backlog), so the request is sent and then hangs.
#[tokio::test]
async fn fetch_borrowers_list_with_retry_should_return_none_when_endpoint_hangs() {
	let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
	let addr = listener.local_addr().expect("local addr");

	let https = https::new();
	let url = format!("http://{addr}/borrowers").parse().expect("valid uri");

	let borrowers = fetch_borrowers_list_with_retry(
		&https,
		url,
		"test",
		2,
		std::time::Duration::from_millis(1),
		std::time::Duration::from_millis(200),
	)
	.await;

	assert_eq!(borrowers, None);
	drop(listener);
}

// Event-driven discovery — a BORROW log from the resolved pool yields the borrower
// (topic[2]); the same signature from any other contract is ignored.
#[test]
fn process_events_should_extract_borrower_when_borrow_log_present() {
	use sp_core::H160;

	let pool = H160::repeat_byte(0x77);
	let borrower = H160::repeat_byte(0xAB);
	let record = |log| EventRecord::<RuntimeEvent, hydradx_runtime::Hash> {
		phase: frame_system::Phase::Initialization,
		event: RuntimeEvent::EVM(pallet_evm::Event::Log { log }),
		topics: Vec::new(),
	};

	let borrow_log = pallet_evm::Log {
		address: pool,
		topics: vec![events::BORROW, H256::zero(), H256::from(borrower)],
		data: Vec::new(),
	};
	let unrelated_log = pallet_evm::Log {
		address: H160::repeat_byte(0x01),
		topics: vec![events::BORROW, H256::zero(), H256::from(H160::repeat_byte(0xCD))],
		data: Vec::new(),
	};

	let discovered = process_events(vec![record(borrow_log), record(unrelated_log)], pool, "test");

	assert_eq!(discovered, vec![borrower]);
}

// Multi-MM: one pass over the event queue tags each BORROW discovery with its owning pool, so
// the caller can route borrowers to the right market instance.
#[test]
fn process_events_multi_should_tag_borrowers_by_pool_when_logs_come_from_two_pools() {
	use sp_core::H160;

	let pool_a = H160::repeat_byte(0x77);
	let pool_b = H160::repeat_byte(0x88);
	let borrower_a = H160::repeat_byte(0xAB);
	let borrower_b = H160::repeat_byte(0xCD);
	let record = |log| EventRecord::<RuntimeEvent, hydradx_runtime::Hash> {
		phase: frame_system::Phase::Initialization,
		event: RuntimeEvent::EVM(pallet_evm::Event::Log { log }),
		topics: Vec::new(),
	};

	let log_a = pallet_evm::Log {
		address: pool_a,
		topics: vec![events::BORROW, H256::zero(), H256::from(borrower_a)],
		data: Vec::new(),
	};
	let log_b = pallet_evm::Log {
		address: pool_b,
		topics: vec![events::BORROW, H256::zero(), H256::from(borrower_b)],
		data: Vec::new(),
	};
	let log_unknown_pool = pallet_evm::Log {
		address: H160::repeat_byte(0x01),
		topics: vec![events::BORROW, H256::zero(), H256::from(H160::repeat_byte(0xEF))],
		data: Vec::new(),
	};

	let discovered = process_events_multi(
		vec![record(log_a), record(log_b), record(log_unknown_pool)],
		&[pool_a, pool_b],
		"test",
	);

	assert_eq!(discovered, vec![(pool_a, borrower_a), (pool_b, borrower_b)]);
}

#[test]
fn map_decision_collateral_should_pass_through_when_market_is_generic() {
	use sp_core::H160;

	let decision = LiquidationDecision {
		collateral_asset: 670,
		debt_asset: 222,
		user: H160::repeat_byte(0xAB),
		debt_to_cover: 1_000,
		priority: 7,
	};

	let mapped = map_decision_collateral(&decision, InstanceKind::Generic, &HashMap::new(), "test")
		.expect("generic market passes through");
	assert_eq!(mapped, decision);
}

#[test]
fn map_decision_collateral_should_replace_underlying_with_atoken_when_market_is_gigahdx() {
	use sp_core::H160;

	let decision = LiquidationDecision {
		collateral_asset: 670, // stHDX underlying
		debt_asset: 222,       // HOLLAR
		user: H160::repeat_byte(0xAB),
		debt_to_cover: 1_000,
		priority: 7,
	};
	let atoken_map = HashMap::from([(670u32, 67u32)]); // stHDX -> GIGAHDX aToken

	let mapped =
		map_decision_collateral(&decision, InstanceKind::GigaHdx, &atoken_map, "test").expect("mapped decision");
	assert_eq!(mapped.collateral_asset, 67);
	assert_eq!(mapped.debt_asset, decision.debt_asset);
	assert_eq!(mapped.debt_to_cover, decision.debt_to_cover);
	assert_eq!(mapped.priority, decision.priority);
}

// Fail-closed: without the aToken mapping the underlying would route to the generic path
// on-chain and fail the pool check — skip the submission instead.
#[test]
fn map_decision_collateral_should_return_none_when_atoken_mapping_is_missing() {
	use sp_core::H160;

	let decision = LiquidationDecision {
		collateral_asset: 670,
		debt_asset: 222,
		user: H160::repeat_byte(0xAB),
		debt_to_cover: 1_000,
		priority: 7,
	};

	assert_eq!(
		map_decision_collateral(&decision, InstanceKind::GigaHdx, &HashMap::new(), "test"),
		None
	);
}

// The submitted call must be `liquidate_with_pool` carrying the market's pool — decode the
// opaque extrinsic back and pin every field.
#[test]
fn encode_liquidation_opaque_should_encode_liquidate_with_pool_when_pool_is_given() {
	use sp_core::H160;

	let decision = LiquidationDecision {
		collateral_asset: 5,
		debt_asset: 10,
		user: H160::repeat_byte(0xAB),
		debt_to_cover: 123_456,
		priority: 42,
	};
	let market_pool = H160::repeat_byte(0x77);

	let opaque = encode_liquidation_opaque(&decision, market_pool, "test").expect("encode");
	let encoded = opaque.encode();
	let xt = hydradx_runtime::HydraUncheckedExtrinsic::decode(&mut &encoded[..]).expect("decode");

	match xt.0.function {
		RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_with_pool {
			pool,
			collateral_asset,
			debt_asset,
			user,
			debt_to_cover,
			unsigned_priority,
			..
		}) => {
			assert_eq!(pool, market_pool);
			assert_eq!(collateral_asset, 5);
			assert_eq!(debt_asset, 10);
			assert_eq!(user, H160::repeat_byte(0xAB));
			assert_eq!(debt_to_cover, 123_456);
			assert_eq!(unsigned_priority, Some(42));
		}
		other => panic!("unexpected call encoded: {other:?}"),
	}
}

// `storage_prefix` must produce exactly the layout our hardcoded SYSTEM_EVENTS key uses —
// this pins the mechanism the borrowing-contract/gigahdx-pool reads rely on.
#[test]
fn storage_key_helpers_should_match_known_system_events_key() {
	assert_eq!(
		frame_support::storage::storage_prefix(b"System", b"Events"),
		storage_key::SYSTEM_EVENTS
	);
	assert_ne!(storage_key::borrowing_contract(), storage_key::gigahdx_pool_contract());
}

// The omniwatch by-health schema carries a `pool` per borrower; the fetch must return it so
// borrowers can be bucketed per market (dropping it scans everyone against the wrong pool).
#[tokio::test]
async fn fetch_borrowers_list_should_return_pool_tagged_pairs_when_response_has_pool_field() {
	use std::io::{Read, Write};

	let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
	let addr = listener.local_addr().expect("local addr");

	std::thread::spawn(move || {
		if let Ok((mut stream, _)) = listener.accept() {
			let mut buf = [0u8; 2048];
			let _ = stream.read(&mut buf);
			let body = r#"{"lastGlobalUpdate":0,"lastUpdate":0,"borrowers":[["0x222222ff7be76052e023ec1a306fcca8f9659d80",{"totalCollateralBase":1.0,"totalDebtBase":1.0,"availableBorrowsBase":0.0,"currentLiquidationThreshold":0.5,"ltv":0.5,"healthFactor":1.5,"updated":0,"account":"7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb","pool":"0x1b02e051683b5cfac5929c25e84adb26ecf87b38"}],["0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a",{"totalCollateralBase":2.0,"totalDebtBase":1.0,"availableBorrowsBase":0.0,"currentLiquidationThreshold":0.5,"ltv":0.5,"healthFactor":1.8,"updated":0,"account":"7KATdGakyhfBGnAt3XVgXTL7cYjzRXeSZHezKNtENcbwWibb","pool":"0x2ce2cfff743cdb6637f4b5d351937a541b8c8923"}]]}"#;
			let resp = format!(
				"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
				body.len(),
				body
			);
			let _ = stream.write_all(resp.as_bytes());
		}
	});

	let https = https::new();
	let url = format!("http://{addr}/api/borrowers/by-health")
		.parse()
		.expect("valid uri");
	let pairs = fetch_borrowers_list(&https, url, "test").await.expect("fetch to work");

	use sp_core::H160;
	assert_eq!(
		pairs,
		vec![
			(
				H160::from(hex_literal::hex!("222222ff7be76052e023ec1a306fcca8f9659d80")),
				H160::from(hex_literal::hex!("1b02e051683b5cfac5929c25e84adb26ecf87b38")),
			),
			(
				H160::from(hex_literal::hex!("19e7e376e7c213b7e7e7e46cc70a5dd086daff2a")),
				H160::from(hex_literal::hex!("2ce2cfff743cdb6637f4b5d351937a541b8c8923")),
			),
		]
	);
}

// --- Instance registry / discovery (P4) ---

mod registry {
	use super::*;
	use pepl_worker_support::traits::{RuntimeApiErr, RuntimeApiProvider};
	use pepl_worker_support::types::Timestamp;
	use sp_core::H160;

	type TestBlock = hydradx_runtime::Block;
	type TestHash = <TestBlock as BlockT>::Hash;

	/// Answers `ADDRESSES_PROVIDER()` with `pap` and `getPool()` with `pool` — enough to
	/// drive `ensure_instance_for_pool`'s resolve + sanity round-trip.
	struct MockPoolApi {
		pap: EvmAddress,
		pool: EvmAddress,
	}

	impl RuntimeApiProvider<TestBlock> for MockPoolApi {
		fn call(
			&self,
			_block: TestHash,
			_from: EvmAddress,
			_to: EvmAddress,
			data: Vec<u8>,
			_gas_limit: sp_core::U256,
		) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
			let addresses_provider = Into::<u32>::into(pepl_worker_support::Function::AddressesProvider).to_be_bytes();
			let get_pool = Into::<u32>::into(pepl_worker_support::Function::GetPool).to_be_bytes();
			let addr = if data.starts_with(&addresses_provider) {
				self.pap
			} else if data.starts_with(&get_pool) {
				self.pool
			} else {
				return Err(RuntimeApiErr::Dispatch(sp_runtime::DispatchError::Other(
					"unexpected call",
				)));
			};
			let mut word = vec![0u8; 32];
			word[12..].copy_from_slice(addr.as_bytes());
			Ok(fp_evm::ExecutionInfoV2 {
				exit_reason: fp_evm::ExitReason::Succeed(fp_evm::ExitSucceed::Returned),
				value: word,
				used_gas: fp_evm::UsedGas {
					standard: sp_core::U256::zero(),
					effective: sp_core::U256::zero(),
				},
				weight_info: None,
				logs: Vec::new(),
			})
		}

		fn address_to_asset(&self, _block: TestHash, _address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
			Ok(None)
		}

		fn minimum_balance(&self, _block: TestHash, _asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
			Ok(0)
		}

		fn timestamp(&self, _block: TestHash) -> Option<Timestamp> {
			None
		}
	}

	fn cfg() -> LiquidationTaskConfig {
		LiquidationTaskConfig::default()
	}

	#[test]
	fn ensure_instance_should_create_instance_when_round_trip_matches() {
		let pool = H160::repeat_byte(0x11);
		let pap = H160::repeat_byte(0x22);
		let api = MockPoolApi { pap, pool };
		let mut instances = Vec::new();

		let idx = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&cfg(),
		);

		assert_eq!(idx, Some(0));
		assert_eq!(instances.len(), 1);
		assert_eq!(instances[0].pool, Some(pool));
		assert_eq!(instances[0].pap, pap);
		assert_eq!(instances[0].source, InstanceSource::Discovered);
	}

	#[test]
	fn ensure_instance_should_return_existing_index_when_pool_already_instanced() {
		let pool = H160::repeat_byte(0x11);
		let api = MockPoolApi {
			pap: H160::repeat_byte(0x22),
			pool,
		};
		let mut instances = Vec::new();
		let first = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&cfg(),
		);
		let second = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&cfg(),
		);

		assert_eq!(first, second);
		assert_eq!(instances.len(), 1);
	}

	#[test]
	fn ensure_instance_should_skip_when_pool_denylisted() {
		let pool = H160::repeat_byte(0x11);
		let api = MockPoolApi {
			pap: H160::repeat_byte(0x22),
			pool,
		};
		let mut config = cfg();
		config.pool_denylist = vec![pool];
		let mut instances = Vec::new();

		let idx = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&config,
		);

		assert_eq!(idx, None);
		assert!(instances.is_empty());
	}

	#[test]
	fn ensure_instance_should_adopt_config_pin_when_pap_matches() {
		let pool = H160::repeat_byte(0x11);
		let pap = H160::repeat_byte(0x22);
		let api = MockPoolApi { pap, pool };
		// A config-pinned instance whose pool has not resolved yet.
		let mut instances = vec![MmInstance::new("test", pap, None, InstanceSource::Config)];

		let idx = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&cfg(),
		);

		assert_eq!(idx, Some(0));
		assert_eq!(instances.len(), 1, "no duplicate instance for the same market");
		assert_eq!(instances[0].pool, Some(pool));
		assert_eq!(instances[0].source, InstanceSource::Config);
	}

	#[test]
	fn ensure_instance_should_refuse_when_round_trip_mismatches() {
		// The PAP answers getPool() with a DIFFERENT pool than the one being instanced —
		// a bogus contract answering ADDRESSES_PROVIDER() with garbage.
		let pool = H160::repeat_byte(0x11);
		let api = MockPoolApi {
			pap: H160::repeat_byte(0x22),
			pool: H160::repeat_byte(0x33),
		};
		let mut instances = Vec::new();

		let idx = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			pool,
			InstanceSource::Discovered,
			&cfg(),
		);

		assert_eq!(idx, None);
		assert!(instances.is_empty());
	}

	#[test]
	fn ensure_instance_should_not_exceed_max_instances() {
		let api = MockPoolApi {
			pap: H160::repeat_byte(0x22),
			pool: H160::repeat_byte(0xFF),
		};
		let mut instances: Vec<MmInstance> = (0..MAX_MM_INSTANCES)
			.map(|i| {
				let mut inst = MmInstance::new("test", H160::repeat_byte(i as u8), None, InstanceSource::Discovered);
				inst.pool = Some(H160::repeat_byte(0x40 + i as u8));
				inst
			})
			.collect();

		let idx = ensure_instance_for_pool(
			&mut instances,
			&api,
			TestHash::default(),
			H160::repeat_byte(0xFF),
			InstanceSource::Discovered,
			&cfg(),
		);

		assert_eq!(idx, None);
		assert_eq!(instances.len(), MAX_MM_INSTANCES);
	}
}

// --- borrower-set persistence ---

mod cache {
	use super::*;
	use sp_core::H160;

	fn tmp_path(name: &str) -> std::path::PathBuf {
		// Unique-ish per test via the name; std::env::temp_dir avoids polluting the repo.
		std::env::temp_dir().join(format!("pepl-w11-{name}.json"))
	}

	fn instance_with(pool: H160, borrowers: &[H160]) -> MmInstance {
		let mut inst = MmInstance::new("test", H160::repeat_byte(0x01), Some(pool), InstanceSource::Chain);
		inst.borrowers = borrowers.iter().copied().collect();
		inst
	}

	#[test]
	fn borrower_cache_should_round_trip_per_pool_sets() {
		let path = tmp_path("roundtrip");
		let _ = std::fs::remove_file(&path);
		let pool_a = H160::repeat_byte(0xAA);
		let pool_b = H160::repeat_byte(0xBB);
		let a1 = H160::repeat_byte(0x11);
		let a2 = H160::repeat_byte(0x12);
		let b1 = H160::repeat_byte(0x21);
		let instances = vec![instance_with(pool_a, &[a1, a2]), instance_with(pool_b, &[b1])];

		borrower_cache::save(&path, &instances, "test");
		let mut loaded = borrower_cache::load(&path, "test");
		loaded.sort();
		let mut expected = vec![(a1, pool_a), (a2, pool_a), (b1, pool_b)];
		expected.sort();

		assert_eq!(loaded, expected);
		let _ = std::fs::remove_file(&path);
	}

	#[test]
	fn borrower_cache_load_should_return_empty_when_file_missing() {
		let path = tmp_path("missing-xyz");
		let _ = std::fs::remove_file(&path);
		assert!(borrower_cache::load(&path, "test").is_empty());
	}

	#[test]
	fn borrower_cache_load_should_return_empty_when_file_corrupt() {
		let path = tmp_path("corrupt");
		std::fs::write(&path, b"not json at all {{{").expect("write");
		assert!(borrower_cache::load(&path, "test").is_empty());
		let _ = std::fs::remove_file(&path);
	}

	#[test]
	fn borrower_cache_should_skip_instances_without_resolved_pool() {
		let path = tmp_path("unresolved");
		let _ = std::fs::remove_file(&path);
		let pool_a = H160::repeat_byte(0xAA);
		let resolved = instance_with(pool_a, &[H160::repeat_byte(0x11)]);
		// pool = None → not persisted (borrowers get re-seeded anyway)
		let mut unresolved = MmInstance::new("test", H160::repeat_byte(0x02), None, InstanceSource::Config);
		unresolved.borrowers.insert(H160::repeat_byte(0x99));

		borrower_cache::save(&path, &[resolved, unresolved], "test");
		let loaded = borrower_cache::load(&path, "test");

		assert_eq!(loaded, vec![(H160::repeat_byte(0x11), pool_a)]);
		let _ = std::fs::remove_file(&path);
	}

	// A reachable-but-empty response (all instances empty) must NOT overwrite a good file.
	#[test]
	fn borrower_cache_save_should_not_wipe_file_when_set_is_empty() {
		let path = tmp_path("no-wipe");
		let pool = H160::repeat_byte(0xAA);
		borrower_cache::save(&path, &[instance_with(pool, &[H160::repeat_byte(0x11)])], "test");
		assert_eq!(borrower_cache::load(&path, "test").len(), 1);

		// now save an all-empty set — the file must be untouched
		let empty = MmInstance::new("test", H160::repeat_byte(0x02), Some(pool), InstanceSource::Chain);
		borrower_cache::save(&path, &[empty], "test");
		assert_eq!(
			borrower_cache::load(&path, "test").len(),
			1,
			"empty set must not wipe the file"
		);
		let _ = std::fs::remove_file(&path);
	}
}

// A borrower with zero debt is healthy (HF = max), not an error — decide_liquidation skips.
#[test]
fn decide_liquidation_should_return_none_when_borrower_has_no_debt() {
	use pepl_worker_support::types::{Borrower, MoneyMarket, UserConfiguration};
	use sp_core::H160;
	use std::collections::HashMap;

	let cfg = LiquidationTaskConfig::default();
	let mm = MoneyMarket {
		pool: H160::zero(),
		oracle: H160::zero(),
		reserves: HashMap::new(),
		poisoned: Vec::new(),
	};
	let borrower = Borrower {
		configuration: UserConfiguration(U256::zero()),
		address: H160::repeat_byte(0xAB),
		reserves: Vec::new(),
		emode_id: None,
		total_debt: U256::zero(),
		total_collateral: U256::from(2 * MIN_COLLATERAL_BASE),
		updated_at: 0,
	};

	assert_eq!(decide_liquidation(&cfg, &mm, &borrower), None);
}

// Parse a DIA setValue oracle tx into (base_asset_lowercase, price).
#[test]
fn parse_oracle_price_updates_should_extract_base_and_price_from_setvalue() {
	use ethabi::Token;
	use pepl_worker_support::Function;

	let price = U256::from(150_000_000u64); // 8-dec USD
	let mut input = Into::<u32>::into(Function::SetValue).to_be_bytes().to_vec();
	input.extend(ethabi::encode(&[
		Token::String("DOT/USD".to_string()),
		Token::Uint(price),
		Token::Uint(U256::from(1_700_000_000u64)),
	]));

	let signature = ethereum::eip2930::TransactionSignature::new(
		false,
		sp_core::H256::from_low_u64_be(1),
		sp_core::H256::from_low_u64_be(1),
	)
	.expect("sig in range");
	let tx = pallet_ethereum::Transaction::EIP1559(ethereum::EIP1559Transaction {
		chain_id: 222_222,
		nonce: U256::zero(),
		max_priority_fee_per_gas: U256::zero(),
		max_fee_per_gas: U256::zero(),
		gas_limit: U256::from(1_000_000u32),
		action: ethereum::TransactionAction::Call(sp_core::H160::repeat_byte(0xDE)),
		value: U256::zero(),
		input,
		access_list: Vec::new(),
		signature,
	});

	assert_eq!(parse_oracle_price_updates(&tx), vec![("dot".to_string(), price)]);
}

// The derived-token reprice. A DOT price update must reprice BOTH the direct DOT reserve (set to
// the new price) AND a derived reserve whose symbol contains "dot" (e.g. gDOT), scaled by the
// ratio new_base/old_base.
#[test]
fn apply_oracle_updates_should_reprice_direct_and_derived_reserves() {
	use pepl_worker_support::types::{Reserve, ReserveData};
	use sp_core::H160;
	use std::collections::HashMap;

	let mk = |idx: usize, symbol: &str, addr: u8, price: u128| Reserve {
		idx,
		data: ReserveData {
			configuration: U256::zero(),
			liquidity_index: 0,
			current_liquidity_rate: 0,
			variable_borrow_index: 0,
			current_variable_borrow_rate: 0,
			last_update_timestamp: 0,
			a_token_address: H160::zero(),
			stable_debt_token_address: H160::zero(),
			variable_debt_token_address: H160::zero(),
		},
		address: H160::repeat_byte(addr),
		asset_id: idx as u32,
		symbol: symbol.to_string(),
		price: U256::from(price),
		existential_deposit: 0,
		emode: None,
	};

	let dot = mk(0, "DOT", 0x01, 100);
	let gdot = mk(1, "gDOT", 0x02, 250);
	let mut reserves = HashMap::new();
	reserves.insert(dot.address, dot);
	reserves.insert(gdot.address, gdot);
	let mut mm = MoneyMarket {
		pool: H160::zero(),
		oracle: H160::zero(),
		reserves,
		poisoned: Vec::new(),
	};

	// no borrowers → no decisions, but the reprice still happens (we assert on mm)
	let decisions = apply_oracle_updates_and_decide(
		&LiquidationTaskConfig::default(),
		&mut mm,
		&[("dot".to_string(), U256::from(150u32))],
		&[],
	);
	assert!(decisions.is_empty());

	let price_of = |sym: &str| mm.reserves.values().find(|r| r.symbol == sym).unwrap().price;
	assert_eq!(price_of("DOT"), U256::from(150u32)); // direct: set to new price
	assert_eq!(price_of("gDOT"), U256::from(375u32)); // derived: 250 * 150 / 100
}
