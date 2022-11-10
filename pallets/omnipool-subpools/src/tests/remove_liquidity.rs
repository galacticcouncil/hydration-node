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

			//TODO: verify the bug with Martin first in remove_li, asset should be used, and not position.asset_id,
			//TODO: Once convert is implemented: ensure the position data is update correctly - mainly when position has to be migrated within remove liquidity - it should change asset id to share asset id and data recalculated
		});
}

#[test_case(Option::Some(ASSET_3))]
#[test_case(Option::None)]
fn remove_liqudity_should_work_when_asset_is_not_migrated_to_subpool(asset_id: Option<AssetId>) {
	let omnipool_account_asset_3_balance = 3000 * ONE;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, omnipool_account_asset_3_balance))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity);
			assert_balance!(&pool_account, ASSET_3, 0);
			assert_balance!(
				&omnipool_account,
				ASSET_3,
				omnipool_account_asset_3_balance + new_liquidity
			);

			assert_balance!(ALICE, SHARE_ASSET_AS_POOL_ID, 0);

			//Act
			assert_ok!(OmnipoolSubpools::remove_liquidity(
				Origin::signed(ALICE),
				position_id,
				new_liquidity,
				asset_id,
			));

			//Assert
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE);
			assert_balance!(&omnipool_account, ASSET_3, omnipool_account_asset_3_balance);

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

#[test]
fn remove_liqudity_should_fail_when_asset_is_migrated_but_withdraw_asset_is_not_specified() {
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
			assert_noop!(
				OmnipoolSubpools::remove_liquidity(
					Origin::signed(ALICE),
					position_id,
					deposited_share_of_alice,
					Option::None,
				),
				Error::<Test>::WithdrawAssetNotSpecified
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_called_with_non_origin() {
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

			//Act
			let deposited_share_of_alice = 65051679689491;
			assert_noop!(
				OmnipoolSubpools::remove_liquidity(
					Origin::none(),
					position_id,
					deposited_share_of_alice,
					Option::Some(ASSET_3),
				),
				BadOrigin
			);
		});
}
