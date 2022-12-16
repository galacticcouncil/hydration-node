use super::*;
use crate::types::Balance;
use crate::*;
use proptest::prelude::*;
use test_utils::assert_balance;

proptest! {
	//Spec: https://www.notion.so/Add-Omnipool-asset-to-existing-stableswap-subpool-permissioned-d7ece293a23546a186a385a51f53212c
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn add_omnipool_asset_to_existing_stableswap_subpool(
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		asset_5 in pool_token(ASSET_5),
		amplification in amplification(),
		share_asset_weight_cap in percent(),
		trade_fee in percent(),
		withdraw_fee in percent()
	) {
		ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(asset_5.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((LP1, asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((LP1, asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((LP1, asset_5.asset_id, asset_5.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_5.asset_id, asset_5.amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::root(), asset_3.asset_id, asset_3.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_4.asset_id, asset_4.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_5.asset_id, asset_5.price,Permill::from_percent(100),LP1));

				let asset_state_3 = Omnipool::load_asset_state(asset_3.asset_id).unwrap();
				let asset_state_4 = Omnipool::load_asset_state(asset_4.asset_id).unwrap();
				let asset_state_5 = Omnipool::load_asset_state(asset_5.asset_id).unwrap();

				let asset_3_lrna = asset_state_3.hub_reserve;
				let asset_4_lrna = asset_state_4.hub_reserve;
				let asset_5_lrna = asset_state_5.hub_reserve;

				let asset_3_reserve = asset_state_3.reserve;
				let asset_4_reserve = asset_state_4.reserve;
				let asset_5_reserve = asset_state_5.reserve;

				let omnipool_lrna_balance_before = get_lrna_of_omnipool_protocol_account();

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

				let stableswap_pool_share_asset_before_migration = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				//Act
				assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
					Origin::root(),
					SHARE_ASSET_AS_POOL_ID,
					asset_5.asset_id,
				));

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id, asset_5.asset_id], None);

				//Check that the full amount of lrna has not been changed
				let omnipool_lrna_balance_after = get_lrna_of_omnipool_protocol_account();
				assert_eq!(omnipool_lrna_balance_before, omnipool_lrna_balance_after);

				//Check that we transfer the right reserve from omnipool to subpool
				assert_balance!(Omnipool::protocol_account(), asset_3.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_4.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_5.asset_id, 0);
				assert_balance!(pool_account, asset_3.asset_id, asset_3_reserve);
				assert_balance!(pool_account, asset_4.asset_id, asset_4_reserve);
				assert_balance!(pool_account, asset_5.asset_id, asset_5_reserve);

				//Spec: https://www.notion.so/Add-Omnipool-asset-to-existing-stableswap-subpool-permissioned-d7ece293a23546a186a385a51f53212c#eea9ff0460b343e2be1fad13388f247f
				let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				assert_eq!(stableswap_pool_share_asset.shares, stableswap_pool_share_asset.reserve);
				assert_eq!(stableswap_pool_share_asset.shares, asset_3_lrna + asset_4_lrna +  asset_5_lrna);

				//Spec: https://www.notion.so/Add-Omnipool-asset-to-existing-stableswap-subpool-permissioned-d7ece293a23546a186a385a51f53212c
				let left_expression = stableswap_pool_share_asset.hub_reserve * stableswap_pool_share_asset.protocol_shares / stableswap_pool_share_asset.shares;
				let right_expression_for_asset5 = asset_state_5.hub_reserve * asset_state_5.protocol_shares / asset_state_5.shares;
				let right_expression_for_share_asset_before_migration = stableswap_pool_share_asset_before_migration.hub_reserve * stableswap_pool_share_asset_before_migration.protocol_shares / stableswap_pool_share_asset_before_migration.shares;
				assert_eq!(left_expression, right_expression_for_asset5 + right_expression_for_share_asset_before_migration);
			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
