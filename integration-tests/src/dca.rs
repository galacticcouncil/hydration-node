#![cfg(test)]

use crate::count_dca_event;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;

use crate::{assert_balance, assert_reserved_balance};
use frame_support::storage::with_transaction;
use frame_system::RawOrigin;
use hydradx_runtime::AssetPairAccountIdFor;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	AssetRegistry, Balances, Currencies, InsufficientEDinHDX, Omnipool, Router, Runtime, RuntimeEvent, RuntimeOrigin,
	Stableswap, Tokens, Treasury, DCA,
};
use hydradx_traits::registry::{AssetKind, Create};
use hydradx_traits::router::AssetPair;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::Trade;
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_broadcast::types::*;
use pallet_dca::types::{Order, Schedule};
use pallet_omnipool::types::Tradability;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use primitives::{AssetId, Balance};
use sp_runtime::traits::ConstU32;
use sp_runtime::DispatchError;
use sp_runtime::Permill;
use sp_runtime::{BoundedVec, FixedU128};
use sp_runtime::{DispatchResult, TransactionOutcome};
use xcm_emulator::TestExt;
const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

mod omnipool {
	use super::*;
	use frame_support::assert_ok;
	use hydradx_runtime::{Balances, Currencies, Treasury, DCA, XYK};
	use hydradx_traits::router::{PoolType, Trade};
	use hydradx_traits::AssetKind;
	use pallet_broadcast::types::Destination;
	use sp_runtime::{FixedU128, TransactionOutcome};

	#[test]
	fn create_schedule_should_work() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let block_id = 11;
			set_relaychain_block_number(block_id);

