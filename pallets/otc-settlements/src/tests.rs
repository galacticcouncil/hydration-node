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
use hydradx_traits::Inspect;
use orml_traits::MultiCurrency;
use pallet_support::types::Asset;
use pallet_support::types::Fee;
use pallet_support::types::Recipient;
pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub fn expect_last_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
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
		// create tuples of adjacent elements
		let result = sorted_list_of_otcs
			.iter()
			.zip(sorted_list_of_otcs.iter().skip(1))
			.collect::<Vec<_>>();

		// for all adjacent tuples (a,b) should be true that price_diff_a >= price_diff_b
		for (otc_id_a, otc_id_b) in result.iter() {
			// calculate price diff for the first otc id
			let otc = <pallet_otc::Orders<Test>>::get(otc_id_a).unwrap();
			let otc_price = FixedU128::checked_from_rational(otc.amount_out, otc.amount_in).unwrap();

			let route = Router::get_route(AssetPair {
				asset_in: otc.asset_out,
				asset_out: otc.asset_in,
			});
			let router_price = Router::spot_price_with_fee(&route.clone()).unwrap();
			let price_diff_a = otc_price.saturating_sub(router_price);

			// calculate price diff for the second otc id
			let otc = <pallet_otc::Orders<Test>>::get(otc_id_b).unwrap();
			let otc_price = FixedU128::checked_from_rational(otc.amount_out, otc.amount_in).unwrap();

			let route = Router::get_route(AssetPair {
				asset_in: otc.asset_out,
				asset_out: otc.asset_in,
			});
			let router_price = Router::spot_price_with_fee(&route.clone()).unwrap();
			let price_diff_b = otc_price.saturating_sub(router_price);
			assert!(price_diff_a >= price_diff_b);
		}

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
fn profit_should_be_transferred_to_treasury_when_zero_initial_pallet_balance() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX,
			DAI,
			100_000 * ONE,
			205_000 * ONE,
			true,
		));

		let pallet_acc = OtcSettlements::account_id();

		assert!(Currencies::free_balance(HDX, &pallet_acc) == 0);
		assert!(Currencies::free_balance(DAI, &pallet_acc) == 0);

		let balance_before = Currencies::free_balance(HDX, &TreasuryAccount::get());

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		assert!(Currencies::free_balance(HDX, &pallet_acc) == 0);
		assert!(Currencies::free_balance(DAI, &pallet_acc) == 0);

		let balance_after = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert!(balance_after > balance_before);
	});
}

#[test]
fn profit_should_be_transferred_to_treasury_when_nonzero_initial_pallet_balance() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX,
			DAI,
			100_000 * ONE,
			205_000 * ONE,
			true,
		));

		let pallet_acc = OtcSettlements::account_id();
		let initial_amount = 1_000 * ONE;
		<Test as Config>::Currency::mint_into(HDX, &pallet_acc, initial_amount).unwrap();

		assert!(Currencies::free_balance(HDX, &pallet_acc) == initial_amount);
		assert!(Currencies::free_balance(DAI, &pallet_acc) == 0);

		let balance_before = Currencies::free_balance(HDX, &TreasuryAccount::get());

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		assert!(Currencies::free_balance(HDX, &pallet_acc) == initial_amount);
		assert!(Currencies::free_balance(DAI, &pallet_acc) == 0);

		let balance_after = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert!(balance_after > balance_before);
	});
}

#[test]
fn existing_arb_opportunity_should_trigger_trade_when_correct_amount_can_be_found() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			205_000 * ONE,
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

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

		expect_events(vec![
			Event::Executed {
				asset_id: HDX,
				profit: 17_736_110_470_326,
			}
			.into(),
			pallet_support::Event::Swapped {
				swapper: otc.owner,
				filler: OtcSettlements::account_id(),
				filler_type: pallet_support::types::Filler::OTC(otc_id),
				operation: pallet_support::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(HDX, 2413749694825193)],
				outputs: vec![Asset::new(DAI, 4948186874391645)],
				fees: vec![Fee::new(
					DAI,
					49481868743917,
					Recipient::Account(<Test as pallet_otc::Config>::FeeReceiver::get()),
				)],
				operation_stack: vec![],
			}
			.into(),
		]);
	});
}

