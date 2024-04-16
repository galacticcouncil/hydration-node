// This file is part of HydraDX-node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// we don't need to run tests with benchmarking feature
#![cfg(not(feature = "runtime-benchmarks"))]
#![allow(clippy::bool_assert_comparison)]
use super::*;
pub use crate::mock::*;
use frame_support::{assert_ok, assert_storage_noop};

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub fn calculate_otc_price(otc: &pallet_otc::Order<AccountId, AssetId>) -> FixedU128 {
	FixedU128::checked_from_rational(otc.amount_out, otc.amount_in).unwrap()
}

#[test]
fn offchain_worker_should_store_last_update_in_storage() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		frame_system::Pallet::<Test>::set_block_number(10);

		let last_update_storage = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA_LAST_UPDATE);
		let last_update = last_update_storage
			.get::<BlockNumberFor<Test>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(last_update == 0); // not stored, default value

		frame_system::Pallet::<Test>::set_block_number(11);

		let block_num = frame_system::Pallet::<Test>::block_number();

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(block_num);

		let last_update = last_update_storage
			.get::<BlockNumberFor<Test>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(last_update == block_num);

		frame_system::Pallet::<Test>::set_block_number(12);

		let block_num = frame_system::Pallet::<Test>::block_number();

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(block_num);

		let last_update = last_update_storage
			.get::<BlockNumberFor<Test>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(last_update == block_num);

		// skip some blocks
		frame_system::Pallet::<Test>::set_block_number(15);

		let block_num = frame_system::Pallet::<Test>::block_number();

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(block_num);

		let last_update = last_update_storage
			.get::<BlockNumberFor<Test>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(last_update == block_num);

		// older block should not update the storage
		frame_system::Pallet::<Test>::set_block_number(14);

		let last_updated_block = 15;

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(block_num);

		let last_update = last_update_storage
			.get::<BlockNumberFor<Test>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(last_update == last_updated_block);
	});
}

#[test]
fn otcs_list_storage_should_be_sorted_on_new_block() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		place_orders();

		let sorted_list_of_otcs_storage = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);
		let sorted_list_of_otcs = sorted_list_of_otcs_storage
			.get::<Vec<SortedOtcsStorageType>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(sorted_list_of_otcs.is_empty()); // not stored, default value

		frame_system::Pallet::<Test>::set_block_number(15);
		let block_num = frame_system::Pallet::<Test>::block_number();

		OtcSettlements::sort_otcs(block_num);

		let sorted_list_of_otcs = sorted_list_of_otcs_storage
			.get::<Vec<SortedOtcsStorageType>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(!sorted_list_of_otcs.is_empty());

		// the list should be sorted
		let mut copy_of_list = sorted_list_of_otcs.clone();
		copy_of_list.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
		assert!(sorted_list_of_otcs == copy_of_list);

		// place new order and verify that the list is not updated again in the same block
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX,
			DAI,
			1_000_000_000,
			2_000_000_000,
			true,
		));

		OtcSettlements::sort_otcs(block_num);

		let latest_sorted_list_of_otcs = sorted_list_of_otcs_storage
			.get::<Vec<SortedOtcsStorageType>>()
			.unwrap_or_default()
			.unwrap_or_default();

		assert!(sorted_list_of_otcs == latest_sorted_list_of_otcs);
	});
}

#[test]
fn profit_should_be_transferred_to_treasury() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX,
			DAI,
			100_000 * ONE,
			200_001 * ONE,
			true,
		));

		let balance_before = Currencies::free_balance(HDX, &TreasuryAccount::get());

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		let balance_after = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert!(balance_after > balance_before);
	});
}

#[test]
fn existing_arb_opportunity_should_trigger_trade() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			200_001 * ONE,
			true,
		));

		// get otc price
		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dai_total_issuance = Currencies::total_issuance(DAI);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dai_total_issuance, Currencies::total_issuance(DAI));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		expect_events(vec![Event::Executed {
			otc_id,
			otc_asset_in: HDX,
			otc_asset_out: DAI,
			otc_amount_in: 762_939_453_125,
			otc_amount_out: 1_525_886_535_644,
			trade_amount_in: 1_525_886_535_644,
			trade_amount_out: 762_941_521_577,
		}
		.into()]);
	});
}

