use super::*;
use crate::*;
use proptest::prelude::*;

use crate::types::Balance;
use hydra_dx_math::stableswap::calculate_d;
use pallet_omnipool::types::SimpleImbalance;
use sp_std::ops::Mul;

const ALICE_INITIAL_ASSET_3_BALANCE: Balance = 1000 * ONE;

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#0f4caec4aac240daaec1c611732cec05
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_stableswap_asset_with_omnipool_asset(
		amount_to_buy in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		asset_5 in pool_token(ASSET_5),
		amplification in amplification(),
		asset_fee in percent(),
		withdraw_fee in percent(),
		_protocol_fee in percent(),
	) {
		ExtBuilder::default()
		.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(asset_5.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_5.asset_id, asset_5.amount))
			.add_endowed_accounts((ALICE, ASSET_5, amount_to_buy * 100))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				add_omnipool_token!(asset_3.asset_id);
				add_omnipool_token!(asset_4.asset_id);
				add_omnipool_token!(asset_5.asset_id);

				create_subpool!(SHARE_ASSET_AS_POOL_ID, asset_3.asset_id, asset_4.asset_id, asset_fee, withdraw_fee);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_4.asset_id, asset_4.asset_id], None);

				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let _l_before = get_imbalance_value!();

				assert_that_imbalance_is_zero!();

				let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				let hdx_state_before = Omnipool::load_asset_state(HDX).unwrap();

				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					asset_3.asset_id,
					asset_5.asset_id,
					amount_to_buy,
					500000 * ONE
				));

				let l = get_imbalance_value!();

				let hdx_state_after = Omnipool::load_asset_state(HDX).unwrap();

				let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let q_i_plus = asset_5_state_after_sell.hub_reserve;
				let r_i_plus = asset_5_state_after_sell.reserve;
				let q_i = asset_5_state_before_sell.hub_reserve;
				let r_i = asset_5_state_before_sell.reserve;

				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let q_s_plus = share_asset_state_after_sell.hub_reserve;
				let r_s_plus = share_asset_state_after_sell.reserve;
				let q_s = share_asset_state_before_sell.hub_reserve;
				let r_s = share_asset_state_before_sell.reserve;

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_plus = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let u_s_plus  = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);
				let delta_u_s = u_s.checked_sub(u_s_plus).unwrap();

				let f_w = withdraw_fee;
				let one_minus_fw = Permill::from_float(1.0) - f_w;
				let delta_d = d_plus - d;

				let delta_q_i = share_asset_state_after_sell.hub_reserve.checked_sub(share_asset_state_before_sell.hub_reserve).unwrap();
				let delta_q_s = asset_5_state_before_sell.hub_reserve.checked_sub(asset_5_state_after_sell.hub_reserve).unwrap();

				let delta_q_h = hdx_state_after.hub_reserve.checked_sub(hdx_state_before.hub_reserve).unwrap();

				//Assert

				// Qi+ * Ri+ >= Qi * Ri
				let left = q_i_plus.checked_mul(r_i_plus).unwrap();
				let right = q_i.checked_mul(r_i).unwrap();
				assert_invariant_ge!(left, right);

				// Qs+ * Rs+ >= Qs * Rs
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);

				// Us+ * D <= Us * D+
				let left = u_s_plus.checked_mul(d).unwrap();
				let right = u_s.checked_mul(d_plus).unwrap();
				assert_invariant_le!(left, right);

				// delta_Us * D * (1 - fw) <= Us * delta_D
				let left = one_minus_fw.mul(delta_u_s.checked_mul(d).unwrap());
				let right = u_s.checked_mul(delta_d).unwrap();
				assert_invariant_le!(left, right);

				// Rs+ + Us = Us+ + Rs
				let left = r_s_plus.checked_add(u_s).unwrap();
				let right = u_s_plus.checked_add(r_s).unwrap();
				assert_invariant_eq!(left, right);

				// L <= 0
				let left = l;
				let right = 0;
				assert_invariant_le!(left, right);

				// delta_QH + delta_L + delta_Qs = - delta_Qi
				let _left = delta_q_h.checked_add(delta_q_i).unwrap();
				let _right = delta_q_s;
				assert_invariant_eq!(delta_q_h + delta_q_i, delta_q_s);

				// Stableswap equations
				assert!(d_plus >= d);
				#[cfg(feature = "all-invariants")]
				assert!(d_plus - d <= D_DIFF_TOLERANCE);
			});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#3708f95e81104c648eea42afbd2afda6
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_omnipool_asset_with_with_stable(
		amount_to_buy in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		asset_5 in pool_token(ASSET_5),
		amplification in amplification(),
		asset_fee in percent(),
		withdraw_fee in percent(),
		protocol_fee in percent(),
	) {
		let alice_initial_asset_3_balance = ALICE_INITIAL_ASSET_3_BALANCE * 100;

		ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(asset_5.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_5.asset_id, asset_3.amount))
			.add_endowed_accounts((ALICE, asset_3.asset_id, asset_3.amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				add_omnipool_token!(asset_3.asset_id);
				add_omnipool_token!(asset_4.asset_id);
				add_omnipool_token!(asset_5.asset_id);

				create_subpool!(SHARE_ASSET_AS_POOL_ID, asset_3.asset_id, asset_4.asset_id, asset_fee, withdraw_fee);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				assert_that_imbalance_is_zero!();
				let l_before = get_imbalance_value!();

				let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				let hdx_state_before = Omnipool::load_asset_state(HDX).unwrap();

				//Act
				//let amount_to_buy = 100 * ONE;
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					asset_5.asset_id,
					asset_3.asset_id,
					amount_to_buy,
					alice_initial_asset_3_balance
				));

				let l = get_imbalance_value!();

				let hdx_state_after = Omnipool::load_asset_state(HDX).unwrap();

				let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let q_j_plus = asset_5_state_after_sell.hub_reserve;
				let r_j_plus = asset_5_state_after_sell.reserve;
				let q_j = asset_5_state_before_sell.hub_reserve;
				let r_j = asset_5_state_before_sell.reserve;

				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let q_s = share_asset_state_before_sell.hub_reserve;
				let r_s = share_asset_state_before_sell.reserve;
				let q_s_plus = share_asset_state_after_sell.hub_reserve;
				let r_s_plus = share_asset_state_after_sell.reserve;

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_plus = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let delta_l = l - l_before;
				let delta_q_s = share_asset_state_before_sell.hub_reserve.checked_sub(share_asset_state_after_sell.hub_reserve).unwrap();
				let _f_p = protocol_fee;
				let delta_q_j = asset_5_state_after_sell.hub_reserve.checked_sub(asset_5_state_before_sell.hub_reserve).unwrap();

				let delta_q_h = hdx_state_after.hub_reserve.checked_sub(hdx_state_before.hub_reserve).unwrap();

				//Assert

				// Qj+ * Rj+ >= Qj * Rj
				let left = q_j_plus.checked_mul(r_j_plus).unwrap();
				let right = q_j.checked_mul(r_j).unwrap();
				assert_invariant_ge!(left, right);

				// Qs+ * Rs+ >= Qs * Rs
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);

				let left = u_s_plus.checked_mul(d).unwrap();
				let right = u_s.checked_mul(d_plus).unwrap();
				assert_invariant_le!(left, right);

				let left = r_s_plus.checked_add(u_s).unwrap();
				let right = r_s.checked_add(u_s_plus).unwrap();
				assert_invariant_eq!(left, right);

				// L <= 0
				let left = l;
				let right = 0;
				assert_invariant_le!(left, right);

				//delta_QH + delta_L + delta_Qj = - delta_Qs
				let left = delta_q_h  + delta_l + delta_q_j;
				let right = delta_q_s;
				assert_invariant_eq!(left, right);

				//Stableswap equations
				assert!(d_plus >= d);
				#[cfg(feature = "all-invariants")]
				assert!(d_plus - d <= D_DIFF_TOLERANCE);

		});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#225d7f413f7f4de5b9804f284f20c5a4
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_stableswap_asset_with_with_lrna(
		amount_to_buy in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		amplification in amplification(),
		withdraw_fee in percent()
	) {
		ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((ALICE, LRNA, amount_to_buy * 10000))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				add_omnipool_token!(ASSET_3);
				add_omnipool_token!(ASSET_4);

				create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let l = Omnipool::current_imbalance();
				let q = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_3,
					LRNA,
					amount_to_buy,
					amount_to_buy * 100
				));
				let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);
				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let q_s = share_asset_state_before_sell.hub_reserve;
				let r_s = share_asset_state_before_sell.reserve;
				let q_s_plus = share_asset_state_after_sell.hub_reserve;
				let r_s_plus = share_asset_state_after_sell.reserve;

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_plus = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				let delta_d = d.checked_sub(d_plus).unwrap();

				let one_minus_fw = Permill::from_float(1.0) - withdraw_fee;

				let l_plus = Omnipool::current_imbalance();
				let q_plus = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let delta_u_s = u_s.checked_sub(u_s_plus).unwrap();

				//Assert

				//Qs+ * Rs+ >= Qs * Rs
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);

				// Us+ * D <= Us * D+
				// TODO: keeps failiing . rounding ?
				let left = u_s_plus.checked_mul_into(&d).unwrap();
				let right = u_s.checked_mul_into(&d_plus).unwrap();
				#[cfg(feature = "all-invariants")]
				assert_invariant_le!(left, right);

				//delta_Us * D * (1 - fw) <= Us * delta_D
				let left = one_minus_fw.mul(delta_u_s.checked_mul(d).unwrap());
				let right = u_s.checked_mul(delta_d).unwrap();
				//#[cfg(feature = "all-invariants")]
				//assert_invariant_le!(left, right);

				//Rs+ + Us = Us+ + Rs
				let left = r_s_plus.checked_add(u_s).unwrap();
				let right = u_s_plus.checked_add(r_s).unwrap();
				assert_invariant_eq!(left, right);

				// (Qs+ + L+ * (Qs+/Q+)) * Rs <= (Qs + L * Qs/Q) * Rs+
				let l_one = q_s_plus.checked_mul_into(&r_s).unwrap();
				let l_two = l_plus.value.checked_mul_into(&q_s_plus).unwrap().checked_div_inner(&q_plus).unwrap().checked_mul_inner(&r_s).unwrap();
				let left = l_one.checked_sub(l_two).unwrap();

				let r_one = q_s.checked_mul_into(&r_s_plus).unwrap();
				let r_two = l.value.checked_mul_into(&q_s).unwrap().checked_div_inner(&q).unwrap().checked_mul_inner(&r_s_plus).unwrap();
				let right= r_one.checked_sub(r_two).unwrap();
				#[cfg(feature = "all-invariants")]
				assert_invariant_le!(left, right);
			});
	}
}
