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

pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000; // * 1_000 * 1_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);

fn asset_reserve() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..5000 * ONE
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

fn pool_token(asset_id: AssetId) -> impl Strategy<Value = PoolToken> {
	(asset_reserve(), price()).prop_map(move |(reserve, price)| PoolToken {
		asset_id,
		amount: reserve,
		price,
	})
}

#[derive(Debug)]
struct PoolToken {
	asset_id: AssetId,
	amount: Balance,
	price: FixedU128,
}

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
		.add_endowed_accounts((ALICE, ASSET_3, sell_amount))
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
				ASSET_4,
				sell_amount,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

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
