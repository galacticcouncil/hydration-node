#![cfg(test)]

use crate::polkadot_test_net::*;
use std::mem::size_of;

use frame_support::assert_ok;

use crate::{assert_balance, assert_reserved_balance};
use frame_system::RawOrigin;
use hydradx_runtime::Balances;
use hydradx_runtime::Currencies;
use hydradx_runtime::Omnipool;
use hydradx_runtime::Origin;
use hydradx_runtime::Tokens;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_dca::types::{Order, Schedule, ScheduleId, Trade};
use polkadot_primitives::v2::BlockNumber;
use primitives::{AssetId, Balance};
use sp_core::MaxEncodedLen;
use sp_runtime::traits::ConstU32;
use sp_runtime::Permill;
use sp_runtime::{BoundedVec, FixedU128};
use xcm_emulator::TestExt;

const TREASURY_ACCOUNT_INIT_BALANCE: Balance = 1000 * UNITS;
const DCA_EXECUTION_FEE: Balance = 670_053_883_056;

#[test]
fn create_schedule_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let block_id = 11;
		set_relaychain_block_number(block_id);

		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, UNITS, 110 * UNITS);

		//Act
		assert_ok!(hydradx_runtime::DCA::schedule(
			hydradx_runtime::Origin::signed(ALICE.into()),
			schedule1,
			None
		));

		//Assert
		let schedule_id = 0;
		let schedule = hydradx_runtime::DCA::schedules(schedule_id);
		assert!(schedule.is_some());

		let next_block_id = block_id + 1;
		let schedule = hydradx_runtime::DCA::schedule_ids_per_block(next_block_id);
		assert!(!schedule.is_empty());
	});
}

#[test]
fn buy_schedule_execution_should_work_when_block_is_initialized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let dca_budget = 110 * UNITS;

		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, UNITS, dca_budget);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

		//Act
		set_relaychain_block_number(11);

		//Assert
		let over_reservation_left_over = 37789121938; //In case of buy we always unreserve more than needed for each transaction, so there will be some positive leftover for the user
		let amount_to_unreserve_for_trade = 2112053882397;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + UNITS);
		assert_balance!(
			ALICE.into(),
			HDX,
			ALICE_INITIAL_NATIVE_BALANCE - dca_budget + over_reservation_left_over
		);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_unreserve_for_trade);

		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE + DCA_EXECUTION_FEE
		);
	});
}

#[test]
fn sell_schedule_execution_should_work_when_block_is_initialized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::Origin::root(),
			ALICE.clone().into(),
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

		//Act
		set_relaychain_block_number(11);

		//Assert
		let amount_out = 70737197939033;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget - amount_to_sell);

		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE + DCA_EXECUTION_FEE
		);
	});
}

#[test]
fn full_buy_dca_should_be_executed_then_completed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let dca_budget = 110 * UNITS;
		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, UNITS, 110 * UNITS);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - dca_budget);
		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE
		);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

		//Act
		run_to_block(11, 150);

		//Assert
		let over_reservation_left_over = 2138232380497; //In case of buy we always unreserve more than needed for each transaction, so there will be some positive leftover for the user
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + 52 * UNITS);
		assert_balance!(
			ALICE.into(),
			HDX,
			ALICE_INITIAL_NATIVE_BALANCE - dca_budget + over_reservation_left_over
		);

		assert_reserved_balance!(&ALICE.into(), HDX, 0);

		let fees = 34842801918912;
		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE + fees
		);

		let schedule = hydradx_runtime::DCA::schedules(1);
		assert!(schedule.is_none());
	});
}

#[test]
fn full_sell_dca_should_be_executed_then_completed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::Origin::root(),
			ALICE.clone().into(),
			alice_init_hdx_balance,
			0,
		));

		init_omnipool_with_oracle_for_block_10();

		let dca_budget = 1100 * UNITS;

		let amount_to_sell = 100 * UNITS;
		let schedule1 = schedule_fake_with_sell_order(ALICE, dca_budget, HDX, DAI, amount_to_sell);
		create_schedule(ALICE, schedule1);

		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE);
		assert_reserved_balance!(&ALICE.into(), HDX, dca_budget);

		//Act
		run_to_block(11, 100);

		//Assert
		let amount_out = 778108918061801;
		let fee = 7_370_592_713_616;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, 0);

		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE + fee
		);

		let schedule = hydradx_runtime::DCA::schedules(1);
		assert!(schedule.is_none());
	});
}

#[test]
fn full_sell_dca_should_be_executed_then_completed_for_multiple_users() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let alice_init_hdx_balance = 5000 * UNITS;
		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::Origin::root(),
			ALICE.clone().into(),
			alice_init_hdx_balance,
			0,
		));

		let bob_init_hdx_balance = 5000 * UNITS;
		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::Origin::root(),
			BOB.clone().into(),
			bob_init_hdx_balance,
			0,
		));

		init_omnipool_with_oracle_for_block_10();

		let dca_budget = 1100 * UNITS;
		let dca_budget_for_bob = 1300 * UNITS;

		let amount_to_sell = 100 * UNITS;
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
		let amount_out = 778108653291461;

		assert_balance!(ALICE.into(), DAI, ALICE_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(ALICE.into(), HDX, alice_init_hdx_balance - dca_budget);
		assert_reserved_balance!(&ALICE.into(), HDX, 0);

		let amount_out = 919582781844482;

		assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);
		assert_balance!(BOB.into(), HDX, bob_init_hdx_balance - dca_budget_for_bob);
		assert_reserved_balance!(&BOB.into(), HDX, 0);

		let fee = 16081293193344;
		assert_balance!(
			&hydradx_runtime::Treasury::account_id(),
			HDX,
			TREASURY_ACCOUNT_INIT_BALANCE + fee
		);

		let schedule = hydradx_runtime::DCA::schedules(1);
		assert!(schedule.is_none());

		let schedule = hydradx_runtime::DCA::schedules(2);
		assert!(schedule.is_none());
	});
}

