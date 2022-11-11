use super::*;

use crate::AssetDetail;
use crate::{
	add_omnipool_token, assert_balance, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_sharetoken_in_omnipool_as_another_asset,
	assert_that_stableswap_subpool_is_created_with_poolinfo, create_subpool, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::AssetState;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pallet_stableswap::types::PoolInfo;
use pretty_assertions::assert_eq;
use sp_runtime::BoundedVec;

const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
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

			let amount_to_get = 99835772816269;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, amount_to_get);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE - amount_to_get);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
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

			let amount_to_get = 4902260110173227;

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
		});
}

#[ignore] // Implement once it is implemented in the prod code
#[test]
fn sell_should_work_when_one_asset_in_omnipool_and_other_in_subpool() {
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
			/*let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			let amount_to_get = 99835772816269;

			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - amount_to_sell);
			assert_balance!(ALICE, ASSET_4, 0);
			assert_balance!(ALICE, ASSET_5, amount_to_get);

			assert_balance!(pool_account, ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE + amount_to_sell);
			assert_balance!(pool_account, ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE);
			assert_balance!(pool_account, ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE - amount_to_get);

			assert_balance!(omnipool_account, ASSET_3, 0);
			assert_balance!(omnipool_account, ASSET_4, 0);
			assert_balance!(omnipool_account, ASSET_5, OMNIPOOL_INITIAL_ASSET_5_BALANCE -);*/
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
