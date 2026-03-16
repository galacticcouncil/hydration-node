//! Comprehensive tests for pepl-worker.
//!
//! Tests are organized by module:
//! - `oracle_tests`: DIA oracle transaction parsing and reserve matching
//! - `worker_tests`: process_new_block, add_new_borrowers, waitlist management
//! - `trait_impl_tests`: standalone trait implementations (oracle injector, report submitter)
//! - `flow_tests`: run_worker state machine control flow

#[cfg(test)]
mod oracle_tests {
	use crate::oracle::*;
	use ethabi::ethereum_types::U256;
	use hex_literal::hex;
	use sp_core::H160;
	use std::collections::HashMap;

	// Raw input bytes for a setValue("tBTC/USD", price, timestamp) call.
	// Extracted from the original node test: dummy_dia_tx_single_value().
	fn single_value_input() -> Vec<u8> {
		hex!(
			"7898e0c2\
			0000000000000000000000000000000000000000000000000000000000000060\
			000000000000000000000000000000000000000000000000000007b205c4101d\
			0000000000000000000000000000000000000000000000000000000067fd2a55\
			0000000000000000000000000000000000000000000000000000000000000008\
			744254432f555344000000000000000000000000000000000000000000000000"
		)
		.to_vec()
	}

	// Raw input bytes for a setMultipleValues(["DOT/ETH", "DAI/ETH"], [...]) call.
	// Extracted from the original node test: dummy_dia_tx_multiple_values().
	fn multiple_values_input() -> Vec<u8> {
		hex!(
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
		.to_vec()
	}

	#[test]
	fn parse_single_value_oracle_input() {
		let input = single_value_input();
		let result = parse_oracle_input(&input).unwrap();

		assert_eq!(result.len(), 1);
		assert_eq!(result[0].base_asset_name, b"tBTC".to_vec());
		assert_eq!(result[0].quote_asset, b"USD".to_vec());
		assert_eq!(result[0].price, U256::from(8461182308381u128));
		assert_eq!(result[0].timestamp, U256::from(1744644693u128));
	}

	#[test]
	fn parse_multiple_values_oracle_input() {
		let input = multiple_values_input();
		let result = parse_oracle_input(&input).unwrap();

		assert_eq!(result.len(), 2);

		assert_eq!(result[0].base_asset_name, b"DOT".to_vec());
		assert_eq!(result[0].quote_asset, b"ETH".to_vec());
		assert_eq!(result[0].price, U256::from(699777847u128));
		assert_eq!(result[0].timestamp, U256::from(1739373797u128));

		assert_eq!(result[1].base_asset_name, b"DAI".to_vec());
		assert_eq!(result[1].quote_asset, b"ETH".to_vec());
		assert_eq!(result[1].price, U256::from(23951192810u128));
		assert_eq!(result[1].timestamp, U256::from(1739373797u128));
	}

	#[test]
	fn parse_oracle_input_too_short() {
		assert!(parse_oracle_input(&[0, 1, 2]).is_none());
	}

	#[test]
	fn parse_oracle_input_unknown_selector() {
		let input = hex!("deadbeef0000000000000000000000000000000000000000000000000000000000000000");
		let result = parse_oracle_input(&input);
		// Unknown selector returns Some(empty vec) — the function doesn't error, just finds no data.
		assert_eq!(result, Some(vec![]));
	}

	#[test]
	fn match_oracle_to_reserves_exact_match() {
		let dot_addr = H160::from_low_u64_be(1);
		let mut reserves = HashMap::new();
		reserves.insert(dot_addr, b"DOT".to_vec());

		let oracle_data = vec![OracleUpdateData::new(
			b"DOT".to_vec(),
			b"USD".to_vec(),
			U256::from(350u32),
			U256::from(100u32),
		)];

		let matched = match_oracle_to_reserves(&oracle_data, &reserves);
		assert_eq!(matched.len(), 1);
		assert_eq!(matched[0].0, dot_addr);
		assert_eq!(matched[0].1, Some(U256::from(350u32)));
	}

	#[test]
	fn match_oracle_to_reserves_partial_name_match() {
		// e.g., oracle updates "DOT" and reserve has "aDOT" (aToken)
		let adot_addr = H160::from_low_u64_be(2);
		let mut reserves = HashMap::new();
		reserves.insert(adot_addr, b"aDOT".to_vec());

		let oracle_data = vec![OracleUpdateData::new(
			b"DOT".to_vec(),
			b"USD".to_vec(),
			U256::from(350u32),
			U256::from(100u32),
		)];

		let matched = match_oracle_to_reserves(&oracle_data, &reserves);
		assert_eq!(matched.len(), 1);
		assert_eq!(matched[0].0, adot_addr);
		// Partial match (aDOT != DOT) so price is None.
		assert_eq!(matched[0].1, None);
	}

	#[test]
	fn match_oracle_to_reserves_no_match() {
		let eth_addr = H160::from_low_u64_be(3);
		let mut reserves = HashMap::new();
		reserves.insert(eth_addr, b"ETH".to_vec());

		let oracle_data = vec![OracleUpdateData::new(
			b"BTC".to_vec(),
			b"USD".to_vec(),
			U256::from(60000u32),
			U256::from(100u32),
		)];

		let matched = match_oracle_to_reserves(&oracle_data, &reserves);
		assert!(matched.is_empty());
	}

	#[test]
	fn match_oracle_to_reserves_multiple_reserves() {
		let dot_addr = H160::from_low_u64_be(1);
		let adot_addr = H160::from_low_u64_be(2);
		let eth_addr = H160::from_low_u64_be(3);
		let mut reserves = HashMap::new();
		reserves.insert(dot_addr, b"DOT".to_vec());
		reserves.insert(adot_addr, b"aDOT".to_vec());
		reserves.insert(eth_addr, b"ETH".to_vec());

		let oracle_data = vec![OracleUpdateData::new(
			b"DOT".to_vec(),
			b"USD".to_vec(),
			U256::from(350u32),
			U256::from(100u32),
		)];

		let matched = match_oracle_to_reserves(&oracle_data, &reserves);
		// Should match both DOT (exact) and aDOT (partial).
		assert_eq!(matched.len(), 2);

		let exact = matched.iter().find(|(addr, _)| *addr == dot_addr).unwrap();
		assert_eq!(exact.1, Some(U256::from(350u32)));

		let partial = matched.iter().find(|(addr, _)| *addr == adot_addr).unwrap();
		assert_eq!(partial.1, None);
	}

	#[test]
	fn match_oracle_empty_reserves() {
		let reserves = HashMap::new();
		let oracle_data = vec![OracleUpdateData::new(
			b"DOT".to_vec(),
			b"USD".to_vec(),
			U256::from(350u32),
			U256::from(100u32),
		)];

		let matched = match_oracle_to_reserves(&oracle_data, &reserves);
		assert!(matched.is_empty());
	}

	#[test]
	fn match_oracle_empty_data() {
		let mut reserves = HashMap::new();
		reserves.insert(H160::from_low_u64_be(1), b"DOT".to_vec());

		let matched = match_oracle_to_reserves(&[], &reserves);
		assert!(matched.is_empty());
	}
}

#[cfg(test)]
mod worker_tests {
	use crate::traits::*;
	use ethabi::ethereum_types::U256;
	use liquidation_worker_support::Borrower;
	use sp_core::H160;
	use std::collections::HashMap;