#[test]
fn schedules_should_be_ordered_based_on_random_number_when_executed_in_a_block() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_10();

		let schedule1 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);
		let schedule2 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);
		let schedule3 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);
		let schedule4 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);
		let schedule5 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);
		let schedule6 = schedule_fake_with_invalid_min_limit(ALICE, 110 * UNITS, HDX, DAI, UNITS);

		create_schedule(ALICE, schedule1);
		create_schedule(ALICE, schedule2);
		create_schedule(ALICE, schedule3);
		create_schedule(ALICE, schedule4);
		create_schedule(ALICE, schedule5);
		create_schedule(ALICE, schedule6);

		//Act
		run_to_block(11, 12);

		//Assert
		//We check the random ordering based on the the emitted events.
		//The orders should fail due to invalid min limit.
		expect_schedule_ids_from_events(vec![2, 5, 0, 4, 3, 1]);
	});
}
#[test]
#[ignore] //This test is ignored as only used for estimating the storage bond size
fn calculate_storage_bond() {
	let schedule_key_size = size_of::<ScheduleId>();
	let schedule_value_size = Schedule::<AccountId, ScheduleId, BlockNumber>::max_encoded_len();

	let schedule_ownership_key_size = size_of::<ScheduleId>();
	let schedule_ownership_value_size = size_of::<common_runtime::AccountId>();

	let suspended_key_size = size_of::<ScheduleId>();

	let remaining_reccurrencies_key_size = size_of::<ScheduleId>();
	let remaining_reccurrencies_value_size = size_of::<u32>();

	let schedule_ids_per_block_entry_size = size_of::<ScheduleId>();

	let storage_bond_size: usize = vec![
		schedule_key_size,
		schedule_value_size,
		schedule_ownership_key_size,
		schedule_ownership_value_size,
		suspended_key_size,
		remaining_reccurrencies_key_size,
		remaining_reccurrencies_value_size,
		schedule_ids_per_block_entry_size,
	]
	.iter()
	.sum();

	let _ = primitives::constants::currency::bytes_to_balance(storage_bond_size as u32);
}

fn create_schedule(owner: [u8; 32], schedule1: Schedule<AccountId, AssetId, u32>) {
	assert_ok!(hydradx_runtime::DCA::schedule(
		hydradx_runtime::Origin::signed(owner.into()),
		schedule1,
		None
	));
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
		order: Order::Buy {
			asset_in,
			asset_out,
			amount_out: amount,
			max_limit: 2 * UNITS,
			slippage: None,
			route: create_bounded_vec(vec![]),
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
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_limit: Balance::MIN,
			slippage: None,
			route: create_bounded_vec(vec![]),
		},
	}
}

fn schedule_fake_with_invalid_min_limit(
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
		order: Order::Sell {
			asset_in,
			asset_out,
			amount_in: amount,
			min_limit: Balance::MAX,
			slippage: None,
			route: create_bounded_vec(vec![]),
		},
	}
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_float(0.5);
	let stable_price = FixedU128::from_float(0.7);
	let acc = hydradx_runtime::Omnipool::protocol_account();

	assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(Origin::root(), u128::MAX));

	let stable_amount: Balance = 5_000_000_000_000_000_000_000u128;
	let native_amount: Balance = 5_000_000_000_000_000_000_000u128;
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		acc.clone().into(),
		DAI,
		stable_amount,
		0
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::Origin::root(),
		acc.clone().into(),
		HDX,
		native_amount as i128,
	));

	assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
		hydradx_runtime::Origin::root(),
		stable_price,
		native_price,
		Permill::from_percent(60),
		Permill::from_percent(60)
	));

	assert_ok!(Balances::set_balance(
		RawOrigin::Root.into(),
		hydradx_runtime::Treasury::account_id(),
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
		hydradx_runtime::Origin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		hydradx_runtime::Origin::signed(CHARLIE.into()),
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

pub fn expect_schedule_ids_from_events(e: Vec<u32>) {
	let last_schedule_ids_from_events: Vec<u32> = get_last_schedule_ids_from_trade_failed_events();
	pretty_assertions::assert_eq!(last_schedule_ids_from_events, e);
}

pub fn get_last_schedule_ids_from_trade_failed_events() -> Vec<u32> {
	let last_events: Vec<hydradx_runtime::Event> = last_hydra_events(1000);
	let mut schedule_ids = vec![];

	for event in last_events {
		let e = event.clone();
		if let hydradx_runtime::Event::DCA(pallet_dca::Event::TradeFailed { id, .. }) = e {
			schedule_ids.push(id);
		}
	}

	schedule_ids
}
