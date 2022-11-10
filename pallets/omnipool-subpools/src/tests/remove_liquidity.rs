use super::*;

use crate::{
	add_omnipool_token, assert_balance, assert_balance_approx, assert_stableswap_pool_assets,
	assert_that_asset_is_migrated_to_omnipool_subpool, assert_that_asset_is_not_present_in_omnipool,
	assert_that_nft_position_is_not_present, assert_that_nft_position_is_present,
	assert_that_position_is_added_to_omnipool, assert_that_position_is_not_present_in_omnipool,
	assert_that_sharetoken_in_omnipool_as_another_asset, AssetDetail, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, Position, Tradability};
use pretty_assertions::assert_eq;
use test_case::test_case;

const ALICE_INITIAL_ASSET_3_BALANCE: u128 = 1000 * ONE;
const ALICE_INITIAL_ASSET_4_BALANCE: u128 = 2000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: u128 = 5000 * ONE;

#[test]
fn remove_liqudity_should_work_when_asset_is_migrated_to_subpool() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 4550000000000000;

			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity
			));

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE + new_liquidity);

			//Assert that share of ALICE is deposited and added to omnipool
			assert_balance!(ALICE, SHARE_ASSET_AS_POOL_ID, 0);
			let deposited_share_of_alice = 65051679689491;
			assert_balance!(
				&omnipool_account,
				SHARE_ASSET_AS_POOL_ID,
				all_subpool_shares + deposited_share_of_alice
			);

			//Act
			assert_ok!(OmnipoolSubpools::remove_liquidity(
				Origin::signed(ALICE),
				position_id,
				deposited_share_of_alice,
				Option::Some(ASSET_3),
			));

			//Assert
			let delta_due_to_rounding_error = 10;
			assert_balance_approx!(
				ALICE,
				ASSET_3,
				ALICE_INITIAL_ASSET_3_BALANCE,
				delta_due_to_rounding_error
			);

			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			assert_that_nft_position_is_not_present!(position_id);
			assert_that_position_is_not_present_in_omnipool!(ALICE, position_id);
		});
}