			let budget = 1000 * UNITS;
			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, 100 * UNITS, budget);

			//Act
			assert_ok!(DCA::schedule(
				RuntimeOrigin::signed(ALICE.into()),
				schedule1.clone(),
				None
			));

			//Assert
			let schedule_id = 0;
			let schedule = DCA::schedules(schedule_id);
			assert!(schedule.is_some());

			let next_block_id = block_id + 2;
			let schedule = DCA::schedule_ids_per_block(next_block_id);
			assert!(!schedule.is_empty());
			expect_hydra_last_events(vec![pallet_dca::Event::Scheduled {
				id: 0,
				who: ALICE.into(),
				period: schedule1.period,
				total_amount: schedule1.total_amount,
				order: schedule1.order,
			}
			.into()]);
		});
	}
	#[test]
	fn create_schedule_should_work_when_insufficient_asset_as_fee() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				init_omnipool_with_oracle_for_block_10();
				add_dot_as_payment_currency();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 10000 * UNITS, DAI, 20000 * UNITS);
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1200000 * UNITS);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracle
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					2 * UNITS as i128,
				));
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					UNITS,
					0,
					false
				));

				//Arrange
				let block_id = 11;
				set_relaychain_block_number(block_id);

				let budget = 5000 * UNITS;
				let schedule1 =
					schedule_fake_with_buy_order(PoolType::XYK, insufficient_asset, DOT, 100 * UNITS, budget);

				//Act
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					insufficient_asset,
					5000 * UNITS as i128,
				));
				assert_ok!(DCA::schedule(
					RuntimeOrigin::signed(ALICE.into()),
					schedule1.clone(),
					None
				));

				//Assert
				let schedule_id = 0;
				let schedule = DCA::schedules(schedule_id);
				assert!(schedule.is_some());

				let next_block_id = block_id + 2;
				let schedule = DCA::schedule_ids_per_block(next_block_id);
				assert!(!schedule.is_empty());
				expect_hydra_last_events(vec![pallet_dca::Event::Scheduled {
					id: 0,
					who: ALICE.into(),
					period: schedule1.period,
					total_amount: schedule1.total_amount,
					order: schedule1.order,
				}
				.into()]);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_schedule_execution_should_work_when_block_is_initialized() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			let amount_out = 100 * UNITS;
			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, amount_out, dca_budget);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
			assert!(fee > 0, "Treasury got rugged");

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, 858666753156614);
		});
	}

	#[test]
	fn buy_schedule_execution_should_emit_swapped_events() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;
			let amount_out = 100 * UNITS;
			let schedule_id = 0;
			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, amount_out, dca_budget);
			create_schedule(ALICE, schedule1);

			//Act
			run_to_block(11, 12);

			//Assert
			let swapped_events = get_last_swapped_events();
			let last_two_swapped_events = &swapped_events[swapped_events.len() - 2..];
			pretty_assertions::assert_eq!(
				last_two_swapped_events,
				vec![
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(HDX, 140421094366889)],
						outputs: vec![Asset::new(LRNA, 70210545436637)],
						fees: vec![
							Fee::new(LRNA, 17552636359, Destination::Burned),
							Fee::new(LRNA, 17552636359, Destination::Account(Treasury::account_id()))
						],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 0),
							ExecutionType::Router(1),
							ExecutionType::Omnipool(2)
						]
					},
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(LRNA, 70175440163919)],
						outputs: vec![Asset::new(DAI, amount_out)],
						fees: vec![Fee::new(
							DAI,
							250626566417,
							Destination::Account(Omnipool::protocol_account())
						)],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 0),
							ExecutionType::Router(1),
							ExecutionType::Omnipool(2)
						],
					}
				]
			);

			run_to_block(13, 17);

			let swapped_events = get_last_swapped_events();
			let last_two_swapped_events = &swapped_events[swapped_events.len() - 2..];
			pretty_assertions::assert_eq!(
				last_two_swapped_events,
				vec![
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(HDX, 140421107723192)],
						outputs: vec![Asset::new(LRNA, 70210548452699)],
						fees: vec![
							Fee::new(LRNA, 17552637113, Destination::Burned),
							Fee::new(LRNA, 17552637113, Destination::Account(Treasury::account_id()))
						],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 3),
							ExecutionType::Router(4),
							ExecutionType::Omnipool(5)
						],
					},
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(LRNA, 70175443178473)],
						outputs: vec![Asset::new(DAI, amount_out)],
						fees: vec![Fee::new(
							DAI,
							250626566417,
							Destination::Account(Omnipool::protocol_account())
						)],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 3),
							ExecutionType::Router(4),
							ExecutionType::Omnipool(5)
						],
					}
				]
			);
		});
	}

	#[test]
	fn buy_schedule_execution_should_work_without_route() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			let amount_out = 100 * UNITS;
			let no_route = vec![];
			let schedule1 = schedule_fake_with_buy_order_with_route(HDX, DAI, amount_out, dca_budget, no_route);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
			assert!(fee > 0, "Treasury got rugged");

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, 858666753156614);
		});
	}

	#[test]
	fn buy_schedule_should_be_retried_multiple_times_then_terminated() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			let amount_out = 100 * UNITS;
			let schedule1 = Schedule {
				owner: AccountId::from(ALICE),
				period: 5u32,
				total_amount: dca_budget,
				max_retries: None,
				stability_threshold: None,
				slippage: Some(Permill::from_percent(5)),
				order: Order::Buy {
					asset_in: HDX,
					asset_out: DAI,
					amount_out,
					max_amount_in: Balance::MIN,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DAI,
					}]),
				},
			};
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act and assert
			let schedule_id = 0;
			set_relaychain_block_number(12);
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - fee);
			assert_eq!(DCA::retries_on_error(schedule_id), 1);

			set_relaychain_block_number(22);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 2 * fee);
			assert_eq!(DCA::retries_on_error(schedule_id), 2);

			set_relaychain_block_number(42);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 3 * fee);
			assert_eq!(DCA::retries_on_error(schedule_id), 3);

			//After this retry we terminate
			set_relaychain_block_number(82);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - 4 * fee);
			assert_reserved_balance!(&ALICE.into(), HDX, 0);
			assert_eq!(DCA::retries_on_error(schedule_id), 0);
			let schedule = DCA::schedules(schedule_id);
			assert!(schedule.is_none());
		});
	}

	#[test]
	fn buy_schedule_execution_should_work_when_asset_in_is_hub_asset() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let alice_init_hub_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_hub_balance);

			let dca_budget = 2500 * UNITS;

			let amount_out = 100 * UNITS;
			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, LRNA, DAI, amount_out, dca_budget);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
			assert_balance!(&Treasury::account_id(), LRNA, 0);

			//Act
			set_relaychain_block_number(12);

			//Assert
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
			assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), LRNA, 2429368483709670);

			let treasury_balance = Currencies::free_balance(LRNA, &Treasury::account_id());
			assert!(treasury_balance > 0);
		});
	}

	#[test]
	fn buy_schedule_and_direct_buy_and_router_should_yield_same_result_when_selling_native_asset() {
		let amount_out = 100 * UNITS;

		//DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, amount_out, dca_budget);
			create_schedule(ALICE, schedule1);

			//Act
			set_relaychain_block_number(12);

			//Assert
			assert_reserved_balance!(&ALICE.into(), HDX, 858666753156614);

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		});

		//Direct Omnipool
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				HDX,
				amount_out,
				Balance::MAX,
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, 859578905568959);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		});

		//Router
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			let trade = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				amount_out,
				Balance::MAX,
				trade
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, 859578905568959);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		});
	}

	#[test]
	fn buy_schedule_and_direct_buy_and_router_should_yield_same_result_when_asset_in_is_hub_asset() {
		let amount_out = 100 * UNITS;

		//DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, LRNA, DAI, amount_out, dca_budget);
			create_schedule(ALICE, schedule1);

			//Act
			set_relaychain_block_number(12);

			//Assert
			assert_reserved_balance!(&ALICE.into(), LRNA, 929368483709670);
			assert_balance!(ALICE.into(), DAI, 2100000000000000);
		});

		//Direct Omnipool
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			init_omnipool_with_oracle_for_block_10();

			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(ALICE.into()),
				DAI,
				LRNA,
				amount_out,
				Balance::MAX,
			));

			//Assert
			assert_balance!(ALICE.into(), LRNA, 4929824559916281);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		});

		//Router
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			init_omnipool_with_oracle_for_block_10();

			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			let trade = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: DAI,
			}];
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE.into()),
				LRNA,
				DAI,
				amount_out,
				Balance::MAX,
				trade
			));

			//Assert
			assert_balance!(ALICE.into(), LRNA, 4929824559916281);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		});
	}

	#[test]
	fn full_buy_dca_should_be_executed_then_completed() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1000 * UNITS;
			let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, 100 * UNITS, dca_budget);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act
			run_to_block(11, 50);

			//Assert
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + 700 * UNITS);

			//Because the last trade is not enough for a whole trade, it is returned to the user
			let amount_in = 140_421_094_431_120;
			let alice_new_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
			assert!(alice_new_hdx_balance < amount_in);
			assert!(alice_new_hdx_balance > 0);

			assert_reserved_balance!(&ALICE.into(), HDX, 0);

			let schedule = DCA::schedules(0);
			assert!(schedule.is_none());
		});
	}

	#[test]
	fn rolling_buy_dca_should_continue_until_funds_are_spent() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let balance = 20000 * UNITS;
			let trade_size = 500 * UNITS;
			let dca_budget = 0; // rolling
			Balances::force_set_balance(RuntimeOrigin::root(), ALICE.into(), balance).unwrap();
			create_schedule(
				ALICE,
				schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, trade_size, dca_budget),
			);
			let reserved = Balances::reserved_balance(&ALICE.into());
			assert!(Balances::free_balance(&ALICE.into()) <= balance - reserved);
			let dai_balance = Currencies::free_balance(DAI, &ALICE.into());

			//Act
			run_to_block(11, 150);

			//Assert
			assert!(Balances::free_balance(&ALICE.into()) > reserved);
			assert!(Currencies::free_balance(DAI, &ALICE.into()) > dai_balance);
			assert!(DCA::schedules(0).is_none());
		});
	}

	#[test]
	fn sell_schedule_execution_should_work_when_block_is_initialized() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
			assert!(fee > 0, "Treasury got rugged");

			assert_balance!(ALICE.into(), DAI, 2071214372591672);
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);
		});
	}

	#[test]
	fn sell_schedule_execution_should_emit_swapped_event() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule_id = 0;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			//Act
			run_to_block(11, 12);

			//Assert
			let swapped_events = get_last_swapped_events();
			let last_two_swapped_events = &swapped_events[swapped_events.len() - 2..];
			pretty_assertions::assert_eq!(
				last_two_swapped_events,
				vec![
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactIn,
						inputs: vec![Asset::new(HDX, amount_to_sell)],
						outputs: vec![Asset::new(LRNA, 49999999160157)],
						fees: vec![
							Fee::new(LRNA, 12499999790, Destination::Burned),
							Fee::new(LRNA, 12499999790, Destination::Account(Treasury::account_id()))
						],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 0),
							ExecutionType::Router(1),
							ExecutionType::Omnipool(2)
						],
					},
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactIn,
						inputs: vec![Asset::new(LRNA, 49974999160577)],
						outputs: vec![Asset::new(DAI, 71214372624206)],
						fees: vec![Fee::new(
							DAI,
							178482136903,
							Destination::Account(Omnipool::protocol_account())
						)],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 0),
							ExecutionType::Router(1),
							ExecutionType::Omnipool(2)
						],
					}
				]
			);

			run_to_block(13, 17);

			let swapped_events = get_last_swapped_events();
			let last_two_swapped_events = &swapped_events[swapped_events.len() - 2..];
			pretty_assertions::assert_eq!(
				last_two_swapped_events,
				vec![
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactIn,
						inputs: vec![Asset::new(HDX, amount_to_sell)],
						outputs: vec![Asset::new(LRNA, 49999997360494)],
						fees: vec![
							Fee::new(LRNA, 12499999340, Destination::Burned),
							Fee::new(LRNA, 12499999340, Destination::Account(Treasury::account_id()))
						],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 3),
							ExecutionType::Router(4),
							ExecutionType::Omnipool(5)
						],
					},
					pallet_broadcast::Event::Swapped {
						swapper: ALICE.into(),
						filler: Omnipool::protocol_account(),
						filler_type: pallet_broadcast::types::Filler::Omnipool,
						operation: pallet_broadcast::types::TradeOperation::ExactIn,
						inputs: vec![Asset::new(LRNA, 49974997361814)],
						outputs: vec![Asset::new(DAI, 71214367823821)],
						fees: vec![Fee::new(
							DAI,
							178482124872,
							Destination::Account(Omnipool::protocol_account())
						)],
						operation_stack: vec![
							ExecutionType::DCA(schedule_id, 3),
							ExecutionType::Router(4),
							ExecutionType::Omnipool(5)
						],
					}
				]
			);
		});
	}

	#[test]
	fn sell_schedule_be_retried_when_route_is_invalid() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				BOB.into(),
				ETH,
				1_000 * UNITS as i128,
			));
			let position_id = hydradx_runtime::Omnipool::next_position_id();

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				ETH,
				FixedU128::from_rational(3, 10),
				Permill::from_percent(60),
				BOB.into(),
			));

			rococo_run_to_block(12);

			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, ETH, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			//Remove ETH token resulting invalid route

			assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
				hydradx_runtime::RuntimeOrigin::root(),
				ETH,
				Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
			));
			let position =
				pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, BOB.into()).unwrap();
			assert_ok!(hydradx_runtime::Omnipool::remove_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
				position_id,
				position.shares,
			));

			assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
				hydradx_runtime::RuntimeOrigin::root(),
				ETH,
				Tradability::FROZEN
			));
			assert_ok!(hydradx_runtime::Omnipool::remove_token(
				hydradx_runtime::RuntimeOrigin::root(),
				ETH,
				BOB.into(),
			));
			rococo_run_to_block(14);

			//Assert
			let schedule_id = 0;
			let schedule = DCA::schedules(schedule_id);
			assert!(schedule.is_some());
			assert_eq!(DCA::retries_on_error(schedule_id), 1);
		});
	}

	#[test]
	fn insufficient_fee_asset_should_be_swapped_for_dot() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 10000 * UNITS, DAI, 20000 * UNITS);
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1000000000000);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					200 * UNITS as i128,
				));

				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					100 * UNITS,
					0,
					false
				));

				set_relaychain_block_number(11);

				//init_omnipool_with_oracle_for_block_10();
				let alice_init_insuff_balance = 10000000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					insufficient_asset,
					alice_init_insuff_balance as i128,
				));

				add_dot_as_payment_currency_with_details(100 * UNITS, FixedU128::from_rational(2000, 1));

				set_relaychain_block_number(12);

				let dca_budget = 500000 * UNITS;
				let amount_to_sell = 10000 * UNITS;
				let schedule1 = schedule_fake_with_sell_order(
					ALICE,
					PoolType::XYK,
					dca_budget,
					insufficient_asset,
					DOT,
					amount_to_sell,
				);

				let init_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());

				create_schedule(ALICE, schedule1.clone());

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);
				assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
				assert_reserved_balance!(&ALICE.into(), insufficient_asset, dca_budget);
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				//We get these xyk pool data before execution to later calculate the proper fee amount in insufficient asset
				let asset_pair_account =
					<hydradx_runtime::Runtime as pallet_xyk::Config>::AssetPairAccountId::from_assets(
						insufficient_asset,
						DOT,
						"xyk",
					);
				let in_reserve = Currencies::free_balance(insufficient_asset, &asset_pair_account.clone());
				let out_reserve = Currencies::free_balance(DOT, &asset_pair_account);

				//Act
				set_relaychain_block_number(14);

				//Assert
				let new_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
				assert_eq!(new_treasury_balance, init_treasury_balance);

				//No insufficient asset should be accumulated
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				let fee_in_dot = Currencies::free_balance(DOT, &Treasury::account_id());
				assert!(fee_in_dot > 0, "Treasury got rugged");

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);

				let fee_in_insufficient =
					hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, fee_in_dot).unwrap();
				let xyk_trade_fee_in_insufficient =
					hydra_dx_math::fee::calculate_pool_trade_fee(fee_in_insufficient, (3, 1000)).unwrap();

				assert_reserved_balance!(
					&ALICE.into(),
					insufficient_asset,
					dca_budget - amount_to_sell - fee_in_insufficient - xyk_trade_fee_in_insufficient
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn insufficient_fee_asset_should_be_swapped_for_dot_when_dot_reseve_is_relative_low() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();
				add_dot_as_payment_currency();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 10000 * UNITS, DAI, 20000 * UNITS);
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1200000 * UNITS);

				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					200 * UNITS as i128,
				));

				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					UNITS,
					0,
					false
				));

				set_relaychain_block_number(11);

				//init_omnipool_with_oracle_for_block_10();
				let alice_init_insuff_balance = 10000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					insufficient_asset,
					alice_init_insuff_balance as i128,
				));

				let dca_budget = 5000 * UNITS;
				let amount_to_sell = 100 * UNITS;
				let schedule1 = schedule_fake_with_sell_order(
					ALICE,
					PoolType::XYK,
					dca_budget,
					insufficient_asset,
					DOT,
					amount_to_sell,
				);

				let init_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());

				create_schedule(ALICE, schedule1);

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);
				assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
				assert_reserved_balance!(&ALICE.into(), insufficient_asset, dca_budget);
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				//We get these xyk pool data before execution to later calculate the proper fee amount in insufficient asset
				let asset_pair_account =
					<hydradx_runtime::Runtime as pallet_xyk::Config>::AssetPairAccountId::from_assets(
						insufficient_asset,
						DOT,
						"xyk",
					);
				let in_reserve = Currencies::free_balance(insufficient_asset, &asset_pair_account.clone());
				let out_reserve = Currencies::free_balance(DOT, &asset_pair_account);

				//Act
				set_relaychain_block_number(13);

				//Assert
				let new_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
				assert_eq!(new_treasury_balance, init_treasury_balance);

				//No insufficient asset should be accumulated
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				let fee_in_dot = Currencies::free_balance(DOT, &Treasury::account_id());
				assert!(fee_in_dot > 0, "Treasury got rugged");

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);

				let fee_in_insufficient =
					hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, fee_in_dot).unwrap();
				let xyk_trade_fee_in_insufficient =
					hydra_dx_math::fee::calculate_pool_trade_fee(fee_in_insufficient, (3, 1000)).unwrap();
				assert_reserved_balance!(
					&ALICE.into(),
					insufficient_asset,
					dca_budget - amount_to_sell - fee_in_insufficient - xyk_trade_fee_in_insufficient
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn insufficient_fee_asset_should_work_for_bigger_route() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();
				add_dot_as_payment_currency();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 10000 * UNITS, DAI, 20000 * UNITS);
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1200000 * UNITS);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					200 * UNITS as i128,
				));
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					UNITS,
					0,
					false
				));

				set_relaychain_block_number(11);

				//init_omnipool_with_oracle_for_block_10();
				let alice_init_insuff_balance = 10000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					insufficient_asset,
					alice_init_insuff_balance as i128,
				));
				let dca_budget = 5000 * UNITS;
				let amount_to_sell = 150 * UNITS;
				let route = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DOT,
						asset_out: HDX,
					},
				];
				let schedule1 = schedule_fake_with_sell_order_with_route(
					ALICE,
					dca_budget,
					insufficient_asset,
					HDX,
					amount_to_sell,
					route,
				);
				let init_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());

				create_schedule(ALICE, schedule1);

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);
				assert_reserved_balance!(&ALICE.into(), insufficient_asset, dca_budget);
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				//We get these xyk pool data before execution to later calculate the proper fee amount in insufficient asset
				let asset_pair_account =
					<hydradx_runtime::Runtime as pallet_xyk::Config>::AssetPairAccountId::from_assets(
						insufficient_asset,
						DOT,
						"xyk",
					);
				let in_reserve = Currencies::free_balance(insufficient_asset, &asset_pair_account.clone());
				let out_reserve = Currencies::free_balance(DOT, &asset_pair_account);

				//Act
				set_relaychain_block_number(13);

				//Assert
				let new_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
				assert_eq!(new_treasury_balance, init_treasury_balance);

				//No insufficient asset should be accumulated
				assert_balance!(&Treasury::account_id(), insufficient_asset, 0);

				let fee_in_dot = Currencies::free_balance(DOT, &Treasury::account_id());
				assert!(fee_in_dot > 0, "Treasury got rugged");

				assert_balance!(ALICE.into(), insufficient_asset, alice_init_insuff_balance - dca_budget);

				let fee_in_insufficient =
					hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, fee_in_dot).unwrap();
				let xyk_trade_fee_in_insufficient =
					hydra_dx_math::fee::calculate_pool_trade_fee(fee_in_insufficient, (3, 1000)).unwrap();
				assert_reserved_balance!(
					&ALICE.into(),
					insufficient_asset,
					dca_budget - amount_to_sell - fee_in_insufficient - xyk_trade_fee_in_insufficient
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sufficient_but_not_accepted_fee_asset_should_be_swapped_for_dot() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();

				let name = b"INSUF1".to_vec();
				let sufficient_asset = AssetRegistry::register_sufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					1_000,
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(sufficient_asset, 10000 * UNITS, DAI, 20000 * UNITS);
				create_xyk_pool(sufficient_asset, 1000000 * UNITS, DOT, 1000000000000);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, sufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					sufficient_asset,
					200 * UNITS as i128,
				));

				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					sufficient_asset,
					DOT,
					100 * UNITS,
					0,
					false
				));

				set_relaychain_block_number(11);

				//init_omnipool_with_oracle_for_block_10();
				let alice_init_suff_balance = 10000000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					sufficient_asset,
					alice_init_suff_balance as i128,
				));

				add_dot_as_payment_currency_with_details(100 * UNITS, FixedU128::from_rational(2000, 1));

				set_relaychain_block_number(12);

				let dca_budget = 500000 * UNITS;
				let amount_to_sell = 10000 * UNITS;
				let schedule1 = schedule_fake_with_sell_order(
					ALICE,
					PoolType::XYK,
					dca_budget,
					sufficient_asset,
					DOT,
					amount_to_sell,
				);

				let init_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());

				create_schedule(ALICE, schedule1.clone());

				assert_balance!(ALICE.into(), sufficient_asset, alice_init_suff_balance - dca_budget);
				assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
				assert_reserved_balance!(&ALICE.into(), sufficient_asset, dca_budget);
				assert_balance!(&Treasury::account_id(), sufficient_asset, 0);

				//We get these xyk pool data before execution to later calculate the proper fee amount in sufficient asset
				let asset_pair_account =
					<hydradx_runtime::Runtime as pallet_xyk::Config>::AssetPairAccountId::from_assets(
						sufficient_asset,
						DOT,
						"xyk",
					);
				let in_reserve = Currencies::free_balance(sufficient_asset, &asset_pair_account.clone());
				let out_reserve = Currencies::free_balance(DOT, &asset_pair_account);

				//Act
				set_relaychain_block_number(14);

				//Assert
				let new_treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
				assert_eq!(new_treasury_balance, init_treasury_balance);

				//No sufficient (but non fee payment) asset should be accumulated
				assert_balance!(&Treasury::account_id(), sufficient_asset, 0);

				let fee_in_dot = Currencies::free_balance(DOT, &Treasury::account_id());
				assert!(fee_in_dot > 0, "Treasury got rugged");

				assert_balance!(ALICE.into(), sufficient_asset, alice_init_suff_balance - dca_budget);

				let fee_in_sufficient =
					hydra_dx_math::xyk::calculate_in_given_out(out_reserve, in_reserve, fee_in_dot).unwrap();
				let xyk_trade_fee_in_sufficient =
					hydra_dx_math::fee::calculate_pool_trade_fee(fee_in_sufficient, (3, 1000)).unwrap();

				assert_reserved_balance!(
					&ALICE.into(),
					sufficient_asset,
					dca_budget - amount_to_sell - fee_in_sufficient - xyk_trade_fee_in_sufficient
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_schedule_execution_should_work_without_route() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1000 * UNITS;

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance);

			let amount_in = 100 * UNITS;
			let no_route = vec![];
			let schedule1 = schedule_fake_with_sell_order_with_route(ALICE, dca_budget, HDX, DAI, amount_in, no_route);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
			assert!(fee > 0, "Treasury got rugged");

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, 2071214372591672);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_in - fee);
		});
	}

	#[test]
	fn sell_schedule_should_be_terminated_after_retries() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule1 = Schedule {
				owner: AccountId::from(ALICE),
				period: 5u32,
				total_amount: dca_budget,
				max_retries: None,
				stability_threshold: None,
				slippage: Some(Permill::from_percent(1)),
				order: Order::Sell {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: amount_to_sell,
					min_amount_out: Balance::MAX,
					route: create_bounded_vec(vec![Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DAI,
					}]),
				},
			};
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act and Assert
			let schedule_id = 0;

			set_relaychain_block_number(12);
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - fee);

			assert_eq!(DCA::retries_on_error(schedule_id), 1);

			set_relaychain_block_number(22);
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 2 * fee);
			assert_eq!(DCA::retries_on_error(schedule_id), 2);

			set_relaychain_block_number(42);
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 3 * fee);
			assert_eq!(DCA::retries_on_error(schedule_id), 3);

			//At this point, the schedule will be terminated as retries max number of times
			set_relaychain_block_number(82);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - 4 * fee);
			assert_reserved_balance!(&ALICE.into(), HDX, 0);
			assert_eq!(DCA::retries_on_error(schedule_id), 0);
			let schedule = DCA::schedules(schedule_id);
			assert!(schedule.is_none());
		});
	}

	#[test]
	fn sell_schedule_execution_should_work_when_hub_asset_is_sold() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let alice_init_hub_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_hub_balance);

			let dca_budget = 2500 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, LRNA, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
			assert_balance!(&Treasury::account_id(), LRNA, 0);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let treasury_balance = Currencies::free_balance(LRNA, &Treasury::account_id());
			assert!(treasury_balance > 0);

			assert_balance!(ALICE.into(), DAI, 2142499995765714);
			assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), LRNA, 2399535878687816);
		});
	}

	#[test]
	fn sell_schedule_and_direct_omnipool_sell_and_router_should_yield_same_result_when_native_asset_sold() {
		let amount_to_sell = 100 * UNITS;

		//DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 1100 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);

			assert_balance!(ALICE.into(), DAI, 2071214372591672);
		});

		//Direct Omnipool
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			//Act
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				amount_to_sell,
				0,
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - amount_to_sell);
			assert_balance!(ALICE.into(), DAI, 2071214372591672);
		});

		//Router
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			//Act
			let trade = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				amount_to_sell,
				0,
				trade
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - amount_to_sell);
			assert_balance!(ALICE.into(), DAI, 2071214372591672);
		});
	}

	#[test]
	fn sell_schedule_and_direct_omnipool_sell_and_router_should_yield_same_result_when_hub_asset_sold() {
		let amount_to_sell = 100 * UNITS;

		//DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			let dca_budget = 1100 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, LRNA, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(LRNA, &Treasury::account_id());
			assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget - amount_to_sell - fee);

			assert_balance!(ALICE.into(), DAI, 2142499995765714);
		});

		//Direct omnipool
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

			//Act
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(ALICE.into()),
				LRNA,
				DAI,
				amount_to_sell,
				0,
			));

			//Assert
			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - amount_to_sell);
			assert_balance!(ALICE.into(), DAI, 2142499995765714);
		});

		//Router
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();
			let alice_init_lrna_balance = 5000 * UNITS;
			set_alice_lrna_balance(alice_init_lrna_balance);

			//Act
			let trade = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: DAI,
			}];
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				LRNA,
				DAI,
				amount_to_sell,
				0,
				trade
			));

			//Assert
			assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - amount_to_sell);
			assert_balance!(ALICE.into(), DAI, 2142499995765714);
		});
	}

	#[test]
	fn full_sell_dca_should_be_executed_then_completed() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			init_omnipool_with_oracle_for_block_10();

			let amount_to_sell = 200 * UNITS;
			let dca_budget = 1200 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act
			run_to_block(11, 100);

			//Assert
			let new_dai_balance = Currencies::free_balance(DAI, &ALICE.into());
			assert!(new_dai_balance > ALICE_INITIAL_DAI_BALANCE);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, 0);

			let schedule = DCA::schedules(0);
			assert!(schedule.is_none());

			check_if_dcas_completed_without_failed_or_terminated_events();
		});
	}

	#[test]
	fn full_sell_dca_should_be_executed_then_completed_for_multiple_users() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let bob_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				BOB.into(),
				bob_init_hdx_balance,
			));

			init_omnipool_with_oracle_for_block_10();

			let amount_to_sell = 200 * UNITS;
			let dca_budget = 1000 * UNITS;
			let dca_budget_for_bob = 1200 * UNITS;

			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule2 =
				schedule_fake_with_sell_order(BOB, PoolType::Omnipool, dca_budget_for_bob, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);
			create_schedule(BOB, schedule2);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), HDX, bob_init_hdx_balance - dca_budget_for_bob);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_reserved_balance!(&BOB.into(), HDX, dca_budget_for_bob);

			//Act
			run_to_block(11, 100);

			//Assert
			check_if_no_failed_events();

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(BOB.into(), HDX, bob_init_hdx_balance - dca_budget_for_bob);
			assert_reserved_balance!(&ALICE.into(), HDX, 0);
			assert_reserved_balance!(&BOB.into(), HDX, 0);

			let schedule = DCA::schedules(0);
			assert!(schedule.is_none());

			let schedule = DCA::schedules(1);
			assert!(schedule.is_none());

			check_if_dcas_completed_without_failed_or_terminated_events();
		});
	}

	#[test]
	fn multiple_full_sell_dca_should_be_executed_then_completed_for_same_user() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let alice_init_hdx_balance = 50000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			init_omnipool_with_oracle_for_block_10();

			//Trade 1
			let amount_to_sell1 = 200 * UNITS;
			let dca_budget1 = 1000 * UNITS;
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget1, HDX, DAI, amount_to_sell1);
			create_schedule(ALICE, schedule1);

			//Trade 2
			let amount_to_sell2 = 220 * UNITS;
			let dca_budget2 = 1500 * UNITS;
			let schedule2 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget2, HDX, DAI, amount_to_sell2);
			create_schedule(ALICE, schedule2);

			//Trade 3
			let amount_to_sell3 = 800 * UNITS;
			let dca_budget3 = 2000 * UNITS;
			let schedule3 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget3, HDX, DAI, amount_to_sell3);
			create_schedule(ALICE, schedule3);

			let budget_for_all_trades = dca_budget1 + dca_budget2 + dca_budget3;
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - budget_for_all_trades);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, budget_for_all_trades);

			//Act
			run_to_block(11, 100);

			//Assert
			assert_reserved_balance!(&ALICE.into(), HDX, 0);

			let schedule = DCA::schedules(0);
			assert!(schedule.is_none());

			let schedule = DCA::schedules(1);
			assert!(schedule.is_none());

			let schedule = DCA::schedules(2);
			assert!(schedule.is_none());

			check_if_dcas_completed_without_failed_or_terminated_events();
		});
	}

	#[test]
	fn schedules_should_be_ordered_based_on_random_number_when_executed_in_a_block() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			let native_amount = 100000 * UNITS;
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				HDX,
				native_amount as i128,
			));

			init_omnipool_with_oracle_for_block_10();

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;

			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule2 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule3 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule4 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule5 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			let schedule6 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);

			create_schedule(ALICE, schedule1);
			create_schedule(ALICE, schedule2);
			create_schedule(ALICE, schedule3);
			create_schedule(ALICE, schedule4);
			create_schedule(ALICE, schedule5);
			create_schedule(ALICE, schedule6);

			//Act
			run_to_block(11, 12);

			//Assert
			//We check if the schedules are processed not in the order they were created,
			// ensuring that they are sorted based on randomness
			assert_ne!(
				vec![0, 1, 2, 3, 4, 5],
				get_last_schedule_ids_from_trade_executed_events()
			)
		});
	}

	#[test]
	fn sell_schedule_should_work_when_user_has_left_less_than_existential_deposit() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipool_with_oracle_for_block_10();

			let amount_to_sell = 1000 * UNITS;
			let fee = DCA::get_transaction_fee(&Order::Sell {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				min_amount_out: Balance::MIN,
				route: create_bounded_vec(vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				}]),
			})
			.unwrap();

			let alice_init_hdx_balance = 2 * (1000 * UNITS + fee) + 1;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let dca_budget = 2 * (1000 * UNITS + fee);
			let schedule1 =
				schedule_fake_with_sell_order(ALICE, PoolType::Omnipool, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

			//Act
			run_to_block(11, 17);

			//Assert
			check_if_no_failed_events();
			assert_reserved_balance!(&ALICE.into(), HDX, 0);
			assert_balance!(ALICE.into(), HDX, 0);
		});
	}
}

