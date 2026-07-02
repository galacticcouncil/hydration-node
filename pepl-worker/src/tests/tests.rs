use crate::*;

// Live smoke test: hits the real omniwatch endpoint over the network, so it is
// #[ignore]d by default (flaky, and the borrower count is non-deterministic). Run it
// manually with `cargo test -p pepl-worker -- --ignored` to check omniwatch
// reachability. Deterministic failure-mode coverage lands in W3, against a local mock.
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

// W3: a down/unreachable omniwatch must return `None` (never panic). Port 1 is not listening, so
// the connection is refused immediately — deterministic, no network dependency.
#[tokio::test]
async fn fetch_borrowers_list_should_return_none_when_endpoint_unreachable() {
	let https = https::new();
	let url = "http://127.0.0.1:1/borrowers".parse().expect("valid uri");

	assert_eq!(fetch_borrowers_list(&https, url, "test").await, None);
}

// W3: on total fetch failure the retry wrapper returns `None` (never panics), so the worker
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

// R1: an omniwatch that accepts the TCP connection but never sends an HTTP response must not hang
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

// R2: event-driven discovery — a BORROW log from the resolved pool yields the borrower
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

// R7: a borrower with zero debt is healthy (HF = max), not an error — decide_liquidation skips.
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

// W5: parse a DIA setValue oracle tx into (base_asset_lowercase, price).
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

// W5: the derived-token reprice — v1's blind spot. A DOT price update must reprice BOTH the direct
// DOT reserve (set to the new price) AND a derived reserve whose symbol contains "dot" (e.g. gDOT),
// scaled by the ratio new_base/old_base. v1 left derived reserves at their stale price.
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
