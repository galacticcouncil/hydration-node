use super::*;
use crate::types::Balance;
use crate::*;
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, SimpleImbalance, Tradability};
use pretty_assertions::assert_eq;
use test_case::test_case;
use test_utils::assert_balance;
const ALICE_INITIAL_LRNA_BALANCE: Balance = 500 * ONE;
const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_3_BALANCE: Balance = 3000 * ONE;
const OMNIPOOL_INITIAL_ASSET_4_BALANCE: Balance = 4000 * ONE;
const OMNIPOOL_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_6_BALANCE: Balance = 6000 * ONE;

#[test]
fn sell_should_work_when_both_asset_in_same_subpool() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				ASSET_3,
				ASSET_4,
				amount_to_sell,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 98446129714113;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, amount_to_get);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE - amount_to_get);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);

			assert_that_imbalance_is_zero!();
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::BUY)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn sell_should_fail_when_assets_in_is_not_sellable_in_one_pool(tradability: Tradability) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID_2)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, OMNIPOOL_INITIAL_ASSET_6_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);
			create_subpool!(SHARE_ASSET_AS_POOL_ID_2, ASSET_5, ASSET_6);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::signed(ALICE), ASSET_3, ASSET_5, amount_to_sell, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::SELL)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn sell_should_fail_when_assets_out_is_not_buyable_in_one_pool(tradability: Tradability) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID_2)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, OMNIPOOL_INITIAL_ASSET_6_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_5, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);
			create_subpool!(SHARE_ASSET_AS_POOL_ID_2, ASSET_5, ASSET_6);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::signed(ALICE), ASSET_3, ASSET_5, amount_to_sell, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_should_work_when_assets_are_in_different_subpool() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID_2)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, OMNIPOOL_INITIAL_ASSET_6_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);
			create_subpool!(SHARE_ASSET_AS_POOL_ID_2, ASSET_5, ASSET_6);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				ASSET_3,
				ASSET_5,
				amount_to_sell,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let pool_account2 = AccountIdConstructor::from_assets(&vec![ASSET_5, ASSET_6], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 97944844908417;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, amount_to_get);
			assert_balance!(ALICE, ASSET_6, 0);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account2, ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE - amount_to_get);
			assert_balance!(pool_account2, ASSET_6, OMNIPOOL_INITIAL_ASSET_6_BALANCE);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(omnipool_account, ASSET_5, 0);
			assert_balance!(omnipool_account, ASSET_6, 0);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4615493725412862,
					hub_reserve: 4485435628698236,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID_2,
				AssetReserveState::<Balance> {
					reserve: 7086013426306942,
					hub_reserve: 7214564371301764,
					shares: 7150 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_that_imbalance_is_zero!();
		});
}

#[test]
fn sell_should_work_when_both_asset_in_omnipool() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				ASSET_3,
				ASSET_4,
				amount_to_sell,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 94488188976377;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, amount_to_get);

			assert_balance!(pool_account, ASSET_3, 0);
			assert_balance!(pool_account, ASSET_4, 0);

			assert_balance!(
				omnipool_account,
				ASSET_3,
				OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell
			);
			assert_balance!(
				omnipool_account,
				ASSET_4,
				OMNIPOOL_INITIAL_ASSET_4_BALANCE - amount_to_get
			);

			assert_that_imbalance_is_zero!();
		});
}

#[test]
fn sell_should_work_when_selling_stable_asset_for_omnipool_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				ASSET_3,
				ASSET_5,
				amount_to_sell,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 97394957631199;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, amount_to_get);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, ASSET_5, 0);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			let new_quantity_of_asset_5_in_omnipool = OMNIPOOL_INITIAL_ASSET_5_BALANCE - amount_to_get;
			assert_balance!(omnipool_account, ASSET_5, new_quantity_of_asset_5_in_omnipool);

			assert_asset_state_in_omnipool!(
				ASSET_5,
				AssetReserveState::<Balance> {
					reserve: new_quantity_of_asset_5_in_omnipool,
					hub_reserve: 3314564371301764,
					shares: 5000 * ONE,
					protocol_shares: 0,
					cap: 1000000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4615493725412862,
					hub_reserve: 4485435628698236,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_that_imbalance_is_zero!();
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::BUY)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn sell_should_fail_when_selling_stable_asset_for_omnipool_asset_but_stable_asset_is_not_sellable(
	tradability: Tradability,
) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::signed(ALICE), ASSET_3, ASSET_5, amount_to_sell, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_should_work_when_selling_omnipool_asset_for_stableswap_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				ASSET_5,
				ASSET_3,
				amount_to_sell,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 95863515768316;

			assert_balance!(ALICE, ASSET_3, amount_to_get);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE - amount_to_sell);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE - amount_to_get);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, ASSET_5, 0);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			let new_quantity_of_asset_5_in_omnipool = OMNIPOOL_INITIAL_ASSET_5_BALANCE + amount_to_sell;
			assert_balance!(omnipool_account, ASSET_5, new_quantity_of_asset_5_in_omnipool);

			assert_asset_state_in_omnipool!(
				ASSET_5,
				AssetReserveState::<Balance> {
					reserve: new_quantity_of_asset_5_in_omnipool,
					hub_reserve: 3186274509803922,
					shares: 5000 * ONE,
					protocol_shares: 0,
					cap: 1000000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4487154696132598,
					hub_reserve: 4613725490196078,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_that_imbalance_is_zero!();
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::SELL)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn sell_should_fail_when_selling_omnipool_asset_for_stableswap_asset_but_stable_asset_is_not_buyable(
	tradability: Tradability,
) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::signed(ALICE), ASSET_5, ASSET_3, amount_to_sell, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_should_work_when_selling_lrna_for_stableswap_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, LRNA, ALICE_INITIAL_LRNA_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			let initial_lrna_balance_in_omnipool = 15050000000000000;
			assert_balance!(omnipool_account, LRNA, initial_lrna_balance_in_omnipool);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				LRNA,
				ASSET_3,
				amount_to_sell,
				0
			));

			//Assert
			let amount_to_get = 149216847560367;

			assert_balance!(ALICE, ASSET_3, amount_to_get);
			assert_balance!(ALICE, ASSET_4, 0);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE - amount_to_get);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(
				omnipool_account,
				LRNA,
				initial_lrna_balance_in_omnipool + amount_to_sell
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4452150537634409,
					hub_reserve: 4650000000000000,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_imbalance!(SimpleImbalance {
				value: 644_606_312_868_540,
				negative: true
			});
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::SELL)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn sell_should_work_when_selling_lrna_for_stableswap_asset_but_stableswap_has_no_buyable_state(
	tradability: Tradability,
) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, LRNA, ALICE_INITIAL_LRNA_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::signed(ALICE), LRNA, ASSET_3, amount_to_sell, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_should_fail_when_called_by_non_signed_user() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::none(), ASSET_3, ASSET_4, amount_to_sell, 0),
				BadOrigin
			);
		});
}

#[test]
fn sell_should_fail_when_called_by_root() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_sell = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::sell(Origin::root(), ASSET_3, ASSET_4, amount_to_sell, 0),
				BadOrigin
			);
		});
}