mod fee {
	use super::*;
	use frame_support::assert_ok;
	use hydradx_runtime::DCA;
	use hydradx_traits::AssetKind;
	use sp_runtime::{FixedU128, TransactionOutcome};

	#[test]
	fn sell_tx_fee_should_be_more_for_insufficient_asset() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();
				add_dot_as_payment_currency();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1000000 * UNITS);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					200 * UNITS as i128,
				));
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					UNITS,
					0,
					false
				));

				//Arrange
				let sell_with_hdx_fee = Order::Sell {
					asset_in: DOT,
					asset_out: insufficient_asset,
					amount_in: 10000 * UNITS,
					min_amount_out: UNITS,
					route: create_bounded_vec(vec![]),
				};

				let sell_with_insufficient_fee = Order::Sell {
					asset_in: insufficient_asset,
					asset_out: DOT,
					amount_in: 10000 * UNITS,
					min_amount_out: UNITS,
					route: create_bounded_vec(vec![]),
				};

				set_relaychain_block_number(11);

				//Assert
				let fee_for_dot = DCA::get_transaction_fee(&sell_with_hdx_fee).unwrap();
				let fee_for_insufficient = DCA::get_transaction_fee(&sell_with_insufficient_fee).unwrap();

				let diff = fee_for_insufficient - fee_for_dot;
				let relative_fee_difference = FixedU128::from_rational(diff, fee_for_dot);
				let min_difference = FixedU128::from_rational(10, 100);

				//The fee with insufficient asset fee should be significantly bigger as involves more reads/writes, also due to buy swap
				assert!(relative_fee_difference > min_difference);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_tx_fee_should_be_more_for_insufficient_asset() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

				//Arrange
				init_omnipool_with_oracle_for_block_10();
				add_dot_as_payment_currency();

				let name = b"INSUF1".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 1000000 * UNITS);
				assert_ok!(hydradx_runtime::EmaOracle::add_oracle(
					RuntimeOrigin::root(),
					primitives::constants::chain::XYK_SOURCE,
					(DOT, insufficient_asset)
				));
				//Populate oracLe
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					insufficient_asset,
					200 * UNITS as i128,
				));
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					insufficient_asset,
					DOT,
					UNITS,
					0,
					false
				));

				//Arrange
				let buy_with_hdx_fee = Order::Buy {
					asset_in: DOT,
					asset_out: insufficient_asset,
					amount_out: 10000 * UNITS,
					max_amount_in: u128::MAX,
					route: create_bounded_vec(vec![]),
				};

				let buy_with_insufficient_fee = Order::Buy {
					asset_in: insufficient_asset,
					asset_out: DOT,
					amount_out: 10000 * UNITS,
					max_amount_in: u128::MAX,
					route: create_bounded_vec(vec![]),
				};

				set_relaychain_block_number(11);

				let fee_for_dot = DCA::get_transaction_fee(&buy_with_hdx_fee).unwrap();
				let fee_for_insufficient = DCA::get_transaction_fee(&buy_with_insufficient_fee).unwrap();

				let diff = fee_for_insufficient - fee_for_dot;
				let relative_fee_difference = FixedU128::from_rational(diff, fee_for_dot);
				let min_difference = FixedU128::from_rational(10, 100);

				//The fee with insufficient asset fee should be significantly bigger as involves more reads/writes, also due to buy swap
				assert!(relative_fee_difference > min_difference);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