	fn addr(n: u64) -> H160 {
		H160::from_low_u64_be(n)
	}

	fn borrower(n: u64, hf: u128) -> Borrower {
		Borrower {
			user_address: addr(n),
			health_factor: U256::from(hf),
		}
	}

	// ---- add_new_borrowers tests ----

	#[test]
	fn add_new_borrowers_to_empty_list() {
		let mut borrowers = Vec::new();
		crate::worker::add_new_borrowers(vec![addr(1), addr(2)], &mut borrowers);

		assert_eq!(borrowers.len(), 2);
		// Both should have HF=0.
		assert!(borrowers.iter().all(|b| b.health_factor == U256::zero()));
	}

	#[test]
	fn add_new_borrowers_preserves_sort_order() {
		let mut borrowers = vec![
			borrower(1, 500),
			borrower(2, 1000),
			borrower(3, 2000),
		];

		crate::worker::add_new_borrowers(vec![addr(4)], &mut borrowers);

		// New borrower has HF=0, should be first.
		assert_eq!(borrowers[0].user_address, addr(4));
		assert_eq!(borrowers[0].health_factor, U256::zero());
		// Rest should be sorted ascending.
		for w in borrowers.windows(2) {
			assert!(w[0].health_factor <= w[1].health_factor);
		}
	}

