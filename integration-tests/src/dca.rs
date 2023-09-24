#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;

use crate::{assert_balance, assert_reserved_balance};
use frame_system::RawOrigin;
use hydradx_runtime::{Balances, Currencies, Omnipool, Router, RuntimeEvent, RuntimeOrigin, Tokens, DCA};
use hydradx_traits::router::PoolType;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_dca::types::{Order, Schedule};
use pallet_route_executor::Trade;
use polkadot_primitives::v2::BlockNumber;
use primitives::{AssetId, Balance};
use sp_runtime::traits::ConstU32;
use sp_runtime::Permill;
use sp_runtime::{BoundedVec, FixedU128};
use xcm_emulator::TestExt;
const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;

#[test]
fn create_schedule_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let block_id = 11;
		set_relaychain_block_number(block_id);

		let budget = 1000 * UNITS;
		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, 100 * UNITS, budget);

		//Act
		assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE.into()), schedule1, None));

		//Assert
		let schedule_id = 0;
		let schedule = DCA::schedules(schedule_id);
		assert!(schedule.is_some());

		let next_block_id = block_id + 1;
		let schedule = DCA::schedule_ids_per_block(next_block_id);
		assert!(!schedule.is_empty());
		expect_hydra_events(vec![pallet_dca::Event::Scheduled {
			id: 0,
			who: ALICE.into(),
		}
		.into()]);
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
		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, amount_out, dca_budget);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
		assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
		let amount_in = 140421094431120;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_in - fee);

		let treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
		assert!(treasury_balance > TREASURY_ACCOUNT_INIT_BALANCE);
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
			period: 1u32,
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
		set_relaychain_block_number(11);
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - fee);
		assert_eq!(DCA::retries_on_error(schedule_id), 1);

		set_relaychain_block_number(21);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 2 * fee);
		assert_eq!(DCA::retries_on_error(schedule_id), 2);

		set_relaychain_block_number(41);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 3 * fee);
		assert_eq!(DCA::retries_on_error(schedule_id), 3);

		//After this retry we terminate
		set_relaychain_block_number(81);
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
		let schedule1 = schedule_fake_with_buy_order(LRNA, DAI, amount_out, dca_budget);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
		assert_balance!(&Treasury::account_id(), LRNA, 0);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(LRNA, &Treasury::account_id());
		let amount_in = 70175440083618;
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget - amount_in - fee);

		let treasury_balance = Currencies::free_balance(LRNA, &Treasury::account_id());
		assert!(treasury_balance > 0);
	});
}

#[test]
fn buy_schedule_and_direct_buy_and_router_should_yield_same_result_when_selling_native_asset() {
	let amount_in = 140_421_094_431_120;
	let amount_out = 100 * UNITS;

	//DCA
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let dca_budget = 1000 * UNITS;

		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, amount_out, dca_budget);
		create_schedule(ALICE, schedule1);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_in - fee);

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
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_in);
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
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_in);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
	});
}

#[test]
fn buy_schedule_and_direct_buy_and_router_should_yield_same_result_when_asset_in_is_hub_asset() {
	let amount_in = 70_175_440_083_618;
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

		let schedule1 = schedule_fake_with_buy_order(LRNA, DAI, amount_out, dca_budget);
		create_schedule(ALICE, schedule1);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(LRNA, &Treasury::account_id());
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget - amount_in - fee);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
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
		assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - amount_in);
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
		assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - amount_in);
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
		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, 100 * UNITS, dca_budget);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

		//Act
		run_to_block(11, 40);

		//Assert
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + 600 * UNITS);

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
fn sell_schedule_execution_should_work_when_block_is_initialized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let dca_budget = 1100 * UNITS;
		let amount_to_sell = 100 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
		assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let amount_out = 71_214_372_591_631;
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);

		//Assert that fee is sent to treasury
		let treasury_balance = Currencies::free_balance(HDX, &Treasury::account_id());
		assert!(treasury_balance > TREASURY_ACCOUNT_INIT_BALANCE);
	});
}