mod stableswap {
	use super::*;

	#[test]
	fn sell_should_work_when_two_stableassets_swapped() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, asset_a, asset_b) = init_stableswap().unwrap();

				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					RuntimeOrigin::root(),
					asset_a,
					FixedU128::from_rational(88, 100),
				));

				let alice_init_asset_a_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					asset_a,
					alice_init_asset_a_balance as i128,
				));

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					asset_a,
					5000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					asset_a,
					asset_b,
					100 * UNITS,
					0u128,
				));

				let dca_budget = 1100 * UNITS;
				let amount_to_sell = 100 * UNITS;
				let schedule1 = schedule_fake_with_sell_order(
					ALICE,
					PoolType::Stableswap(pool_id),
					dca_budget,
					asset_a,
					asset_b,
					amount_to_sell,
				);
				set_relaychain_block_number(10);

				create_schedule(ALICE, schedule1);

				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, 0);
				assert_reserved_balance!(&ALICE.into(), asset_a, dca_budget);
				assert_balance!(&Treasury::account_id(), asset_a, 0);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(asset_a, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, 98999999706917);
				assert_reserved_balance!(&ALICE.into(), asset_a, dca_budget - amount_to_sell - fee);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn two_stableswap_asssets_should_be_swapped_when_they_have_different_decimals() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, asset_a, asset_b) =
					init_stableswap_with_three_assets_having_different_decimals().unwrap();

				//Populate oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					asset_b,
					5000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					asset_a,
					asset_b,
					10_000_000,
					0u128,
				));

				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					RuntimeOrigin::root(),
					asset_a,
					FixedU128::from_rational(88, 100),
				));

				let alice_init_asset_a_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					asset_a,
					alice_init_asset_a_balance as i128,
				));

				let dca_budget = 1100 * UNITS;
				let amount_to_sell = 100 * UNITS;
				let schedule1 = schedule_fake_with_sell_order(
					ALICE,
					PoolType::Stableswap(pool_id),
					dca_budget,
					asset_a,
					asset_b,
					amount_to_sell,
				);
				set_relaychain_block_number(10);

				create_schedule(ALICE, schedule1);

				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, 0);
				assert_reserved_balance!(&ALICE.into(), asset_a, dca_budget);
				assert_balance!(&Treasury::account_id(), asset_a, 0);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(asset_a, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, 93176719400532);
				assert_reserved_balance!(&ALICE.into(), asset_a, dca_budget - amount_to_sell - fee);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_omnipool_and_stable_trades() {
		let amount_to_sell = 200 * UNITS;
		let amount_to_receive = 197218633037918;
		//With DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					10000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					30_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];
				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Sell {
						asset_in: HDX,
						asset_out: stable_asset_1,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(trades),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, 0);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
				assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_receive);

				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);

				let treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
				assert!(treasury_balance > TREASURY_ACCOUNT_INIT_BALANCE);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Do the same in with pool trades
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					10000 * UNITS,
					0,
				));

				init_omnipol();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					30_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				set_relaychain_block_number(10);

				//Act
				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(ALICE.into()),
					HDX,
					pool_id,
					amount_to_sell,
					0,
				));

				let pool_id_balance = Currencies::free_balance(pool_id, &AccountId::from(ALICE));

				assert_ok!(Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(ALICE.into()),
					pool_id,
					stable_asset_1,
					pool_id_balance,
					0
				));

				//Assert
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_receive);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Do the same with plain router
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					10000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					30_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];

				assert_ok!(Router::sell(
					RuntimeOrigin::signed(ALICE.into()),
					HDX,
					stable_asset_1,
					amount_to_sell,
					0,
					trades
				));

				//Assert
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - amount_to_sell);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_receive);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_stable_trades_and_omnipool() {
		let amount_to_sell = 100 * UNITS;
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//To populate stableswap oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					100 * UNITS,
					0,
				));

				//Set stable asset 1 as accepted payment currency
				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					RuntimeOrigin::root(),
					stable_asset_1,
					FixedU128::from_rational(50, 100),
				));

				//Init omnipool and add pool id as token
				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					3000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				//Populate oracle with omnipool source
				assert_ok!(Tokens::set_balance(
					RawOrigin::Root.into(),
					CHARLIE.into(),
					pool_id,
					1000 * UNITS,
					0,
				));

				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					HDX,
					500 * UNITS,
					Balance::MIN
				));

				set_relaychain_block_number(1000);

				let alice_init_stable1_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance as i128,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: stable_asset_1,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: pool_id,
						asset_out: HDX,
					},
				];
				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Sell {
						asset_in: stable_asset_1,
						asset_out: HDX,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(trades),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), stable_asset_1, alice_init_stable1_balance - dca_budget);
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
				assert_reserved_balance!(&ALICE.into(), stable_asset_1, dca_budget);
				assert_balance!(&Treasury::account_id(), stable_asset_1, 0);

				//Act
				set_relaychain_block_number(1002);

				//Assert
				let fee = Currencies::free_balance(stable_asset_1, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), stable_asset_1, alice_init_stable1_balance - dca_budget);
				assert_balance!(ALICE.into(), HDX, 1070832735983195);

				assert_reserved_balance!(&ALICE.into(), stable_asset_1, dca_budget - amount_to_sell - fee);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Do the same in with pool trades
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//To populate stableswap oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					100 * UNITS,
					0,
				));

				init_omnipol();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					3000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);

				//Populate oracle with omnipool source
				assert_ok!(Tokens::set_balance(
					RawOrigin::Root.into(),
					CHARLIE.into(),
					pool_id,
					1000 * UNITS,
					0,
				));
				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					HDX,
					500 * UNITS,
					Balance::MIN
				));

				let alice_init_stable1_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance as i128,
				));

				assert_balance!(ALICE.into(), pool_id, 0);

				set_relaychain_block_number(10);

				//Act
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(ALICE.into()),
					pool_id,
					BoundedVec::truncate_from(vec![AssetAmount {
						asset_id: stable_asset_1,
						amount: amount_to_sell,
					}]),
				));
				let alice_pool_id_balance = Currencies::free_balance(pool_id, &AccountId::from(ALICE));

				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(ALICE.into()),
					pool_id,
					HDX,
					alice_pool_id_balance,
					0,
				));

				//Assert
				assert_balance!(
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance - amount_to_sell
				);
				assert_balance!(ALICE.into(), HDX, 1070832735983195);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		//Do the same with plain router
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//To populate stableswap oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					100 * UNITS,
					0,
				));

				init_omnipol();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					3000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);

				//Populate oracle with omnipool source
				assert_ok!(Tokens::set_balance(
					RawOrigin::Root.into(),
					CHARLIE.into(),
					pool_id,
					1000 * UNITS,
					0,
				));
				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					HDX,
					500 * UNITS,
					Balance::MIN
				));

				let alice_init_stable1_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance as i128,
				));

				assert_balance!(ALICE.into(), pool_id, 0);

				set_relaychain_block_number(10);

				//Act
				let trades = vec![
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: stable_asset_1,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: pool_id,
						asset_out: HDX,
					},
				];
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(ALICE.into()),
					stable_asset_1,
					HDX,
					amount_to_sell,
					0,
					trades
				));

				//Assert
				assert_balance!(
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance - amount_to_sell
				);
				assert_balance!(ALICE.into(), HDX, 1070832735983195);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_should_work_with_omnipool_and_stable_trades() {
		let amount_to_buy = 200 * UNITS;
		//With DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//To populate stableswap oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					3000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					3000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				do_trade_to_populate_oracle(DAI, HDX, UNITS);
				set_zero_reward_for_referrals(pool_id);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];
				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Buy {
						asset_in: HDX,
						asset_out: stable_asset_1,
						amount_out: amount_to_buy,
						max_amount_in: Balance::MAX,
						route: create_bounded_vec(trades),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, 0);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
				assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_buy);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_should_work_when_two_stableassets_swapped() {
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, asset_a, asset_b) = init_stableswap().unwrap();

				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					RuntimeOrigin::root(),
					asset_a,
					FixedU128::from_rational(88, 100),
				));

				let alice_init_asset_a_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					asset_a,
					alice_init_asset_a_balance as i128,
				));

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					asset_a,
					5000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					asset_a,
					asset_b,
					100 * UNITS,
					0u128,
				));

				let dca_budget = 1100 * UNITS;
				let amount_to_buy = 100 * UNITS;
				let schedule1 = schedule_fake_with_buy_order(
					PoolType::Stableswap(pool_id),
					asset_a,
					asset_b,
					amount_to_buy,
					dca_budget,
				);
				set_relaychain_block_number(12);

				create_schedule(ALICE, schedule1);

				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, 0);
				assert_reserved_balance!(&ALICE.into(), asset_a, dca_budget);
				assert_balance!(&Treasury::account_id(), asset_a, 0);

				//Act
				set_relaychain_block_number(14);

				//Assert
				let fee = Currencies::free_balance(asset_a, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), asset_a, alice_init_asset_a_balance - dca_budget);
				assert_balance!(ALICE.into(), asset_b, amount_to_buy);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_should_work_with_stable_trades_and_omnipool() {
		let amount_to_buy = 100 * UNITS;
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//Set stable asset 1 as accepted payment currency
				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					RuntimeOrigin::root(),
					stable_asset_1,
					FixedU128::from_rational(50, 100),
				));

				//For populating oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					5000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					1000 * UNITS,
					0u128,
				));

				//Init omnipool and add pool id as token
				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					3000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(pool_id, HDX, 100 * UNITS);

				set_relaychain_block_number(10);

				let alice_init_stable1_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					alice_init_stable1_balance as i128,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: stable_asset_1,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: pool_id,
						asset_out: HDX,
					},
				];
				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(70)),
					order: Order::Buy {
						asset_in: stable_asset_1,
						asset_out: HDX,
						amount_out: amount_to_buy,
						max_amount_in: Balance::MAX,
						route: create_bounded_vec(trades),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), stable_asset_1, alice_init_stable1_balance - dca_budget);
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
				assert_reserved_balance!(&ALICE.into(), stable_asset_1, dca_budget);
				assert_balance!(&Treasury::account_id(), stable_asset_1, 0);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(stable_asset_1, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), stable_asset_1, alice_init_stable1_balance - dca_budget);
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE + amount_to_buy);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

