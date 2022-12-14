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

fn percent() -> impl Strategy<Value = Permill> {
	(1..100u32).prop_map(Permill::from_percent)
}

fn amplification() -> impl Strategy<Value = u16> {
	(2..10_000u16)
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
	fn sell_lrna_for_stableswap_asset(
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		amplification in amplification(),
		share_asset_weight_cap in percent(),
		trade_fee in percent(),
		withdraw_fee in percent()
	) {
		ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((LP1, asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((LP1, asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::root(), asset_3.asset_id, asset_3.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_4.asset_id, asset_4.price,Permill::from_percent(100),LP1));

				let asset_state_3 = Omnipool::load_asset_state(asset_3.asset_id).unwrap();
				let asset_state_4 = Omnipool::load_asset_state(asset_4.asset_id).unwrap();

				let asset_3_lrna = asset_state_3.hub_reserve;
				let asset_4_lrna = asset_state_4.hub_reserve;

				let asset_3_reserve = asset_state_3.reserve;
				let asset_4_reserve = asset_state_4.reserve;

				let omnipool_lrna_balance_before = get_lrna_of_omnipool_protocol_account();

				//Act
				assert_ok!(OmnipoolSubpools::create_subpool(
					Origin::root(),
					SHARE_ASSET_AS_POOL_ID,
					asset_3.asset_id,
					asset_4.asset_id,
					share_asset_weight_cap,
					amplification,
					trade_fee,
					withdraw_fee,
				));

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				//Check that the lrna has been migrated
				let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let share_asset_lrna = stableswap_pool_share_asset.hub_reserve;
				assert_eq!(asset_3_lrna + asset_4_lrna, share_asset_lrna);

				//Check that the full amount of lrna has not been changed
				let omnipool_lrna_balance_after = get_lrna_of_omnipool_protocol_account();
				assert_eq!(omnipool_lrna_balance_before, omnipool_lrna_balance_after);

				//Check that we transfer the right reserve from omnipool to subpool
				assert_balance!(Omnipool::protocol_account(), asset_3.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_4.asset_id, 0);
				assert_balance!(pool_account, asset_3.asset_id, asset_3_reserve);
				assert_balance!(pool_account, asset_4.asset_id, asset_4_reserve);

				//Spec: https://www.notion.so/Create-new-stableswap-subpool-from-two-assets-in-the-Omnipool-permissioned-20028c583ac64c55aee8443a23a096b9#f1da37ba2acb4c8a8f40cdbae5751cc0
				assert_eq!(stableswap_pool_share_asset.shares, stableswap_pool_share_asset.reserve);
				assert_eq!(stableswap_pool_share_asset.shares, asset_3_lrna + asset_4_lrna);

				//Spec: https://www.notion.so/Create-new-stableswap-subpool-from-two-assets-in-the-Omnipool-permissioned-20028c583ac64c55aee8443a23a096b9#9e1438cd504040e38e25269ea9fca1b4
				let left_expression = stableswap_pool_share_asset.hub_reserve * stableswap_pool_share_asset.protocol_shares / stableswap_pool_share_asset.shares;
				let right_expression_for_asset3 = asset_state_3.hub_reserve * asset_state_3.protocol_shares / asset_state_3.shares;
				let right_expression_for_asset4 = asset_state_3.hub_reserve * asset_state_3.protocol_shares / asset_state_3.shares;
				assert_eq!(left_expression, right_expression_for_asset3 + right_expression_for_asset4);
			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
