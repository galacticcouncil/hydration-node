use super::*;

use crate::{
	add_omnipool_token, assert_balance, assert_stableswap_pool_assets,
	assert_that_asset_is_migrated_to_omnipool_subpool, assert_that_asset_is_not_present_in_omnipool,
	assert_that_nft_is_minted, assert_that_position_is_added_to_omnipool,
	assert_that_sharetoken_in_omnipool_as_another_asset, AssetDetail, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, Position, Tradability};
use pretty_assertions::assert_eq;

const ALICE_INITIAL_ASSET_3_BALANCE: u128 = 1000 * ONE;
const ALICE_INITIAL_ASSET_4_BALANCE: u128 = 2000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: u128 = 5000 * ONE;

#[test]
fn add_liqudity_should_add_liqudity_to_both_omnipool_and_stableswap_when_asset_is_already_migrated_to_subpool() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
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
				share_asset_as_pool_id,
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
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(&omnipool_account, share_asset_as_pool_id, all_subpool_shares);

			//Act
			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity
			));

			//Assert that liquidity is added to subpool
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE + new_liquidity);

			//Assert that share of ALICE is deposited and added to omnipool
			let deposited_share_of_alice = 65051679689491;
			assert_balance!(
				&omnipool_account,
				share_asset_as_pool_id,
				all_subpool_shares + deposited_share_of_alice
			);

			assert_that_nft_is_minted!(position_id);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id,
				Position {
					asset_id: share_asset_as_pool_id,
					amount: deposited_share_of_alice,
					shares: deposited_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liqudity_should_work_when_added_for_both_subpool_asset() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
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
				share_asset_as_pool_id,
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
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(ALICE, ASSET_4, ALICE_INITIAL_ASSET_4_BALANCE);
			assert_balance!(&pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(&omnipool_account, share_asset_as_pool_id, all_subpool_shares);

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
			assert_balance!(
				&omnipool_account,
				share_asset_as_pool_id,
				all_subpool_shares + all_share_of_alice_to_be_deposited
			);

			assert_that_nft_is_minted!(position_id_for_asset_3_liq);
			assert_that_nft_is_minted!(position_id_for_asset_4_liq);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_3_liq,
				Position {
					asset_id: share_asset_as_pool_id,
					amount: deposited_asset_3_share_of_alice,
					shares: deposited_asset_3_share_of_alice,
					price: token_price.into_inner()
				}
			);

			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_4_liq,
				Position {
					asset_id: share_asset_as_pool_id,
					amount: deposited_asset_4_share_of_alice,
					shares: deposited_asset_4_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liquidity_should_work_when_liqudity_added_for_newly_migrated_asset() {
	let share_asset_as_pool_id: AssetId = 6;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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
				share_asset_as_pool_id,
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
				share_asset_as_pool_id,
				ASSET_5,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);

			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 7800000000000000;
			assert_balance!(ALICE, ASSET_5, ALICE_INITIAL_ASSET_5_BALANCE);
			assert_balance!(&pool_account, ASSET_5, 5000 * ONE);
			assert_balance!(&omnipool_account, share_asset_as_pool_id, all_subpool_shares);

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
				share_asset_as_pool_id,
				all_subpool_shares + deposited_asset_5_share_of_alice
			);

			assert_that_nft_is_minted!(position_id_for_asset_5_liq);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_added_to_omnipool!(
				ALICE,
				position_id_for_asset_5_liq,
				Position {
					asset_id: share_asset_as_pool_id,
					amount: deposited_asset_5_share_of_alice,
					shares: deposited_asset_5_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liqudity_should_add_liqudity_to_both_omnipool_when_asset_is_not_migrated_to_subpool() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
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
			assert_balance!(&omnipool_account, share_asset_as_pool_id, 0);

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
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
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
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(50),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				share_asset_as_pool_id,
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
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
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

//TODO: add liqudity when it is not migrated - then added
//TODO: add liqudity fail tradable when not migrated

//TODO: Add liqudity without enough balance
//TODO: Add liqudity fail with wrong origin