#[test]
fn sell_schedule_should_sell_remaining_in_next_trade_when_there_is_not_enough_left() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let dca_budget = 1000 * UNITS;
		let amount_to_sell = 700 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
		assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

		//Act
		run_to_block(11, 15);

		//Assert
		let schedule_id = 0;
		let schedule = DCA::schedules(schedule_id);
		assert!(schedule.is_none());

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, 0);
	});
}

#[test]
fn sell_schedule_should_be_terminated_after_retries() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let dca_budget = 1100 * UNITS;
		let amount_to_sell = 100 * UNITS;
		let schedule1 = Schedule {
			owner: AccountId::from(ALICE),
			period: 1u32,
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

		set_relaychain_block_number(11);
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - fee);

		assert_eq!(DCA::retries_on_error(schedule_id), 1);

		set_relaychain_block_number(21);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 2 * fee);
		assert_eq!(DCA::retries_on_error(schedule_id), 2);

		set_relaychain_block_number(41);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - 3 * fee);
		assert_eq!(DCA::retries_on_error(schedule_id), 3);

		//At this point, the schedule will be terminated as retries max number of times
		set_relaychain_block_number(81);
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
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, LRNA, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
		assert_balance!(&Treasury::account_id(), LRNA, 0);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let amount_out = 142499995765917;
		let fee = Currencies::free_balance(LRNA, &Treasury::account_id());
		let treasury_balance = Currencies::free_balance(LRNA, &Treasury::account_id());
		assert!(treasury_balance > 0);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), LRNA, alice_init_hub_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget - amount_to_sell - fee);
	});
}

#[test]
fn sell_schedule_and_direct_omnipool_sell_and_router_should_yield_same_result_when_native_asset_sold() {
	let amount_out = 71_214_372_591_631;
	let amount_to_sell = 100 * UNITS;

	//DCA
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let dca_budget = 1100 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(HDX, &Treasury::account_id()) - TREASURY_ACCOUNT_INIT_BALANCE;
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell - fee);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
	});

	//Direct Omnipool
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
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
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
	});

	//Router
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
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
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
	});
}

#[test]
fn sell_schedule_and_direct_omnipool_sell_and_router_should_yield_same_result_when_hub_asset_sold() {
	let amount_out = 142_499_995_765_917;
	let amount_to_sell = 100 * UNITS;

	//DCA
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_lrna_balance = 5000 * UNITS;
		set_alice_lrna_balance(alice_init_lrna_balance);

		let dca_budget = 1100 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, LRNA, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), LRNA, alice_init_lrna_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let fee = Currencies::free_balance(LRNA, &Treasury::account_id());
		assert_reserved_balance!(&ALICE.into(), LRNA, dca_budget - amount_to_sell - fee);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
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
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
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
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
	});
}

#[test]
fn full_sell_dca_should_be_executed_then_completed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		init_omnipool_with_oracle_for_block_10();

		let amount_to_sell = 100 * UNITS;
		let dca_budget = 1200 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
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
	});
}

#[test]
fn full_sell_dca_should_be_executed_then_completed_for_multiple_users() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let bob_init_hdx_balance = 5000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			BOB.into(),
			bob_init_hdx_balance,
			0,
		));

		init_omnipool_with_oracle_for_block_10();

		let amount_to_sell = 100 * UNITS;
		let dca_budget = 1000 * UNITS;
		let dca_budget_for_bob = 1200 * UNITS;

		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule2 = schedule_fake_with_sell_order(BOB, dca_budget_for_bob, HDX, DAI, amount_to_sell);
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
	});
}

