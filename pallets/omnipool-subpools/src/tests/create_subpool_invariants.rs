use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
use primitive_types::U256;
use proptest::prelude::*;
use test_utils::assert_balance;
pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000; // * 1_000 * 1_000;

//TODO: Add all of these strategy prop test helprs to some common place
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
	//Spec: https://www.notion.so/Create-new-stableswap-subpool-from-two-assets-in-the-Omnipool-permissioned-20028c583ac64c55aee8443a23a096b9#5a361cb3ed434788a035fe3cfc48e170
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_lrna_for_stableswap_asset(sell_amount in trade_amount(),
		token_1 in pool_token(ASSET_3),
		token_2 in pool_token(ASSET_4),
		native_reserve in asset_reserve(),
	) {
		ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, token_1.asset_id, token_1.amount))
		.add_endowed_accounts((LP1, token_2.asset_id, token_2.amount))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(Origin::root(), token_1.asset_id, token_1.price,Permill::from_percent(100),LP1));
			assert_ok!(Omnipool::add_token(Origin::root(), token_2.asset_id, token_2.price,Permill::from_percent(100),LP1));

			let asset_state_3 = Omnipool::load_asset_state(ASSET_3).unwrap();
			let asset_state_4 = Omnipool::load_asset_state(ASSET_4).unwrap();

			let asset_3_lrna = asset_state_3.hub_reserve;
			let asset_4_lrna = asset_state_4.hub_reserve;
			// assert_eq!(asset_state_4.hub_reserve, 400 * ONE);

			//Act
			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Assert
			let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
			let share_asset_lrna = stableswap_pool_share_asset.hub_reserve;

			assert_eq!(asset_3_lrna + asset_4_lrna, share_asset_lrna)

			//let migrate_asset = OmnipoolSubpools::migrated_assets(token_1.asset_id);

		});
	}
}

//Create subpools
//1
//- after the pool is crete,d all the lrna is correctly migrated. the two assets is migrated and we get share assets. the share asset lrna
//asset a has 100lrna, asset b has 200lrna, Checking the sum for separate

//2
// the amount of lrna in omnipool should not change

//3
// Migrating means we transfer reerver from omnipool to stableswap account

//4
//the share asset amount should have lrna and shares. THe reserve is equal to the shares is equal to share asset. It should be equal to amount of LRNA
//make sure that we mint  the correct amount of the share asset. it must be in the omnipool account.

//5
//STATE OF SHARE ASSET -

//LRNA * protocol shares / shares = SUMMA same for each asset migrated
