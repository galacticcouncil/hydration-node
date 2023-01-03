use super::*;
use crate::types::Balance;
use crate::*;
use proptest::prelude::*;
use sp_runtime::traits::CheckedAdd;
use sp_runtime::traits::CheckedDiv;
use sp_runtime::traits::CheckedMul;
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

				let q_s = FixedU128::from(stableswap_pool_share_asset_before_migration.hub_reserve);
				let b_s = FixedU128::from(stableswap_pool_share_asset_before_migration.protocol_shares);
				let s_s = FixedU128::from(stableswap_pool_share_asset_before_migration.shares);

				//Act
				assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
					Origin::root(),
					SHARE_ASSET_AS_POOL_ID,
					asset_5.asset_id,
				));

				let omnipool_lrna_balance_after = get_lrna_of_omnipool_protocol_account();
				let sum_q_k = omnipool_lrna_balance_before;
				let sum_q_k_plus = omnipool_lrna_balance_after;

				let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let s_s_plus = stableswap_pool_share_asset.shares;
				let u_s_plus = stableswap_pool_share_asset.reserve;

				let q_s_plus = FixedU128::from(stableswap_pool_share_asset.hub_reserve);
				let b_s_plus = FixedU128::from(stableswap_pool_share_asset.protocol_shares);

				 let q_5 = FixedU128::from(asset_state_5.hub_reserve);
				let b_5 = FixedU128::from(asset_state_5.protocol_shares);
				let s_5 = FixedU128::from(asset_state_5.shares);

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id, asset_5.asset_id], None);

				//Sum(Qk) = Sum(Qk+)
				let left = sum_q_k;
				let right = sum_q_k_plus;
				assert_invariant_eq!(left, right);

				//No risk assets are accounted for: Ri = Rsi
				assert_balance!(Omnipool::protocol_account(), asset_3.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_4.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_5.asset_id, 0);
				assert_balance!(pool_account, asset_3.asset_id, asset_3_reserve);
				assert_balance!(pool_account, asset_4.asset_id, asset_4_reserve);
				assert_balance!(pool_account, asset_5.asset_id, asset_5_reserve);

				// Us+ = Ss+
				let left = u_s_plus;
				let right = s_s_plus;
				assert_invariant_eq!(left, right);

				// Ss+ = sum_Qk
				let sum_q_k = asset_3_lrna + asset_4_lrna + asset_5_lrna;
				let left = s_s_plus;
				let right = sum_q_k;
				 assert_invariant_eq!(left, right);

				// Qs+ * Bs+/Ss+ = (Qi * Bi/Si) + (Qs * Bs/Ss)
				let s_s_plus = FixedU128::from(s_s_plus);
				let left = q_s_plus.checked_mul(&b_s_plus.checked_div(&s_s_plus).unwrap()).unwrap();
				let right1 = q_5.checked_mul(&b_5.checked_div(&s_5).unwrap()).unwrap();
				let right2 = q_s.checked_mul(&b_s.checked_div(&s_s).unwrap()).unwrap();
				let right = right1.checked_add(&right2).unwrap();

				#[cfg(feature = "all-invariants")]
				assert_invariant_eq!(left, right);

			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