#[test]
fn multiple_full_sell_dca_should_be_executed_then_completed_for_same_user() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let alice_init_hdx_balance = 50000 * UNITS;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		init_omnipool_with_oracle_for_block_10();

		//Trade 1
		let amount_to_sell1 = 100 * UNITS;
		let dca_budget1 = 1000 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget1, HDX, DAI, amount_to_sell1);
		create_schedule(ALICE, schedule1);

		//Trade 2
		let amount_to_sell2 = 125 * UNITS;
		let dca_budget2 = 1500 * UNITS;
		let schedule2 = schedule_fake_with_sell_order(ALICE, dca_budget2, HDX, DAI, amount_to_sell2);
		create_schedule(ALICE, schedule2);

		//Trade 3
		let amount_to_sell3 = 250 * UNITS;
		let dca_budget3 = 2000 * UNITS;
		let schedule3 = schedule_fake_with_sell_order(ALICE, dca_budget3, HDX, DAI, amount_to_sell3);
		create_schedule(ALICE, schedule3);

		let budget_for_all_trades = dca_budget1 + dca_budget2 + dca_budget3;
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - budget_for_all_trades);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, budget_for_all_trades);

		//Act
		run_to_block(11, 100);

		//Assert
		assert_reserved_balance!(&ALICE.into(), HDX, 0);
		assert_balance!(
			ALICE.into(),
			HDX,
			alice_init_hdx_balance - dca_budget1 - dca_budget2 - dca_budget3
		);

		let schedule = DCA::schedules(0);
		assert!(schedule.is_none());

		let schedule = DCA::schedules(1);
		assert!(schedule.is_none());

		let schedule = DCA::schedules(2);
		assert!(schedule.is_none());
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

		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule2 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule3 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule4 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule5 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		let schedule6 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);

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

		let alice_init_hdx_balance = 1000 * UNITS + fee + 1;
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			alice_init_hdx_balance,
			0,
		));

		let dca_budget = 1000 * UNITS + fee;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);
		assert_balance!(&Treasury::account_id(), HDX, TREASURY_ACCOUNT_INIT_BALANCE);

		//Act
		set_relaychain_block_number(11);

		//Assert
		check_if_no_failed_events();
		assert_balance!(ALICE.into(), HDX, 0);
		assert_reserved_balance!(&ALICE.into(), HDX, 0);
	});
}

fn create_schedule(owner: [u8; 32], schedule1: Schedule<AccountId, AssetId, u32>) {
	assert_ok!(DCA::schedule(RuntimeOrigin::signed(owner.into()), schedule1, None));
}

fn schedule_fake_with_buy_order(
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
	budget: Balance,
) -> Schedule<AccountId, AssetId, u32> {
	Schedule {
		owner: AccountId::from(ALICE),
		period: 2u32,
		total_amount: budget,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(5)),
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_amount_in: Balance::MAX,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
		},
	}
}

fn schedule_fake_with_sell_order(
	owner: [u8; 32],
	total_amount: Balance,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
) -> Schedule<AccountId, AssetId, u32> {
	Schedule {
		owner: AccountId::from(owner),
		period: 3u32,
		total_amount,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(Permill::from_percent(10)),
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_amount_out: Balance::MIN,
			route: create_bounded_vec(vec![Trade {
				pool: PoolType::Omnipool,
				asset_in,
				asset_out,
			}]),
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

	assert_ok!(Omnipool::set_tvl_cap(RuntimeOrigin::root(), u128::MAX));

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

	assert_ok!(Omnipool::initialize_pool(
		RuntimeOrigin::root(),
		stable_price,
		native_price,
		Permill::from_percent(60),
		Permill::from_percent(60)
	));

	assert_ok!(Balances::set_balance(
		RawOrigin::Root.into(),
		Treasury::account_id(),
		TREASURY_ACCOUNT_INIT_BALANCE,
		0,
	));
}

fn init_omnipool_with_oracle_for_block_10() {
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
		do_trade_to_populate_oracle(DAI, HDX, UNITS);
	}
}

pub fn check_if_no_failed_events() {
	let failed_events = count_failed_trade_events();
	assert_eq!(0, failed_events);
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
	let last_events: Vec<RuntimeEvent> = last_hydra_events(100000);

	let mut counter: u32 = 0;
	for event in last_events {
		let e = event.clone();
		if matches!(e, RuntimeEvent::DCA(pallet_dca::Event::TradeFailed { .. })) {
			counter += 1;
		}
	}

	counter
}
