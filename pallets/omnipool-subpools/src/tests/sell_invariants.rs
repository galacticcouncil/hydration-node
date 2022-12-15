use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
use primitive_types::U256;
use proptest::prelude::*;
use test_utils::assert_balance;
const ALICE_INITIAL_LRNA_BALANCE: Balance = 500 * ONE;
const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_3_BALANCE: Balance = 3000 * ONE;
const OMNIPOOL_INITIAL_ASSET_4_BALANCE: Balance = 4000 * ONE;
const OMNIPOOL_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#697fb7cb7bb8464cafcab36089cf18e1
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_stableswap_asset_for_omnipool_asset(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
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

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			let asset_5_state_before_sell = Omnipool::load_asset_state(ASSET_5).unwrap();
			let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

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

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#22db4d7d9fbc4d6fbb718221c16e1af0
			let asset_5_state_after_sell = Omnipool::load_asset_state(ASSET_5).unwrap();
			let asset_5_reserve_with_hub_before = asset_5_state_before_sell.hub_reserve * asset_5_state_before_sell.reserve;
			let asset_5_reserve_with_hub_after = asset_5_state_after_sell.hub_reserve * asset_5_state_after_sell.reserve;
			assert!(asset_5_reserve_with_hub_after > asset_5_reserve_with_hub_before);

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#ed334c898a19431caafcba1f395b2d38
			let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
			let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
			let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
			assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);


			//AssetOutLrnaAfter * AssetOutReserveAfter >= same before
			//AssetInLrnaAfter * AssetInReservAfter >= same before
			//?
			//?
			//AssetInReserveAfter + ? = ?+ + assetInReserveBefore
			//?

			});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#363a38037c2d42d8977107df2439d274
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_omnipool_asset_for_stableswap_asset(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
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

				});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7a2f7db50bf54b41a96c02b633f24b94
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_lrna_for_stableswap_asset(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
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


				});
	}
}

//Q - LRNA
//R - asset in pool - shares of underlying stableswap subpool
//S - shares in pool
//Us - sum of shares held by omnipool (Rs) + those held directly by LPs so => Us = Rs + SUMMA(rs)
