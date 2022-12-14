use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
use pallet_omnipool::types::AssetReserveState;
use pallet_omnipool::types::Tradability;
use primitive_types::U256;
use proptest::prelude::*;
use test_utils::assert_balance;

const ALICE_INITIAL_LRNA_BALANCE: Balance = 500 * ONE;
const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;
const ALICE_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_3_BALANCE: Balance = 3000 * ONE;
const OMNIPOOL_INITIAL_ASSET_4_BALANCE: Balance = 4000 * ONE;
const OMNIPOOL_INITIAL_ASSET_5_BALANCE: Balance = 5000 * ONE;
const OMNIPOOL_INITIAL_ASSET_6_BALANCE: Balance = 6000 * ONE;

const MAX_SELL_AMOUNT: Balance = 1000 * ONE;

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#0f4caec4aac240daaec1c611732cec05
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_stableswap_asset_with_omnipool_asset(sell_amount in trade_amount(),
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

			});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#3708f95e81104c648eea42afbd2afda6
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_omnipool_asset_with_with_stable(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
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
			});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#225d7f413f7f4de5b9804f284f20c5a4
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_stableswap_asset_with_with_lrna(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
	) {
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
			});
	}
}