mod xyk {
	use super::*;

	#[test]
	fn sell_should_work_for_xyk() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				BOB.into(),
				5000 * UNITS,
			));

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				DAI,
				5000 * UNITS,
				0,
			));

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				1000 * UNITS,
				DAI,
				2000 * UNITS,
			));

			//For populating oracle
			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				false
			));

			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			set_relaychain_block_number(10);

			let dca_budget = 1100 * UNITS;
			let amount_to_sell = 100 * UNITS;
			let schedule1 = schedule_fake_with_sell_order(ALICE, PoolType::XYK, dca_budget, HDX, DAI, amount_to_sell);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
			let treasury_init_balance = Balances::free_balance(Treasury::account_id());

			//Act
			set_relaychain_block_number(12);

			//Assert
			let amount_out = 151105924242426;
			let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - treasury_init_balance;

			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);
		});
	}

	#[test]
	fn buy_should_work_for_xyk() {
		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, 1000 * UNITS, DAI, 2000 * UNITS);

			//For populating oracle
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				HDX,
				200 * UNITS as i128,
			));
			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				false
			));

			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			set_relaychain_block_number(10);

			let dca_budget = 1100 * UNITS;
			let amount_to_buy = 150 * UNITS;
			let schedule1 = schedule_fake_with_buy_order(PoolType::XYK, HDX, DAI, amount_to_buy, dca_budget);
			create_schedule(ALICE, schedule1);

			assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
			assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

			//Act
			set_relaychain_block_number(12);

			//Assert
			assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_to_buy);
		});
	}
}

