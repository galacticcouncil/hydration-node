use super::*;
use crate::types::Balance;
use crate::*;
use proptest::prelude::*;
use sp_runtime::traits::CheckedAdd;
use sp_runtime::traits::CheckedDiv;
use sp_runtime::traits::CheckedMul;
use test_utils::assert_balance;

proptest! {
	//Spec: https://www.notion.so/Create-new-stableswap-subpool-from-two-assets-in-the-Omnipool-permissioned-20028c583ac64c55aee8443a23a096b9#5a361cb3ed434788a035fe3cfc48e170
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn create_subpool_invariants(
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
			.add_endowed_accounts((ALICE, asset_3.asset_id, 100 * ONE))
			.add_endowed_accounts((ALICE, asset_4.asset_id, 100 * ONE))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::root(), asset_3.asset_id, asset_3.price,Permill::from_percent(100),LP1));
				assert_ok!(Omnipool::add_token(Origin::root(), asset_4.asset_id, asset_4.price,Permill::from_percent(100),LP1));

				//We need to add then sacrifice liquidity for asset3 to have protocol shares
				let position_id: u32 = Omnipool::next_position_id();
				assert_ok!(OmnipoolSubpools::add_liquidity(
					Origin::signed(ALICE),
					asset_3.asset_id,
					100 * ONE
				));
				assert_ok!(Omnipool::sacrifice_position(Origin::signed(ALICE), position_id));

				//We need to add then sacrifice liquidity for asset3 to have protocol shares
				let position_id: u32 = Omnipool::next_position_id();
				assert_ok!(OmnipoolSubpools::add_liquidity(
					Origin::signed(ALICE),
					asset_4.asset_id,
					100 * ONE
				));
				assert_ok!(Omnipool::sacrifice_position(Origin::signed(ALICE), position_id));

				let asset_state_3 = Omnipool::load_asset_state(asset_3.asset_id).unwrap();
				let asset_state_4 = Omnipool::load_asset_state(asset_4.asset_id).unwrap();

				let sum_q = asset_state_3.hub_reserve.checked_add(asset_state_4.hub_reserve).unwrap();
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

				let stableswap_pool_share_asset = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);
				let q_s = stableswap_pool_share_asset.hub_reserve;

				let omnipool_lrna_balance_after = get_lrna_of_omnipool_protocol_account();

				let s_s_plus = stableswap_pool_share_asset.shares;

				let q_s_plus = stableswap_pool_share_asset.hub_reserve;
				let b_s_plus = stableswap_pool_share_asset.protocol_shares;
				let r_3 = asset_state_3.reserve;
				let q_3 = asset_state_3.hub_reserve;
				let b_3 = asset_state_3.protocol_shares;
				let s_3 = asset_state_3.shares;
				let r_4 = asset_state_4.reserve;
				let q_4 = asset_state_4.hub_reserve;
				let b_4 = asset_state_4.protocol_shares;
				let s_4 = asset_state_4.shares;

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				//Sum(Qk) = Qs
				let left = q_3.checked_add(q_4).unwrap();
				let right = q_s;
				assert_invariant_eq!(left, right);

				//Check that the full amount of lrna has not been changed
				let left = omnipool_lrna_balance_before;
				let right = omnipool_lrna_balance_after;
				assert_invariant_eq!(left, right);

				//No risk assets are accounted for: Rk = Rsk
				assert_balance!(Omnipool::protocol_account(), asset_3.asset_id, 0);
				assert_balance!(Omnipool::protocol_account(), asset_4.asset_id, 0);
				assert_balance!(pool_account, asset_3.asset_id, r_3);
				assert_balance!(pool_account, asset_4.asset_id, r_4);

				// Us+ = Ss+
				let left = u_s_plus;
				let right = s_s_plus;
				assert_invariant_eq!(left, right);

				// Ss+ = sum_Qk
				let left = s_s_plus;
				let right = sum_q;
				assert_invariant_eq!(left, right);

				// Qs+ * Bs+ / Ss+ = Sum(Qk * Bk/Sk)
				let left = q_s_plus.checked_mul_into(&b_s_plus).unwrap().checked_div_inner(&s_s_plus).unwrap();
				let right_3 = q_3.checked_mul_into(&b_3).unwrap().checked_div_inner(&s_3).unwrap();
				let right_4 = q_4.checked_mul_into(&b_4).unwrap().checked_div_inner(&s_4).unwrap();
				let right = right_3.checked_add(right_4).unwrap();

				assert_invariant_eq!(left, right);
			});
	}
}

fn get_lrna_of_omnipool_protocol_account() -> Balance {
	Tokens::free_balance(LRNA, &Omnipool::protocol_account())
}
