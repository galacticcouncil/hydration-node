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
			.add_endowed_accounts((ALICE, asset_5.asset_id, 100 * ONE))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::root(), asset_3.asset_id, asset_3.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_4.asset_id, asset_4.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_5.asset_id, asset_5.price,Permill::from_percent(100),LP1));

				//We need to add then sacrifice liquidity for asset5 to have protocol shares
				let position_id: u32 = Omnipool::next_position_id();
				assert_ok!(OmnipoolSubpools::add_liquidity(
					Origin::signed(ALICE),
					asset_5.asset_id,
					100 * ONE
				));
				assert_ok!(Omnipool::sacrifice_position(Origin::signed(ALICE), position_id));

				let asset_state_3 = Omnipool::load_asset_state(asset_3.asset_id).unwrap();
				let asset_state_4 = Omnipool::load_asset_state(asset_4.asset_id).unwrap();
				let asset_state_5 = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let _r_3 = asset_state_3.reserve;
				let q_3 = asset_state_3.hub_reserve;
				let b_3 = asset_state_3.protocol_shares;
				let s_3 = asset_state_3.shares;
				let _r_4 = asset_state_4.reserve;
				let q_4 = asset_state_4.hub_reserve;
				let b_4 = asset_state_4.protocol_shares;
				let s_4 = asset_state_4.shares;

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

				let _q_s = stableswap_pool_share_asset_before_migration.hub_reserve;
				let _b_s = stableswap_pool_share_asset_before_migration.protocol_shares;
				let _s_s = stableswap_pool_share_asset_before_migration.shares;

				//Act
				assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
					Origin::root(),
					SHARE_ASSET_AS_POOL_ID,
					asset_5.asset_id,
				));

				let omnipool_lrna_balance_after = get_lrna_of_omnipool_protocol_account();

				let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let s_s_plus = stableswap_pool_share_asset.shares;

				let q_s_plus = stableswap_pool_share_asset.hub_reserve;
				let b_s_plus = stableswap_pool_share_asset.protocol_shares;

				let r_5 = asset_state_5.reserve;
				let q_5 = asset_state_5.hub_reserve;
				let s_5 = asset_state_5.shares;
				let b_5 = asset_state_5.protocol_shares;

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id, asset_5.asset_id], None);

				//Sum(Qk) = Sum(Qk+)
				let left = omnipool_lrna_balance_before;
				let right = omnipool_lrna_balance_after;
				assert_invariant_eq!(left, right);

				//No risk assets are accounted for: Ri = Rsi
				assert_balance!(Omnipool::protocol_account(), asset_5.asset_id, 0);
				assert_balance!(pool_account, asset_5.asset_id, r_5);

				// Us+ = Ss+
				let left = u_s_plus;
				let right = s_s_plus;
				assert_invariant_eq!(left, right);

				// Ss+ = sum_Qk
				let left = s_s_plus;
				let right = q_3 + q_4 + q_5;
				assert_invariant_eq!(left, right);

				// Qs+ * Bs+/Ss+ = (Qi * Bi/Si) + (Qs * Bs/Ss)
				let left = q_s_plus.checked_mul_into(&b_s_plus).unwrap().checked_div_inner(&s_s_plus).unwrap();
				let right_3 = q_3.checked_mul_into(&b_3).unwrap().checked_div_inner(&s_3).unwrap();
				let right_4 = q_4.checked_mul_into(&b_4).unwrap().checked_div_inner(&s_4).unwrap();
				let right_5 = q_5.checked_mul_into(&b_5).unwrap().checked_div_inner(&s_5).unwrap();
				let right = right_3.checked_add(right_4).unwrap().checked_add(right_5).unwrap();

				assert_invariant_eq!(left, right);

			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
