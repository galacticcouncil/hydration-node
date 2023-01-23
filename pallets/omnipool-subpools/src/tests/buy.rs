use super::*;
use crate::types::Balance;
use crate::*;
use test_utils::assert_balance;

use frame_support::error::BadOrigin;
use pallet_omnipool::types::SimpleImbalance;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;
use test_case::test_case;

const ALICE_INITIAL_LRNA_BALANCE: Balance = 500 * ONE;
const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_3_BALANCE: Balance = 3000 * ONE;
const OMNIPOOL_INITIAL_ASSET_4_BALANCE: Balance = 4000 * ONE;
const OMNIPOOL_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_6_BALANCE: Balance = 6000 * ONE;

const MAX_SELL_AMOUNT: Balance = 1000 * ONE;

#[test]
fn buy_should_work_when_both_asset_in_same_subpool() {
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
			let amount_to_buy = 100 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_4,
				ASSET_3,
				amount_to_buy,
				MAX_SELL_AMOUNT
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_spend = 98743876403980;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_spend);
			assert_balance!(ALICE, ASSET_4, amount_to_buy);

			assert_balance!(
				pool_account,
				ASSET_3,
				OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_spend
			);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE - amount_to_buy);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);

			assert_that_imbalance_is_zero!();
		});
}

#[test]
fn buy_should_work_when_assets_are_in_different_subpool() {
	let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;

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
		.add_endowed_accounts((ALICE, ASSET_3, alice_initial_asset_3_balance))
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
			let amount_to_buy = 15 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_5,
				ASSET_3,
				amount_to_buy,
				MAX_SELL_AMOUNT * 100
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let pool_account2 = AccountIdConstructor::from_assets(&vec![ASSET_5, ASSET_6], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_spend = 15005692740828;
			assert_balance!(ALICE, ASSET_3, alice_initial_asset_3_balance - amount_to_spend);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, amount_to_buy);
			assert_balance!(ALICE, ASSET_6, 0);

			assert_balance!(
				pool_account,
				ASSET_3,
				OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_spend
			);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account2, ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE - amount_to_buy);
			assert_balance!(pool_account2, ASSET_6, OMNIPOOL_INITIAL_ASSET_6_BALANCE);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(omnipool_account, ASSET_5, 0);
			assert_balance!(omnipool_account, ASSET_6, 0);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4559831807614855,
					hub_reserve: 4540189391509379,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID_2,
				AssetReserveState::<Balance> {
					reserve: 7140202834328501,
					hub_reserve: 7159810608490621,
					shares: 7150 * ONE,
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
fn buy_should_fail_within_different_pools_when_asset_out_has_no_buyable_state(tradability: Tradability) {
	let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;

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
		.add_endowed_accounts((ALICE, ASSET_3, alice_initial_asset_3_balance))
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

			//Act and Assert
			let amount_to_buy = 15 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_5,
					ASSET_3,
					amount_to_buy,
					MAX_SELL_AMOUNT * 100
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::BUY)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn buy_should_fail_within_different_pools_when_asset_in_has_no_sellable_state(tradability: Tradability) {
	let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;

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
		.add_endowed_accounts((ALICE, ASSET_3, alice_initial_asset_3_balance))
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

			//Act and Assert
			let amount_to_buy = 15 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_5,
					ASSET_3,
					amount_to_buy,
					MAX_SELL_AMOUNT * 100
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_should_work_when_both_asset_in_omnipool() {
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
			let amount_to_buy = 100 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_4,
				ASSET_3,
				amount_to_buy,
				MAX_SELL_AMOUNT
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_spend = 106194690265488;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_spend);
			assert_balance!(ALICE, ASSET_4, amount_to_buy);

			assert_balance!(pool_account, ASSET_3, 0);
			assert_balance!(pool_account, ASSET_4, 0);

			assert_balance!(
				omnipool_account,
				ASSET_3,
				OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_spend
			);
			assert_balance!(
				omnipool_account,
				ASSET_4,
				OMNIPOOL_INITIAL_ASSET_4_BALANCE - amount_to_buy
			);

			assert_that_imbalance_is_zero!();
		});
}