	#[test]
	fn add_existing_borrower_resets_hf_and_resorts() {
		let mut borrowers = vec![
			borrower(1, 500),
			borrower(2, 1000),
			borrower(3, 2000),
		];

		// Re-add borrower 3 — its HF should reset to 0 and it should move to front.
		crate::worker::add_new_borrowers(vec![addr(3)], &mut borrowers);

		assert_eq!(borrowers.len(), 3);
		assert_eq!(borrowers[0].user_address, addr(3));
		assert_eq!(borrowers[0].health_factor, U256::zero());
	}

	#[test]
	fn add_mix_of_new_and_existing_borrowers() {
		let mut borrowers = vec![
			borrower(1, 500),
			borrower(2, 1000),
		];

		crate::worker::add_new_borrowers(vec![addr(2), addr(3)], &mut borrowers);

		assert_eq!(borrowers.len(), 3);
		// Borrowers 2 and 3 should have HF=0 (both at front after sort).
		let zero_hf: Vec<_> = borrowers.iter().filter(|b| b.health_factor == U256::zero()).collect();
		assert_eq!(zero_hf.len(), 2);
	}

	// ---- process_new_block tests ----

	fn make_block_event(block_number: BlockNumber) -> BlockEvent {
		BlockEvent {
			block_number,
			block_hash: [block_number as u8; 32],
			new_borrowers: vec![],
			liquidated_users: vec![],
			new_assets: vec![],
		}
	}

	#[test]
	fn process_new_block_updates_block_number_and_hash() {
		let event = make_block_event(42);
		let mut borrowers = vec![];
		let mut snapshot = vec![];
		let mut liquidated = vec![];
		let mut waitlist = HashMap::new();
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		assert_eq!(block_num, 42);
		assert_eq!(block_hash, [42u8; 32]);
	}

	#[test]
	fn process_new_block_clears_liquidated_users() {
		let event = make_block_event(1);
		let mut borrowers = vec![];
		let mut snapshot = vec![];
		let mut liquidated = vec![addr(1), addr(2)];
		let mut waitlist = HashMap::new();
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		assert!(liquidated.is_empty());
	}

	#[test]
	fn process_new_block_removes_liquidated_from_waitlist() {
		let mut event = make_block_event(5);
		event.liquidated_users = vec![addr(1)];

		let mut borrowers = vec![];
		let mut snapshot = vec![];
		let mut liquidated = vec![];
		let mut waitlist = HashMap::new();
		waitlist.insert(addr(1), 3u32); // submitted at block 3
		waitlist.insert(addr(2), 4u32); // submitted at block 4
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		// addr(1) was liquidated, should be removed.
		assert!(!waitlist.contains_key(&addr(1)));
		// addr(2) should still be there.
		assert!(waitlist.contains_key(&addr(2)));
	}