mod all_pools {
	use super::*;

	#[test]
	fn sell_should_work_with_3_different_pools() {
		let amount_to_sell = 200 * UNITS;

		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				//Create stableswap and populate oracle
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					1000 * UNITS,
					0,
				));

				//Create omnipool and populate oracle
				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					1000000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				//Create xyk and populate oracle
				create_xyk_pool(stable_asset_1, 10000 * UNITS, DAI, 20000 * UNITS);
				assert_ok!(Currencies::update_balance(
					RawOrigin::Root.into(),
					BOB.into(),
					stable_asset_1,
					200 * UNITS as i128,
				));
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(BOB.into()),
					stable_asset_1,
					DAI,
					100 * UNITS,
					0,
					false
				));

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_1,
						asset_out: DAI,
					},
				];
				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(15)),
					order: Order::Sell {
						asset_in: HDX,
						asset_out: DAI,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(trades),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
				assert_balance!(
					&Treasury::account_id(),
					HDX,
					TREASURY_ACCOUNT_INIT_BALANCE + InsufficientEDinHDX::get()
				);

				//Act
				set_relaychain_block_number(12);

				//Assert
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), DAI, 2380211607465609);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

fn create_xyk_pool(asset_a: AssetId, amount_a: Balance, asset_b: AssetId, amount_b: Balance) {
	//Arrange
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));

	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