#[test]
fn buy_should_work_when_buying_omnipool_asset_with_stablepool_asset() {
	let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, alice_initial_asset_3_balance))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			let amount_to_buy = 100 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_5,
				ASSET_3,
				amount_to_buy,
				alice_initial_asset_3_balance
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			//TODO: ask Martin - it feels too much, comparing to other tests
			let amount_to_spend = 102771024834982;

			assert_balance!(ALICE, ASSET_3, alice_initial_asset_3_balance - amount_to_spend);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, amount_to_buy);

			assert_balance!(
				pool_account,
				ASSET_3,
				OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_spend
			);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, ASSET_5, 0);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			let new_balance_of_asset_5_in_omnipool = OMNIPOOL_INITIAL_ASSET_5_BALANCE - amount_to_buy;
			assert_balance!(omnipool_account, ASSET_5, new_balance_of_asset_5_in_omnipool);

			assert_asset_state_in_omnipool!(
				ASSET_5,
				AssetReserveState::<Balance> {
					reserve: new_balance_of_asset_5_in_omnipool,
					hub_reserve: 3316326530612245,
					shares: 5000 * ONE,
					protocol_shares: 0,
					cap: 1000000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4617307692307693,
					hub_reserve: 4483673469387755,
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
fn buy_should_fail_when_buying_omnipool_asset_with_stablepool_asset_but_stableswap_is_not_sellable(
	tradability: Tradability,
) {
	let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_3, alice_initial_asset_3_balance))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let amount_to_buy = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_5,
					ASSET_3,
					amount_to_buy,
					alice_initial_asset_3_balance
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_should_work_when_buying_stableswap_asset_with_omnipool_asset() {
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

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4550 * ONE,
					hub_reserve: 4550 * ONE,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			//Act
			let amount_to_buy = 100 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_3,
				ASSET_5,
				amount_to_buy,
				ALICE_INITIAL_ASSET_5_BALANCE
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_spend = 104471838625069;

			assert_balance!(ALICE, ASSET_3, amount_to_buy);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE - amount_to_spend);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE - amount_to_buy);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, ASSET_5, 0);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(
				omnipool_account,
				ASSET_5,
				OMNIPOOL_INITIAL_ASSET_5_BALANCE + amount_to_spend
			);

			assert_asset_state_in_omnipool!(
				ASSET_5,
				AssetReserveState::<Balance> {
					reserve: 5104471838625069,
					hub_reserve: 3183483132777371,
					shares: 5000 * ONE,
					protocol_shares: 0,
					cap: 1000000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4484441537945676,
					hub_reserve: 4616516867222629,
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
fn buy_should_fail_when_buying_stableswap_asset_with_omnipool_asset_but_stableswap_is_not_buyable(
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
			let amount_to_buy = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_3,
					ASSET_5,
					amount_to_buy,
					ALICE_INITIAL_ASSET_5_BALANCE
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_should_work_when_buying_stableswap_asset_with_lrna() {
	let initial_omnipool_lrna_balance = 15050000000000000;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
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

			assert_balance!(omnipool_account, LRNA, initial_omnipool_lrna_balance);

			//Act
			let amount_to_buy = 100 * ONE;
			assert_ok!(OmnipoolSubpools::buy(
				Origin::signed(ALICE),
				ASSET_3,
				LRNA,
				amount_to_buy,
				ALICE_INITIAL_ASSET_5_BALANCE
			));

			//Assert
			let amount_to_spend = 66516867222629;

			assert_balance!(ALICE, ASSET_3, amount_to_buy);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, LRNA, ALICE_INITIAL_LRNA_BALANCE - amount_to_spend);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE - amount_to_buy);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, LRNA, 0);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(omnipool_account, LRNA, initial_omnipool_lrna_balance + amount_to_spend);

			assert_asset_state_in_omnipool!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 4484441537945676,
					hub_reserve: 4616516867222629,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 500000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_imbalance!(SimpleImbalance {
				value: 132741041779086,
				negative: true
			});
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::SELL)]
#[test_case(Tradability::ADD_LIQUIDITY)]
#[test_case(Tradability::REMOVE_LIQUIDITY)]
fn buy_should_fail_when_buying_stableswap_asset_with_lrna_but_stableasset_is_not_buyable(tradability: Tradability) {
	let initial_omnipool_lrna_balance = 15050000000000000;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, LRNA, ALICE_INITIAL_LRNA_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let omnipool_account = Omnipool::protocol_account();

			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(Omnipool::set_asset_tradable_state(Origin::root(), ASSET_3, tradability));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			assert_balance!(omnipool_account, LRNA, initial_omnipool_lrna_balance);

			//Act and assert
			let amount_to_buy = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_3,
					LRNA,
					amount_to_buy,
					ALICE_INITIAL_ASSET_5_BALANCE
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_should_fail_when_called_by_non_signed_user() {
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
			let amount_to_buy = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(Origin::none(), ASSET_4, ASSET_3, amount_to_buy, MAX_SELL_AMOUNT),
				BadOrigin
			);
		});
}

#[test]
fn buy_should_fail_when_called_by_root() {
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
			let amount_to_buy = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::buy(Origin::root(), ASSET_4, ASSET_3, amount_to_buy, MAX_SELL_AMOUNT),
				BadOrigin
			);
		});
}