	#[test]
	fn process_new_block_evicts_stale_waitlist_entries() {
		let event = make_block_event(10);

		let mut borrowers = vec![];
		let mut snapshot = vec![];
		let mut liquidated = vec![];
		let mut waitlist = HashMap::new();
		// Submitted at block 5 — age 5, which is > TTL (2).
		waitlist.insert(addr(1), 5u32);
		// Submitted at block 8 — age 2, which is NOT > TTL (2).
		waitlist.insert(addr(2), 8u32);
		// Submitted at block 9 — age 1, still fresh.
		waitlist.insert(addr(3), 9u32);
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		// addr(1) should be evicted (10-5=5 > 2).
		assert!(!waitlist.contains_key(&addr(1)));
		// addr(2) should be kept (10-8=2, NOT > 2).
		assert!(waitlist.contains_key(&addr(2)));
		// addr(3) should be kept (10-9=1 < 2).
		assert!(waitlist.contains_key(&addr(3)));
	}

	#[test]
	fn process_new_block_adds_new_borrowers() {
		let mut event = make_block_event(1);
		event.new_borrowers = vec![addr(10), addr(20)];

		let mut borrowers = vec![borrower(1, 1000)];
		let mut snapshot = vec![];
		let mut liquidated = vec![];
		let mut waitlist = HashMap::new();
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		assert_eq!(borrowers.len(), 3);
		// Snapshot should match borrowers.
		assert_eq!(snapshot.len(), 3);
		// Verify snapshot has the same addresses as borrowers.
		for (s, b) in snapshot.iter().zip(borrowers.iter()) {
			assert_eq!(s.user_address, b.user_address);
			assert_eq!(s.health_factor, b.health_factor);
		}
	}

	#[test]
	fn process_new_block_combined_scenario() {
		// Block 10: user 1 was liquidated, user 5 is stale in waitlist,
		// user 6 is fresh in waitlist, new borrower user 99.
		let mut event = make_block_event(10);
		event.liquidated_users = vec![addr(1)];
		event.new_borrowers = vec![addr(99)];

		let mut borrowers = vec![borrower(1, 500), borrower(2, 1000)];
		let mut snapshot = vec![];
		let mut liquidated = vec![addr(77)]; // leftover from previous block
		let mut waitlist = HashMap::new();
		waitlist.insert(addr(1), 8u32); // will be removed (liquidated)
		waitlist.insert(addr(5), 3u32); // will be evicted (stale: 10-3=7 > 2)
		waitlist.insert(addr(6), 9u32); // will remain (fresh: 10-9=1)
		let mut block_num = 0u32;
		let mut block_hash = [0u8; 32];

		crate::worker::process_block_state(
			&event,
			&mut borrowers,
			&mut snapshot,
			&mut liquidated,
			&mut waitlist,
			&mut block_num,
			&mut block_hash,
		);

		assert_eq!(block_num, 10);
		assert!(liquidated.is_empty()); // cleared
		assert!(!waitlist.contains_key(&addr(1))); // liquidated
		assert!(!waitlist.contains_key(&addr(5))); // evicted
		assert!(waitlist.contains_key(&addr(6))); // kept
		assert_eq!(borrowers.len(), 3); // original 2 + new 99
		assert!(borrowers.iter().any(|b| b.user_address == addr(99)));
		assert_eq!(snapshot.len(), borrowers.len());
		for (s, b) in snapshot.iter().zip(borrowers.iter()) {
			assert_eq!(s.user_address, b.user_address);
		}
	}
}

#[cfg(test)]
mod trait_impl_tests {
	use crate::traits::*;

	// ---- BlockSource tests ----

	struct VecBlockSource {
		events: Vec<BlockEvent>,
		index: usize,
	}

	impl VecBlockSource {
		fn new(events: Vec<BlockEvent>) -> Self {
			Self { events, index: 0 }
		}
	}

	impl BlockSource for VecBlockSource {
		fn next_block(&mut self) -> Option<BlockEvent> {
			if self.index < self.events.len() {
				let event = self.events[self.index].clone();
				self.index += 1;
				Some(event)
			} else {
				None
			}
		}
	}