#[test_case(Tradability::FROZEN)]
#[test_case(Tradability::SELL)]
#[test_case(Tradability::BUY)]
#[test_case(Tradability::ADD_LIQUIDITY)]
fn remove_liqudity_should_fail_when_asset_has_tradable_state_disallowing_removing_liquidty(tradability: Tradability) {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity
			));

			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 4550000000000000;
			let deposited_share_of_alice = 65051679689491;
			assert_balance!(
				&omnipool_account,
				SHARE_ASSET_AS_POOL_ID,
				all_subpool_shares + deposited_share_of_alice
			);

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				tradability
			));

			//Act
			assert_noop!(
				OmnipoolSubpools::remove_liquidity(
					Origin::signed(ALICE),
					position_id,
					deposited_share_of_alice,
					Option::Some(ASSET_3),
				),
				pallet_omnipool::Error::<Test>::NotAllowed
			);
		});
}
/*
#[test]
fn add_liqudity_should_work_when_added_for_both_subpool_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((ALICE, ASSET_4, ALICE_INITIAL_ASSET_4_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 4550000000000000;
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE);
			assert_balance!(ALICE, ASSET_4, ALICE_INITIAL_ASSET_4_BALANCE);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(&pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			//Act
			let position_id_for_asset_3_liq: u32 = Omnipool::next_position_id();
			let new_liquidity_for_asset_3 = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity_for_asset_3
			));

			let position_id_for_asset_4_liq: u32 = Omnipool::next_position_id();
			let new_liquidity_for_asset_4 = 500 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_4,
				new_liquidity_for_asset_4
			));

			//Assert that liquidity is added to subpool
			assert_balance!(ALICE, SHARE_ASSET_AS_POOL_ID, 0);

			let deposited_asset_3_share_of_alice = 65051679689491;
			let deposited_asset_4_share_of_alice = 324772754874054;
			let all_share_of_alice_to_be_deposited =
				deposited_asset_3_share_of_alice + deposited_asset_4_share_of_alice;
			assert_balance!(
				ALICE,
				ASSET_3,
				ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity_for_asset_3
			);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE + new_liquidity_for_asset_3);

			assert_balance!(
				ALICE,
				ASSET_4,
				ALICE_INITIAL_ASSET_4_BALANCE - new_liquidity_for_asset_4
			);
			assert_balance!(&pool_account, ASSET_4, 4000 * ONE + new_liquidity_for_asset_4);

			//Assert that share of ALICE is deposited and added to omnipool
			assert_balance!(ALICE, SHARE_ASSET_AS_POOL_ID, 0);
			assert_balance!(
				&omnipool_account,
				SHARE_ASSET_AS_POOL_ID,
				all_subpool_shares + all_share_of_alice_to_be_deposited
			);

			assert_that_nft_is_minted!(position_id_for_asset_3_liq);
			assert_that_nft_is_minted!(position_id_for_asset_4_liq);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_3_liq,
				Position {
					asset_id: SHARE_ASSET_AS_POOL_ID,
					amount: deposited_asset_3_share_of_alice,
					shares: deposited_asset_3_share_of_alice,
					price: token_price.into_inner()
				}
			);

			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_4_liq,
				Position {
					asset_id: SHARE_ASSET_AS_POOL_ID,
					amount: deposited_asset_4_share_of_alice,
					shares: deposited_asset_4_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liquidity_should_work_when_liqudity_added_for_newly_migrated_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);

			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 7800000000000000;
			assert_balance!(ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE);
			assert_balance!(&pool_account, ASSET_5, 5000 * ONE);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			//Act
			let position_id_for_asset_5_liq = Omnipool::next_position_id();

			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_5,
				new_liquidity
			));

			//Assert that liquidity is added to subpool
			let deposited_asset_5_share_of_alice = 64984152898695;
			assert_balance!(ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE - new_liquidity);
			assert_balance!(&pool_account, ASSET_5, 5000 * ONE + new_liquidity);

			//Assert that share of ALICE is deposited and added to omnipool
			assert_balance!(
				&omnipool_account,
				SHARE_ASSET_AS_POOL_ID,
				all_subpool_shares + deposited_asset_5_share_of_alice
			);

			assert_that_nft_is_minted!(position_id_for_asset_5_liq);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_5_liq,
				Position {
					asset_id: SHARE_ASSET_AS_POOL_ID,
					amount: deposited_asset_5_share_of_alice,
					shares: deposited_asset_5_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liqudity_should_add_liqudity_to_only_omnipool_when_asset_is_not_migrated_to_subpool() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity
			));

			//Assert that liquidity is not added to stableswap subpool
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			assert_balance!(&pool_account, ASSET_3, 0);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, 0);

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity);
			assert_that_nft_is_minted!(position_id);

			let token_price = FixedU128::from_float(0.65);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id,
				Position {
					asset_id: ASSET_3,
					amount: new_liquidity,
					shares: new_liquidity,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liqudity_should_fail_when_omnipool_asset_has_no_tradeable_state_and_asset_is_migrated() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				Tradability::FROZEN
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::signed(ALICE), ASSET_3, 100 * ONE),
				pallet_omnipool::Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn add_liqudity_should_fail_when_omnipool_asset_has_no_tradeable_state_and_asset_is_not_migrated() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				ASSET_3,
				Tradability::FROZEN
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::signed(ALICE), ASSET_3, 100 * ONE),
				pallet_omnipool::Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn add_liqudity_should_fail_when_weight_cap_exceeded() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, 10000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::signed(ALICE), ASSET_3, 10000 * ONE),
				pallet_omnipool::Error::<Test>::AssetWeightCapExceeded
			);
		});
}

#[test]
fn add_liqudity_should_fail_when_user_has_not_enough_balance_for_migrated_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act
			let new_liquidity = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::signed(ALICE), ASSET_3, new_liquidity),
				pallet_stableswap::Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn add_liqudity_should_fail_when_user_has_not_enough_balance_for_not_migrated_asset() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
			let new_liquidity = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::signed(ALICE), ASSET_3, new_liquidity),
				pallet_omnipool::Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn add_liqudity_should_fail_with_invalid_origin() {
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act
			let new_liquidity = 100 * ONE;
			assert_noop!(
				OmnipoolSubpools::add_liquidity(Origin::none(), ASSET_3, new_liquidity),
				BadOrigin
			);
		});
}
*/