#[test]
fn multiple_arb_opportunities_should_trigger_trades() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			200_001 * ONE,
			true,
		));
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DOT, // otc asset_in
			KSM, // otc asset_out
			100_000 * ONE,
			100_001 * ONE,
			true,
		));

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		expect_events(vec![
			Event::Executed {
				otc_id: 0,
				otc_asset_in: HDX,
				otc_asset_out: DAI,
				otc_amount_in: 762_939_453_125,
				otc_amount_out: 1_525_886_535_644,
				trade_amount_in: 1_525_886_535_644,
				trade_amount_out: 762_941_521_577,
			}
			.into(),
			Event::Executed {
				otc_id: 1,
				otc_asset_in: DOT,
				otc_asset_out: KSM,
				otc_amount_in: 2288818359375,
				otc_amount_out: 2288841247558,
				trade_amount_in: 2288841247558,
				trade_amount_out: 2288830796079,
			}
			.into(),
		]);
	});
}

#[test]
fn trade_should_be_triggered_when_arb_opportunity_appears() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_001 * ONE,
			200_000 * ONE,
			true,
		));

		// get otc price
		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price(&route).unwrap();

		// verify that there's no arb opportunity yet
		assert!(otc_price < router_price);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		// make a trade to move the price and create an arb opportunity
		assert_ok!(Omnipool::sell(RuntimeOrigin::signed(ALICE), HDX, DAI, 10 * ONE, ONE,));

		System::set_block_number(System::block_number() + 1);

		// get otc price
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dai_total_issuance = Currencies::total_issuance(DAI);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dai_total_issuance, Currencies::total_issuance(DAI));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		expect_events(vec![Event::Executed {
			otc_id,
			otc_asset_in: HDX,
			otc_asset_out: DAI,
			otc_amount_in: 8_392_417_907_714,
			otc_amount_out: 16_784_667_968_748,
			trade_amount_in: 16_784_667_968_748,
			trade_amount_out: 8_392_626_224_459,
		}
		.into()]);
	});
}

#[test]
fn trade_should_not_be_triggered_when_there_is_no_arb_opportunity() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_001 * ONE,
			200_000 * ONE,
			true,
		));

		// get otc price
		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price(&route).unwrap();

		// verify that there's no arb opportunity
		assert!(otc_price < router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dai_total_issuance = Currencies::total_issuance(DAI);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		assert_storage_noop!(<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(
			System::block_number()
		));

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dai_total_issuance, Currencies::total_issuance(DAI));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);
	});
}

#[test]
fn trade_should_not_be_triggered_when_optimal_amount_not_found() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			1_000_000 * ONE,
			8_000_000_001 * ONE,
			true,
		));

		// get otc price
		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dai_total_issuance = Currencies::total_issuance(DAI);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		assert_storage_noop!(<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(
			System::block_number()
		));

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dai_total_issuance, Currencies::total_issuance(DAI));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);
	});
}

#[test]
fn test_offchain_worker_unsigned_transaction_submission() {
	let (mut ext, pool_state) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			200_001 * ONE,
			true,
		));

		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		// check that a transaction has been added to the pool
		let tx = pool_state.write().transactions.pop().unwrap();
		assert!(pool_state.read().transactions.is_empty());
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature, None); // unsigned
		assert_eq!(
			tx.call,
			crate::mock::RuntimeCall::OtcSettlements(crate::Call::settle_otc_order {
				otc_id: 0,
				amount: 762_939_453_125,
				route,
			})
		);
	})
}

fn place_orders() {
	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		DAI,
		1_000_000_000,
		2_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		LRNA,
		1_000_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		LRNA,
		HDX,
		4_000_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		LRNA,
		7_000_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		KSM,
		DAI,
		2_000_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		DAI,
		2_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		DAI,
		2_000_000_000,
		3_000_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		DAI,
		DOT,
		2_000_000_000,
		3_000_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		HDX,
		DOT,
		9_000_000_000,
		3_000_000_000,
		true,
	));

	assert_ok!(OTC::place_order(
		RuntimeOrigin::signed(ALICE),
		DAI,
		DOT,
		2_000_000_000,
		13_000_000_000,
		true,
	));
}
