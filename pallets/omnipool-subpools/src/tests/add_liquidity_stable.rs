use super::*;

use crate::{
	add_omnipool_token, assert_asset_state_in_omnipool, assert_balance, assert_stableswap_pool_assets,
	assert_that_asset_is_migrated_to_omnipool_subpool, assert_that_asset_is_not_present_in_omnipool,
	assert_that_nft_position_is_not_present, assert_that_nft_position_is_present,
	assert_that_position_is_not_present_in_omnipool, assert_that_position_is_present_in_omnipool, create_subpool,
	AssetDetail, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, Position, Tradability};
use pretty_assertions::assert_eq;

const ALICE_INITIAL_ASSET_3_BALANCE: u128 = 1000 * ONE;
const ALICE_INITIAL_ASSET_4_BALANCE: u128 = 2000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: u128 = 5000 * ONE;

const MINTING_DEPOSIT_NFT: bool = true;
const NOT_MINTING_DEPOSIT_NFT: bool = false;

#[test]
fn add_liqudity_stable_should_add_liqudity_to_both_omnipool_and_subpool_when_minting_nft_is_on() {
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

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 4550000000000000;
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			//Act
			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity_stable(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity,
				MINTING_DEPOSIT_NFT
			));

			//Assert
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

			assert_that_nft_position_is_present!(position_id);

			let token_price = FixedU128::from_float(1.0);
			assert_that_position_is_present_in_omnipool!(
				ALICE,
				position_id,
				Position {
					asset_id: SHARE_ASSET_AS_POOL_ID,
					amount: deposited_share_of_alice,
					shares: deposited_share_of_alice,
					price: token_price.into_inner()
				}
			);
		});
}

#[test]
fn add_liqudity_stable_should_return_error_when_asset_is_not_migrated_so_stableswap_yet() {
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
			assert_noop!(
				OmnipoolSubpools::add_liquidity_stable(Origin::signed(ALICE), ASSET_3, 100 * ONE, MINTING_DEPOSIT_NFT),
				Error::<Test>::NotStableAsset
			);
		});
}

#[test]
fn add_liqudity_stable_should_add_liqudity_to_subpool_but_not_to_omnipool_when_minting_nft_is_off() {
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

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			let all_subpool_shares = 4550000000000000;
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			//Act
			let position_id: u32 = Omnipool::next_position_id();
			let new_liquidity = 100 * ONE;
			assert_ok!(OmnipoolSubpools::add_liquidity_stable(
				Origin::signed(ALICE),
				ASSET_3,
				new_liquidity,
				NOT_MINTING_DEPOSIT_NFT
			));

			//Assert liquidity added to subpool
			assert_balance!(ALICE, ASSET_3, ALICE_INITIAL_ASSET_3_BALANCE - new_liquidity);
			assert_balance!(&pool_account, ASSET_3, 3000 * ONE + new_liquidity);

			//Assert that share of ALICE is deposited to stableswap
			let deposited_share_of_alice = 65051679689491;
			assert_balance!(ALICE, SHARE_ASSET_AS_POOL_ID, deposited_share_of_alice);
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, all_subpool_shares);

			assert_that_nft_position_is_not_present!(position_id);
			assert_that_position_is_not_present_in_omnipool!(ALICE, position_id);
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

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::add_liquidity_stable(Origin::none(), ASSET_3, 100 * ONE, MINTING_DEPOSIT_NFT),
				BadOrigin
			);
		});
}