#[test]
fn existing_arb_opportunity_should_trigger_trade_when_partially_fillable_otc_can_be_fully_filled() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100 * ONE,
			205 * ONE,
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
		let initial_router_price = Router::spot_price_with_fee(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > initial_router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let dai_total_issuance = Currencies::total_issuance(DAI);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		// verify that the arb is still there, but smaller than before
		let final_router_price = Router::spot_price_with_fee(&route).unwrap();
		assert!(otc_price > final_router_price);
		assert!(final_router_price > initial_router_price);

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(dai_total_issuance, Currencies::total_issuance(DAI));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		expect_last_events(vec![Event::Executed {
			asset_id: HDX,
			profit: 1_444_117_874_415,
		}
		.into()]);
	});
}

#[test]
fn existing_arb_opportunity_should_trigger_trade_when_otc_is_not_partially_fillable() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			300_000 * ONE,
			false, // not partially fillable
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

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

		expect_last_events(vec![Event::Executed {
			asset_id: HDX,
			profit: 2_732_618_471_117_260,
		}
		.into()]);
	});
}

#[test]
fn existing_arb_opportunity_of_insufficient_asset_should_trigger_trade() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		// ensure that BTC is configured as insufficient asset
		assert!(!AssetRegistry::is_sufficient(BTC));

		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			BTC, // otc asset_in
			HDX, // otc asset_out
			200_000 * ONE,
			105_000 * ONE,
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > router_price);

		let hdx_total_issuance = Currencies::total_issuance(HDX);
		let btc_total_issuance = Currencies::total_issuance(BTC);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(BTC, &OtcSettlements::account_id()) == 0);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		assert_eq!(hdx_total_issuance, Currencies::total_issuance(HDX));
		assert_eq!(btc_total_issuance, Currencies::total_issuance(BTC));

		// total issuance of tokens should not change
		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(BTC, &OtcSettlements::account_id()) == 0);

		expect_last_events(vec![Event::Executed {
			asset_id: BTC,
			profit: 245_338_363_920_576,
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
			205_000 * ONE,
			true,
		));
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DOT, // otc asset_in
			KSM, // otc asset_out
			100_000 * ONE,
			102_000 * ONE,
			true,
		));

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		expect_events(vec![
			Event::Executed {
				asset_id: HDX,
				profit: 17736110470326,
			}
			.into(),
			Event::Executed {
				asset_id: DOT,
				profit: 11860096179879,
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

		// verify that there's no arb opportunity yet
		assert!(otc_price < router_price);

		<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(System::block_number());

		// make a trade to move the price and create an arb opportunity
		assert_ok!(Omnipool::sell(RuntimeOrigin::signed(ALICE), HDX, DAI, 3_000 * ONE, ONE,));

		System::set_block_number(System::block_number() + 1);

		// get otc price
		let otc_price = calculate_otc_price(&otc);

		// get trade price
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});
		let router_price = Router::spot_price_with_fee(&route).unwrap();

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

		expect_last_events(vec![Event::Executed {
			asset_id: HDX,
			profit: 5_173_145_606_735,
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

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
fn trade_should_be_triggered_when_optimal_amount_not_found_but_arb_can_be_reduced() {
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

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
	});
}

#[test]
fn trade_should_not_be_triggered_when_amount_not_found() {
	let (mut ext, _) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			1_000 * ONE,
			800_000_000_000_000_001 * ONE,
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
		let router_price = Router::spot_price_with_fee(&route).unwrap();

		// verify that there's an arb opportunity
		assert!(otc_price > router_price);

		assert!(Currencies::free_balance(HDX, &OtcSettlements::account_id()) == 0);
		assert!(Currencies::free_balance(DAI, &OtcSettlements::account_id()) == 0);

		assert_storage_noop!(<OtcSettlements as Hooks<BlockNumberFor<Test>>>::offchain_worker(
			System::block_number()
		));
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
			205_000 * ONE,
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
				amount: 2_413_749_694_825_193,
				route,
			})
		);
	})
}

#[test]
fn test_offchain_worker_signed_transaction_submission() {
	let (mut ext, _pool_state) = ExtBuilder::default().build();
	ext.execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			HDX, // otc asset_in
			DAI, // otc asset_out
			100_000 * ONE,
			205_000 * ONE,
			true,
		));

		let otc_id = 0;
		let otc = <pallet_otc::Orders<Test>>::get(otc_id).unwrap();
		let route = Router::get_route(AssetPair {
			asset_in: otc.asset_out,
			asset_out: otc.asset_in,
		});

		assert_ok!(OtcSettlements::settle_otc_order(
			RuntimeOrigin::signed(ALICE),
			otc_id,
			2_413_749_694_825_193,
			route,
		));
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