	#[test]
	fn vec_block_source_yields_events_in_order() {
		let mut source = VecBlockSource::new(vec![
			BlockEvent {
				block_number: 1,
				block_hash: [1; 32],
				new_borrowers: vec![],
				liquidated_users: vec![],
				new_assets: vec![],
			},
			BlockEvent {
				block_number: 2,
				block_hash: [2; 32],
				new_borrowers: vec![],
				liquidated_users: vec![],
				new_assets: vec![],
			},
		]);

		assert_eq!(source.next_block().unwrap().block_number, 1);
		assert_eq!(source.next_block().unwrap().block_number, 2);
		assert!(source.next_block().is_none());
	}

	// ---- TxSubmitter tests ----

	struct RecordingSubmitter {
		submissions: std::cell::RefCell<Vec<LiquidationTx>>,
	}

	impl RecordingSubmitter {
		fn new() -> Self {
			Self {
				submissions: std::cell::RefCell::new(vec![]),
			}
		}

		fn count(&self) -> usize {
			self.submissions.borrow().len()
		}
	}

	impl TxSubmitter for RecordingSubmitter {
		fn submit(&self, tx: &LiquidationTx, _block_hash: [u8; 32]) -> SubmitResult {
			self.submissions.borrow_mut().push(tx.clone());
			SubmitResult::Submitted
		}
	}

	#[test]
	fn recording_submitter_records_submissions() {
		let submitter = RecordingSubmitter::new();
		let tx = LiquidationTx {
			user: sp_core::H160::from_low_u64_be(1),
			collateral_asset: 5,
			debt_asset: 10,
			debt_to_cover: 1000,
			health_factor: ethabi::ethereum_types::U256::zero(),
		};

		let result = submitter.submit(&tx, [0; 32]);
		assert!(matches!(result, SubmitResult::Submitted));
		assert_eq!(submitter.count(), 1);
	}

	// ---- OracleSource tests ----

	struct VecOracleSource {
		batches: Vec<Vec<OracleUpdate>>,
		index: usize,
	}

	impl VecOracleSource {
		fn new(batches: Vec<Vec<OracleUpdate>>) -> Self {
			Self { batches, index: 0 }
		}
	}

	impl OracleSource for VecOracleSource {
		fn poll_oracle_updates(&mut self) -> Vec<OracleUpdate> {
			if self.index < self.batches.len() {
				let batch = self.batches[self.index].clone();
				self.index += 1;
				batch
			} else {
				vec![]
			}
		}
	}

	#[test]
	fn vec_oracle_source_yields_batches() {
		let mut source = VecOracleSource::new(vec![
			vec![OracleUpdate {
				asset_address: sp_core::H160::from_low_u64_be(1),
				price: Some(ethabi::ethereum_types::U256::from(100u32)),
			}],
			vec![], // empty batch
		]);

		let batch1 = source.poll_oracle_updates();
		assert_eq!(batch1.len(), 1);

		let batch2 = source.poll_oracle_updates();
		assert!(batch2.is_empty());

		// Exhausted.
		let batch3 = source.poll_oracle_updates();
		assert!(batch3.is_empty());
	}

	// ---- DryRunner tests ----

	struct AlwaysPassDryRunner;
	impl DryRunner for AlwaysPassDryRunner {
		fn dry_run(&self, _tx: &LiquidationTx, _block_hash: [u8; 32]) -> bool {
			true
		}
	}

	struct AlwaysFailDryRunner;
	impl DryRunner for AlwaysFailDryRunner {
		fn dry_run(&self, _tx: &LiquidationTx, _block_hash: [u8; 32]) -> bool {
			false
		}
	}

	#[test]
	fn dry_runner_pass_and_fail() {
		let tx = LiquidationTx {
			user: sp_core::H160::from_low_u64_be(1),
			collateral_asset: 5,
			debt_asset: 10,
			debt_to_cover: 1000,
			health_factor: ethabi::ethereum_types::U256::zero(),
		};

		assert!(AlwaysPassDryRunner.dry_run(&tx, [0; 32]));
		assert!(!AlwaysFailDryRunner.dry_run(&tx, [0; 32]));
	}
}

#[cfg(test)]
#[cfg(feature = "standalone")]
mod standalone_tests {
	use crate::standalone::oracle_injector::*;
	use crate::traits::*;
	use ethabi::ethereum_types::U256;
	use sp_core::H160;

