#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};

use pallet_dca::types::{Order, Recurrence, Schedule, Trade};
use polkadot_primitives::v2::BlockNumber;
use primitives::{AssetId, Balance};
use sp_runtime::traits::ConstU32;
use sp_runtime::Permill;
use sp_runtime::{BoundedVec, FixedU128};
use xcm_emulator::TestExt;

#[test]
fn schedules_should_be_ordered_based_on_random_number_when_executed_in_a_block() {
	TestNet::reset();
	Hydra::execute_with(|| {
		//Arrange
		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);
		hydradx_runtime::Omnipool::protocol_account();

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::Origin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));

		let schedule1 = schedule_fake();
		let schedule2 = schedule_fake();
		let schedule3 = schedule_fake();
		let schedule4 = schedule_fake();
		let schedule5 = schedule_fake();
		let schedule6 = schedule_fake();

		create_schedule(schedule1);
		create_schedule(schedule2);
		create_schedule(schedule3);
		create_schedule(schedule4);
		create_schedule(schedule5);
		create_schedule(schedule6);

		//Act
		hydra_run_to_block(5);

		//Assert
		//We check the reordering based on the the emitted events.
		//As the user has no balance of HDX, all the schedule execution will fail and be suspended
		//As the hash is fixed for the relay block number, therefore we should expect the same result
		expect_suspended_events(vec![
			pallet_dca::Event::Suspended {
				id: 1,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 2,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 6,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 4,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 3,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
			pallet_dca::Event::Suspended {
				id: 5,
				who: sp_runtime::AccountId32::from(ALICE),
			}
			.into(),
		]);
	});
}

fn create_schedule(schedule1: Schedule<AssetId, u32>) {
	assert_ok!(hydradx_runtime::DCA::schedule(
		hydradx_runtime::Origin::signed(ALICE.into()),
		schedule1,
		None
	));
}

fn schedule_fake() -> Schedule<AssetId, u32> {
	let schedule1 = Schedule {
		period: 3u32,
		recurrence: Recurrence::Perpetual,
		order: Order::Buy {
			asset_in: DAI,
			asset_out: DOT,
			amount_out: UNITS,
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