mod with_onchain_route {
	use super::*;
	use frame_support::pallet_prelude::DispatchResult;
	use hydradx_traits::router::PoolType;

	#[test]
	fn buy_should_work_with_omnipool_and_stable_with_onchain_routes() {
		let amount_to_buy = 200 * UNITS;
		//With DCA
		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				//To populate stableswap oracle
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					3000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					300_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];

				let asset_pair = AssetPair::new(HDX, stable_asset_1);
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					trades.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), trades);

				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Buy {
						asset_in: HDX,
						asset_out: stable_asset_1,
						amount_out: amount_to_buy,
						max_amount_in: Balance::MAX,
						route: create_bounded_vec(vec![]),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, 0);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
				assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

				//Act
				set_relaychain_block_number(12);

				//Assert
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_buy);

				assert_balance!(Router::router_account(), HDX, 0);
				assert_balance!(Router::router_account(), stable_asset_1, 0);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_omnipool_and_stable_trades_with_onchain_routes() {
		let amount_to_sell = 200 * UNITS;
		let amount_to_receive = 187172768546856u128;

		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					10000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					300_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(DAI, HDX, UNITS);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];

				let asset_pair = AssetPair::new(HDX, stable_asset_1);
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					trades.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), trades);

				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Sell {
						asset_in: HDX,
						asset_out: stable_asset_1,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(vec![]),
					},
				};

				create_schedule(ALICE, schedule);

				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, 0);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
				assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
				assert!(fee > 0, "The treasury did not receive the fee");
				assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
				assert_balance!(ALICE.into(), stable_asset_1, amount_to_receive);
				assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);

				assert_balance!(Router::router_account(), HDX, 0);
				assert_balance!(Router::router_account(), stable_asset_1, 0);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn schedule_should_work_when_fee_asset_is_nonnative_omni_asset() {
		let amount_to_sell = 2000 * UNITS;

		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipol();
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				1_000_000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(50, 100),
			));

			assert_ok!(Omnipool::add_token(
				RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(10, 100),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));
			set_zero_reward_for_referrals(DOT);
			do_trade_to_populate_oracle(DAI, HDX, UNITS);

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE.into(),
				DOT,
				50000 * UNITS as i128,
			));
			let alice_init_dot_balance = 50000 * UNITS + ALICE_INITIAL_DOT_BALANCE;

			set_relaychain_block_number(10);

			let dca_budget = 10000 * UNITS;

			let schedule = Schedule {
				owner: AccountId::from(ALICE),
				period: 5u32,
				total_amount: dca_budget,
				max_retries: None,
				stability_threshold: None,
				slippage: Some(Permill::from_percent(10)),
				order: Order::Sell {
					asset_in: DOT,
					asset_out: HDX,
					amount_in: amount_to_sell,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![]),
				},
			};

			//We verify the price diff between hdx and stable asset.
			//If we sell 3703744780645, we receive 18462849173515,
			// so something like 5x more, so fee should be 5x than normal HDX
			let _dot_amount = with_transaction::<_, _, _>(|| {
				let amount_to_sell = 3703744780645;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					DOT,
					amount_to_sell,
					0,
					vec![]
				));
				let alice_received_dot =
					Currencies::free_balance(DOT, &AccountId::from(ALICE)) - alice_init_dot_balance;

				TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
			})
			.unwrap();

			assert_balance!(ALICE.into(), DOT, alice_init_dot_balance);

			create_schedule(ALICE, schedule);

			assert_balance!(ALICE.into(), DOT, alice_init_dot_balance - dca_budget);
			assert_reserved_balance!(&ALICE.into(), DOT, dca_budget);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(DOT, &Treasury::account_id());
			assert!(fee > 0, "The treasury did not receive the fee");

			assert_balance!(ALICE.into(), DOT, alice_init_dot_balance - dca_budget);
			assert_balance!(ALICE.into(), HDX, 1398004528624518);

			assert_reserved_balance!(&ALICE.into(), DOT, dca_budget - amount_to_sell - fee);
		});
	}

	#[test]
	fn schedule_should_work_when_stable_asset_used_as_fee_asset() {
		let amount_to_sell = 200 * UNITS;

		TestNet::reset();
		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

				assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
					hydradx_runtime::RuntimeOrigin::root(),
					stable_asset_1,
					FixedU128::from_rational(50, 100),
				));

				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					CHARLIE.into(),
					stable_asset_1,
					10000 * UNITS as i128,
				));
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(CHARLIE.into()),
					pool_id,
					stable_asset_1,
					stable_asset_2,
					10000 * UNITS,
					0,
				));

				init_omnipol();
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					300_000_000 * UNITS as i128,
				));

				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(50, 100),
					Permill::from_percent(100),
					AccountId::from(BOB),
				));
				set_zero_reward_for_referrals(pool_id);
				do_trade_to_populate_oracle(pool_id, HDX, 10000000 * UNITS);

				set_relaychain_block_number(10);

				let alice_init_hdx_balance = 5000 * UNITS;
				assert_ok!(Balances::force_set_balance(
					RawOrigin::Root.into(),
					ALICE.into(),
					alice_init_hdx_balance,
				));

				let alice_init_stable_balance = 5000 * UNITS;
				assert_ok!(Currencies::update_balance(
					RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					alice_init_stable_balance as i128,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];

				let asset_pair = AssetPair::new(HDX, stable_asset_1);
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					trades.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), trades);

				let dca_budget = 1100 * UNITS;

				let schedule = Schedule {
					owner: AccountId::from(ALICE),
					period: 5u32,
					total_amount: dca_budget,
					max_retries: None,
					stability_threshold: None,
					slippage: Some(Permill::from_percent(10)),
					order: Order::Sell {
						asset_in: stable_asset_1,
						asset_out: HDX,
						amount_in: amount_to_sell,
						min_amount_out: Balance::MIN,
						route: create_bounded_vec(vec![]),
					},
				};

				//We verify the price diff between hdx and stable asset.
				//If we sell 6503744780645, we receive 5385180382312
				//So fee should be like 0.8x normal HDX fee
				let _stable_amount = with_transaction::<_, _, _>(|| {
					let amount_to_sell = 6503744780645;
					assert_ok!(Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						HDX,
						stable_asset_1,
						amount_to_sell,
						0,
						vec![]
					));
					let alice_received_stable =
						Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)) - alice_init_stable_balance;

					TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_stable))
				})
				.unwrap();

				create_schedule(ALICE, schedule);

				//Act
				set_relaychain_block_number(12);

				//Assert
				let fee = Currencies::free_balance(stable_asset_1, &Treasury::account_id());
				assert!(fee > 0, "The treasury did not receive the fee");

				assert_balance!(ALICE.into(), stable_asset_1, alice_init_stable_balance - dca_budget);
				assert!(Currencies::free_balance(HDX, &ALICE.into()) > alice_init_hdx_balance);

				assert_reserved_balance!(&ALICE.into(), stable_asset_1, dca_budget - amount_to_sell - fee);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn schedule_should_work_when_xyk_asset_used_as_fee_asset() {
		let amount_to_sell = 200 * UNITS;

		TestNet::reset();
		Hydra::execute_with(|| {
			//Arrange
			init_omnipol();

			do_trade_to_populate_oracle(DAI, HDX, 10000000 * UNITS);

			create_xyk_pool_with_amounts(DAI, 10000000000 * UNITS, DOT, 10000000000 * UNITS);
			assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(50, 100),
			));

			//Populate xyk
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				DAVE.into(),
				DAI,
				10000000 * UNITS as i128,
			));
			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(DAVE.into()),
				DAI,
				DOT,
				10000000 * UNITS,
				u128::MIN,
				false
			));

			set_relaychain_block_number(10);

			let alice_init_hdx_balance = 5000 * UNITS;
			assert_ok!(Balances::force_set_balance(
				RawOrigin::Root.into(),
				ALICE.into(),
				alice_init_hdx_balance,
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			let asset_pair = AssetPair::new(HDX, DOT);
			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				trades.clone()
			));
			assert_eq!(Router::route(asset_pair).unwrap(), trades);

			let dca_budget = 1100 * UNITS;

			let schedule = Schedule {
				owner: AccountId::from(ALICE),
				period: 5u32,
				total_amount: dca_budget,
				max_retries: None,
				stability_threshold: None,
				slippage: Some(Permill::from_percent(10)),
				order: Order::Sell {
					asset_in: DOT,
					asset_out: HDX,
					amount_in: amount_to_sell,
					min_amount_out: Balance::MIN,
					route: create_bounded_vec(vec![]),
				},
			};

			//Just to verify the price difference between HDX and DOT
			//Selling 3795361512418 HDX results 2694204333872 DOT
			//So fee should be 0.7x normal HDX feee
			let _dot_amount_out = with_transaction::<_, _, _>(|| {
				let fee_in_hdx = 3795361512418;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					DOT,
					fee_in_hdx,
					0,
					vec![]
				));
				let alice_received_dot =
					Currencies::free_balance(DOT, &AccountId::from(ALICE)) - ALICE_INITIAL_DOT_BALANCE;

				TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
			})
			.unwrap();

			create_schedule(ALICE, schedule);

			//Act
			set_relaychain_block_number(12);

			//Assert
			let fee = Currencies::free_balance(DOT, &Treasury::account_id());
			assert!(fee > 0, "The treasury did not receive the fee");

			assert_balance!(ALICE.into(), HDX, 5268103860359261);
			assert_reserved_balance!(&ALICE.into(), DOT, dca_budget - amount_to_sell - fee);
		});
	}
}