	#[test]
	fn oracle_injector_delivers_queued_updates() {
		let mut injector = OracleInjector::new();

		let updates = vec![OracleUpdate {
			asset_address: H160::from_low_u64_be(1),
			price: Some(U256::from(350u32)),
		}];

		injector.inject(updates.clone());
		injector.inject(vec![OracleUpdate {
			asset_address: H160::from_low_u64_be(2),
			price: None,
		}]);

		let batch1 = injector.poll_oracle_updates();
		assert_eq!(batch1.len(), 1);
		assert_eq!(batch1[0].asset_address, H160::from_low_u64_be(1));

		let batch2 = injector.poll_oracle_updates();
		assert_eq!(batch2.len(), 1);
		assert_eq!(batch2[0].asset_address, H160::from_low_u64_be(2));

		// Exhausted.
		assert!(injector.poll_oracle_updates().is_empty());
	}

	#[test]
	fn oracle_injector_load_scenario() {
		let mut injector = OracleInjector::new();

		let scenario = OracleScenario {
			block: "latest".to_string(),
			oracle_updates: vec![
				OracleScenarioEntry {
					pair: "DOT/USD".to_string(),
					price: 3.5,
					asset_address: Some("0x0000000000000000000000000000000000000001".to_string()),
				},
				OracleScenarioEntry {
					pair: "ETH/USD".to_string(),
					price: 1800.0,
					asset_address: Some("0x0000000000000000000000000000000000000002".to_string()),
				},
			],
		};

		injector.load_scenario(&scenario);

		let updates = injector.poll_oracle_updates();
		assert_eq!(updates.len(), 2);
		assert_eq!(updates[0].asset_address, H160::from_low_u64_be(1));
		assert_eq!(updates[0].price, Some(U256::from(350_000_000u64))); // 3.5 * 1e8
		assert_eq!(updates[1].asset_address, H160::from_low_u64_be(2));
		assert_eq!(updates[1].price, Some(U256::from(180_000_000_000u64))); // 1800 * 1e8
	}

	#[test]
	fn oracle_injector_skips_invalid_addresses() {
		let mut injector = OracleInjector::new();

		let scenario = OracleScenario {
			block: "latest".to_string(),
			oracle_updates: vec![
				OracleScenarioEntry {
					pair: "DOT/USD".to_string(),
					price: 3.5,
					asset_address: None, // No address — should be skipped.
				},
				OracleScenarioEntry {
					pair: "ETH/USD".to_string(),
					price: 1800.0,
					asset_address: Some("invalid".to_string()), // Bad hex — should be skipped.
				},
				OracleScenarioEntry {
					pair: "BTC/USD".to_string(),
					price: 60000.0,
					asset_address: Some("0x00".to_string()), // Too short — should be skipped.
				},
			],
		};

		injector.load_scenario(&scenario);

		// All entries were invalid, so nothing should be queued.
		assert!(injector.poll_oracle_updates().is_empty());
	}

	#[test]
	fn noop_oracle_source_always_empty() {
		let mut source = NoOpOracleSource;
		assert!(source.poll_oracle_updates().is_empty());
		assert!(source.poll_oracle_updates().is_empty());
	}

	#[test]
	fn report_submitter_returns_dry_run() {
		use crate::standalone::report_submitter::ReportSubmitter;

		let submitter = ReportSubmitter;
		let tx = LiquidationTx {
			user: H160::from_low_u64_be(1),
			collateral_asset: 5,
			debt_asset: 10,
			debt_to_cover: 1000,
			health_factor: U256::zero(),
		};

		let result = submitter.submit(&tx, [0; 32]);
		assert!(matches!(result, SubmitResult::DryRun));
	}
}
