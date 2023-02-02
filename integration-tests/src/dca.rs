#![cfg(test)]

use crate::polkadot_test_net::*;
use std::mem::size_of;

use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};

use hydradx_runtime::Origin;
use orml_traits::MultiCurrency;
use pallet_dca::types::{Bond, Order, Recurrence, Schedule, ScheduleId, Trade};
use polkadot_primitives::v2::BlockNumber;
use primitives::{AssetId, Balance};
use sp_core::MaxEncodedLen;
use sp_runtime::traits::ConstU32;
use sp_runtime::Permill;
use sp_runtime::{BoundedVec, FixedU128};
use xcm_emulator::TestExt;
#[test]
fn crate_schedule_should_work() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let schedule1 = schedule_fake_with_buy_order(DAI, HDX, UNITS);

		//Act
		assert_ok!(hydradx_runtime::DCA::schedule(
			hydradx_runtime::Origin::signed(ALICE.into()),
			schedule1,
			None
		));

		//Assert
		let schedule = hydradx_runtime::DCA::schedules(1);
		assert!(schedule.is_some());

		let next_block_id = 2;
		let schedule = hydradx_runtime::DCA::schedule_ids_per_block(next_block_id);
		assert!(schedule.is_some());
	});
}

#[test]
fn schedule_execution_should_work_when_block_is_initialized() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipol();

		let schedule1 = schedule_fake_with_buy_order(HDX, DAI, UNITS);
		create_schedule(schedule1);

		let user_dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &ALICE.into());
		assert_eq!(user_dai_balance, ALICE_INITIAL_DAI_BALANCE);

		//Act
		hydra_run_to_block(5);

		//Assert
		let user_dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &ALICE.into());
		assert_eq!(user_dai_balance, ALICE_INITIAL_DAI_BALANCE + UNITS);
	});
}

#[test]
fn schedules_should_be_ordered_based_on_random_number_when_executed_in_a_block() {
	//We simulate a failing scenarios so we get errros we can use for verification
	//The user don't have enough balance
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		init_omnipol();

		let schedule1 = schedule_fake_with_buy_order(DAI, HDX, UNITS);
		let schedule2 = schedule_fake_with_buy_order(DAI, HDX, UNITS);
		let schedule3 = schedule_fake_with_buy_order(DAI, HDX, UNITS);
		let schedule4 = schedule_fake_with_buy_order(DAI, HDX, UNITS);
		let schedule5 = schedule_fake_with_buy_order(DAI, HDX, UNITS);
		let schedule6 = schedule_fake_with_buy_order(DAI, HDX, UNITS);

		let user_dai_balance = hydradx_runtime::Tokens::free_balance(DAI, &ALICE.into());
		assert_eq!(user_dai_balance, ALICE_INITIAL_DAI_BALANCE);

		create_schedule_by_charlie(schedule1);
		create_schedule_by_charlie(schedule2);
		create_schedule_by_charlie(schedule3);
		create_schedule_by_charlie(schedule4);
		create_schedule_by_charlie(schedule5);
		create_schedule_by_charlie(schedule6);

		//Act
		hydra_run_to_block(5);

		//Assert
		//We check the reordering based on the the emitted events.
		//As the CHARLIE has no balance of DAI, all the schedule execution will fail and be suspended
		//As the hash is fixed for the relay block number for integration tests, therefore we should always expect the same result
		expect_suspended_events(vec![
			pallet_dca::Event::Suspended {
				id: 1,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 2,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 6,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 4,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 3,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 5,
				who: sp_runtime::AccountId32::from(CHARLIE),
			}
			.into(),
		]);
	});
}

#[test]
#[ignore] //This test is ignored as only used for estimating the storage bond size
fn calculate_storage_bond() {
	let schedule_key_size = size_of::<ScheduleId>();
	let schedule_value_size = Schedule::<ScheduleId, BlockNumber>::max_encoded_len();

	let schedule_ownership_key_size = size_of::<ScheduleId>();
	let schedule_ownership_value_size = size_of::<common_runtime::AccountId>();

	let suspended_key_size = size_of::<ScheduleId>();

	let remaining_reccurrencies_key_size = size_of::<ScheduleId>();
	let remaining_reccurrencies_value_size = size_of::<u32>();

	let schedule_ids_per_block_entry_size = size_of::<ScheduleId>();

	let bond_key_size = size_of::<ScheduleId>();
	let bond_value_size = Bond::<primitives::AssetId>::max_encoded_len();

	let storage_bond_size: usize = vec![
		schedule_key_size,
		schedule_value_size,
		schedule_ownership_key_size,
		schedule_ownership_value_size,
		suspended_key_size,
		remaining_reccurrencies_key_size,
		remaining_reccurrencies_value_size,
		schedule_ids_per_block_entry_size,
		bond_key_size,
		bond_value_size,
	]
	.iter()
	.sum();

	let storage_bond = primitives::constants::currency::bytes_to_balance(storage_bond_size as u32);
}

fn create_schedule(schedule1: Schedule<AssetId, u32>) {
	assert_ok!(hydradx_runtime::DCA::schedule(
		hydradx_runtime::Origin::signed(ALICE.into()),
		schedule1,
		None
	));
}

fn create_schedule_by_charlie(schedule1: Schedule<AssetId, u32>) {
	assert_ok!(hydradx_runtime::DCA::schedule(
		hydradx_runtime::Origin::signed(CHARLIE.into()),
		schedule1,
		None
	));
}

fn schedule_fake_with_buy_order(asset_in: AssetId, asset_out: AssetId, amount: Balance) -> Schedule<AssetId, u32> {
	let schedule1 = Schedule {
		period: 3u32,
		recurrence: Recurrence::Perpetual,
		order: Order::Buy {
			asset_in: asset_in,
			asset_out: asset_out,
			amount_out: amount,
			max_limit: Balance::MAX,
			route: create_bounded_vec(vec![]),
		},
	};
	schedule1
}

pub fn create_bounded_vec(trades: Vec<Trade<AssetId>>) -> BoundedVec<Trade<AssetId>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<AssetId>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(801500000000000);
	hydradx_runtime::Omnipool::protocol_account();

	assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
		hydradx_runtime::Origin::root(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(10)
	));
}

pub fn hydra_run_to_block(to: BlockNumber) {
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);
		hydradx_runtime::DCA::on_initialize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

pub fn expect_suspended_events(e: Vec<hydradx_runtime::Event>) {
	let last_events: Vec<hydradx_runtime::Event> = get_last_suspended_events();
	pretty_assertions::assert_eq!(last_events, e);
}

pub fn get_last_suspended_events() -> Vec<hydradx_runtime::Event> {
	let last_events: Vec<hydradx_runtime::Event> = last_hydra_events(1000);
	let mut suspended_events = vec![];

	for event in last_events {
		let e = event.clone();
		if let hydradx_runtime::Event::DCA(pallet_dca::Event::Suspended { .. }) = e {
			suspended_events.push(event.clone());
		}
	}

	suspended_events
}