#[test]
fn terminate_should_work_for_freshly_created_dca() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let block_id = 11;
		set_relaychain_block_number(block_id);

		let budget = 1000 * UNITS;
		let schedule1 = schedule_fake_with_buy_order(PoolType::Omnipool, HDX, DAI, 100 * UNITS, budget);

		assert_ok!(DCA::schedule(
			RuntimeOrigin::signed(ALICE.into()),
			schedule1.clone(),
			None
		));

		let schedule_id = 0;
		let schedule = DCA::schedules(schedule_id);
		assert!(schedule.is_some());

		//Act
		assert_ok!(DCA::terminate(RuntimeOrigin::signed(ALICE.into()), schedule_id, None));

		//Assert
		let schedule = DCA::schedules(schedule_id);
		assert!(schedule.is_none());
	});
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

fn create_schedule(owner: [u8; 32], schedule1: Schedule<AccountId, AssetId, u32>) {
	assert_ok!(DCA::schedule(RuntimeOrigin::signed(owner.into()), schedule1, None));
}

fn schedule_fake_with_buy_order(
	pool: PoolType<AssetId>,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
	budget: Balance,
) -> Schedule<AccountId, AssetId, u32> {
	schedule_fake_with_buy_order_with_route(
		asset_in,
		asset_out,
		amount,
		budget,
		vec![Trade {
			pool,
			asset_in,
			asset_out,
		}],
	)
}

fn schedule_fake_with_buy_order_with_route(
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
	budget: Balance,
	route: Vec<Trade<AssetId>>,
) -> Schedule<AccountId, AssetId, u32> {
	Schedule {
		owner: AccountId::from(ALICE),
		period: 5u32,
		total_amount: budget,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(10)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec(route),
		},
	}
}

fn schedule_fake_with_sell_order(
	owner: [u8; 32],
	pool: PoolType<AssetId>,
	total_amount: Balance,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
) -> Schedule<AccountId, AssetId, u32> {
	schedule_fake_with_sell_order_with_route(
		owner,
		total_amount,
		asset_in,
		asset_out,
		amount,
		vec![Trade {
			pool,
			asset_in,
			asset_out,
		}],
	)
}

fn schedule_fake_with_sell_order_with_route(
	owner: [u8; 32],
	total_amount: Balance,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
	route: Vec<Trade<AssetId>>,
) -> Schedule<AccountId, AssetId, u32> {
	Schedule {
		owner: AccountId::from(owner),
		period: 5u32,
		total_amount,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(15)),
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_amount_out: Balance::MIN,
			route: create_bounded_vec(route),
		},
	}
}

fn set_alice_lrna_balance(alice_init_lrna_balance: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		ALICE.into(),
		LRNA,
		alice_init_lrna_balance,
		0
	));
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_float(0.5);
	let stable_price = FixedU128::from_float(0.7);
	let acc = Omnipool::protocol_account();

	let stable_amount: Balance = 5_000_000_000_000_000_000_000u128;
	let native_amount: Balance = 5_000_000_000_000_000_000_000u128;
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone(),
		DAI,
		stable_amount,
		0
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		acc,
		HDX,
		native_amount as i128,
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(60),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(60),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	assert_ok!(Balances::force_set_balance(
		RawOrigin::Root.into(),
		Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
	));

	set_zero_reward_for_referrals(HDX);
	set_zero_reward_for_referrals(DAI);
}

pub fn init_omnipool_with_oracle_for_block_10() {
	init_omnipol();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	set_relaychain_block_number(10);
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}

pub fn run_to_block(from: BlockNumber, to: BlockNumber) {
	for b in from..=to {
		do_trade_to_populate_oracle(DAI, HDX, UNITS);
		set_relaychain_block_number(b);
	}
}

pub fn check_if_no_failed_events() {
	let failed_events = count_failed_trade_events();
	assert_eq!(0, failed_events);
}

pub fn check_if_dcas_completed_without_failed_or_terminated_events() {
	let failed_events = count_failed_trade_events();
	let terminated_events = count_terminated_trade_events();
	let completed_events = count_completed_event();
	assert_eq!(
		0, failed_events,
		"There has been some dca::TradeFailed events, but not expected"
	);
	assert_eq!(
		0, terminated_events,
		"There has been some dca::Terminated events, but not expected"
	);
	assert!(completed_events > 0, "There has been no dca::Completed events");
}

pub fn get_last_schedule_ids_from_trade_executed_events() -> Vec<u32> {
	let last_events: Vec<RuntimeEvent> = last_hydra_events(1000);
	let mut schedule_ids = vec![];

	for event in last_events {
		let e = event.clone();
		if let RuntimeEvent::DCA(pallet_dca::Event::TradeExecuted { id, .. }) = e {
			schedule_ids.push(id);
		}
	}

	schedule_ids
}

pub fn count_failed_trade_events() -> u32 {
	count_dca_event!(pallet_dca::Event::TradeFailed { .. })
}

pub fn count_terminated_trade_events() -> u32 {
	count_dca_event!(pallet_dca::Event::Terminated { .. })
}

pub fn count_completed_event() -> u32 {
	count_dca_event!(pallet_dca::Event::Completed { .. })
}
#[macro_export]
macro_rules! count_dca_event {
	($pattern:pat) => {{
		let last_events: Vec<RuntimeEvent> = last_hydra_events(100000);

		let mut counter: u32 = 0;
		for event in last_events {
			let e = event.clone();
			if matches!(e, RuntimeEvent::DCA($pattern)) {
				counter += 1;
			}
		}

		counter
	}};
}

pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.try_into().unwrap()),
			AssetKind::Token,
			1u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(18u8),
			None,
			None,
		)?;

		asset_ids.push(asset_id);
		Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
	}
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(RuntimeOrigin::root(), pool_id, asset_ids, amplification, fee)?;

	Stableswap::add_liquidity(
		RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(initial),
	)?;

	Ok((pool_id, asset_in, asset_out))
}

pub fn init_stableswap_with_three_assets_having_different_decimals(
) -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<Runtime as pallet_stableswap::Config>::AssetId>> = vec![];

	let mut asset_ids: Vec<<Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	let decimals_for_each_asset = [12u8, 6u8, 6u8];
	for idx in 0u32..3 {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();

		let asset_id = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.try_into().unwrap()),
			AssetKind::Token,
			1u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(decimals_for_each_asset[idx as usize]),
			None,
			None,
		)?;

		asset_ids.push(asset_id);
		Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			1_000_000_000_000_000i128,
		)?;
		Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(CHARLIE),
			asset_id,
			1_000_000_000_000_000_000_000i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = asset_ids[1];
	let asset_out: AssetId = asset_ids[2];

	Stableswap::create_pool(RuntimeOrigin::root(), pool_id, asset_ids, amplification, fee)?;

	Stableswap::add_liquidity(
		RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(initial),
	)?;

	Ok((pool_id, asset_in, asset_out))
}

pub fn add_dot_as_payment_currency() {
	add_dot_as_payment_currency_with_details(1000 * UNITS, FixedU128::from_rational(10, 100));
}

fn add_dot_as_payment_currency_with_details(amount: Balance, price: FixedU128) {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		Omnipool::protocol_account(),
		DOT,
		amount as i128,
	));

	assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		FixedU128::from_rational(1, 100000),
	));

	assert_ok!(Omnipool::add_token(
		RuntimeOrigin::root(),
		DOT,
		price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	//crate::dca::do_trade_to_populate_oracle(DOT, HDX, UNITS);
}
