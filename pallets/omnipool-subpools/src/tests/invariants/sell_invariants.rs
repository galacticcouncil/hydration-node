use super::*;
use crate::*;
use proptest::prelude::*;
use sp_std::ops::Mul;

use hydra_dx_math::stableswap::calculate_d;
use pallet_omnipool::types::SimpleImbalance;

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#697fb7cb7bb8464cafcab36089cf18e1
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_stableswap_asset_for_omnipool_asset(
		sell_amount in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		asset_5 in pool_token(ASSET_5),
		amplification in amplification(),
		trade_fee in percent(),
		withdraw_fee in percent(),
		protocol_fee in percent()
	) {
		ExtBuilder::default()
		.with_registered_asset(asset_3.asset_id)
		.with_registered_asset(asset_4.asset_id)
		.with_registered_asset(asset_5.asset_id)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_5.asset_id, asset_5.amount))
		.add_endowed_accounts((ALICE, ASSET_3, sell_amount))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_protocol_fee(protocol_fee)
		.build()
		.execute_with(|| {
			add_omnipool_token!(asset_3.asset_id);
			add_omnipool_token!(asset_4.asset_id);
			add_omnipool_token!(asset_5.asset_id);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				asset_3.asset_id,
				asset_4.asset_id,
				Permill::from_percent(50),
				amplification,
				trade_fee,
				withdraw_fee,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

			let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
			let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

			assert_that_imbalance_is_zero!();

			let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

			let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

			//Act
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				asset_3.asset_id,
				asset_5.asset_id,
				sell_amount,
				0
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

			let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
			let q_i = asset_5_state_before_sell.hub_reserve;
			let r_i = asset_5_state_before_sell.reserve;
			let q_i_plus = asset_5_state_after_sell.hub_reserve;
			let r_i_plus = asset_5_state_after_sell.reserve;

			let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
			let q_s = share_asset_state_before_sell.hub_reserve;
			let r_s = share_asset_state_before_sell.reserve;
			let q_s_plus = share_asset_state_after_sell.hub_reserve;
			let r_s_plus = share_asset_state_after_sell.reserve;

			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d_plus = calculate_d::<64u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

			let r_s = share_asset_state_before_sell.reserve;
			let r_s_plus = share_asset_state_after_sell.reserve;
			let u_s_plus  = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

			let l = get_imbalance_value!();

			let delta_q_s = share_asset_state_before_sell.hub_reserve - share_asset_state_after_sell.hub_reserve;
			let f_p = protocol_fee;
			let delta_q_h =  f_p.mul_floor(delta_q_s);
			let delta_q_j = asset_5_state_after_sell.hub_reserve - asset_5_state_before_sell.hub_reserve;
			let delta_l = 0; // Because no LRNA is sold or bought

			//Assert

			// Qi+ * Ri+ >= Qi * Ri
			let left = q_i_plus.checked_mul(r_i_plus).unwrap();
			let right = q_i.checked_mul(r_i).unwrap();
			assert_invariant_ge!(left, right);

			// Qs+ * Rs+ <= Qs * Rs
			let left = q_s_plus.checked_mul(r_s_plus).unwrap();
			let right = q_s.checked_mul(r_s).unwrap();
			assert_invariant_ge!(left, right);

			// Us+ * D <= Us * D+
			let left = u_s_plus.checked_mul(d).unwrap();
			let right = u_s.checked_mul(d_plus).unwrap();
			assert_invariant_le!(left, right);

			//Rs+ + Us = Us+ + Rs
			let left = r_s_plus.checked_add(u_s).unwrap();
			let right = u_s_plus.checked_add(r_s).unwrap();
			assert_invariant_eq!(left, right);

			// L <= 0
			let left = l;
			let right = 0;
			assert_invariant_le!(left, right);

			//delta_QH + delta_L + delta_Qj = - delta_Qs
			let left = delta_q_h.checked_add(delta_l).unwrap().checked_add(delta_q_j).unwrap();
			let right = delta_q_s;
			assert_invariant_eq!(left, right);

			//Stableswap equation holds
			assert!(d_plus >= d);
			#[cfg(feature = "all-invariants")]
			assert!(d_plus - d <= D_DIFF_TOLERANCE);
		});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#363a38037c2d42d8977107df2439d274
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_omnipool_asset_for_stableswap_asset(
		sell_amount in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		asset_5 in pool_token(ASSET_5),
		amplification in amplification(),
		trade_fee in percent(),
		withdraw_fee in percent(),
		protocol_fee in percent(),
	) {
		ExtBuilder::default()
			.with_registered_asset(asset_3.asset_id)
			.with_registered_asset(asset_4.asset_id)
			.with_registered_asset(asset_5.asset_id)
			.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
			.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
			.add_endowed_accounts((Omnipool::protocol_account(), asset_5.asset_id, asset_5.amount))
			.add_endowed_accounts((ALICE, ASSET_5, sell_amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.with_protocol_fee(protocol_fee)
			.build()
			.execute_with(|| {
				add_omnipool_token!(asset_3.asset_id);
				add_omnipool_token!(asset_4.asset_id);
				add_omnipool_token!(asset_5.asset_id);

				assert_ok!(OmnipoolSubpools::create_subpool(
					Origin::root(),
					SHARE_ASSET_AS_POOL_ID,
					asset_3.asset_id,
					asset_4.asset_id,
					Permill::from_percent(50),
					amplification,
					trade_fee,
					withdraw_fee,
				));

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);
				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				assert_that_imbalance_is_zero!();

				let hdx_state_before= Omnipool::load_asset_state(HDX).unwrap();

				let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

				//Act
				assert_ok!(OmnipoolSubpools::sell(
					Origin::signed(ALICE),
					asset_5.asset_id,
					asset_3.asset_id,
					sell_amount,
					0
				));

				let l = get_imbalance_value!();

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

				let u_s_plus  = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);
				let d_plus = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let delta_u_s = u_s.checked_sub(u_s_plus).unwrap();
				let f_w = withdraw_fee;
				let one_minus_fw = Permill::from_float(1.0) - f_w;
				let u_s = share_asset_state_before_sell.shares;
				let delta_d = d.checked_sub(d_plus).unwrap();

				let delta_q_s = share_asset_state_after_sell.hub_reserve.checked_sub(share_asset_state_before_sell.hub_reserve).unwrap();
				let delta_q_h =  protocol_fee.mul_floor(delta_q_s) - l;
				let delta_q_i = asset_5_state_before_sell.hub_reserve.checked_sub(asset_5_state_after_sell.hub_reserve).unwrap();
				let hdx_state_after = Omnipool::load_asset_state(HDX).unwrap();
				let hub_hdx_diff = hdx_state_after.hub_reserve.checked_sub(hdx_state_before.hub_reserve).unwrap();

				//Assert

				// Qj+ * Rj+ >= Qj * Rj
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

				// Rs+ + Us = Us+ + Rs
				let left = r_s_plus.checked_add(u_s).unwrap();
				let right = u_s_plus.checked_add(r_s).unwrap();
				assert_invariant_eq!(left, right);

				// delta_Us * D * (1 - fw) <= Us * delta_D
				// TODO: should be flipped ?
				let left = one_minus_fw.mul(delta_u_s.checked_mul(d).unwrap());
				let right = share_asset_state_before_sell.shares.checked_mul(delta_d).unwrap();
				assert_invariant_le!(right, left);

				// L <= 0
				let left = l;
				let right = 0;
				assert_invariant_le!(left, right);

				//delta_QH + delta_L + delta_Qi = -delta_Qs
				let left = delta_q_i.checked_sub(hub_hdx_diff).unwrap();
				let right = delta_q_s;
				assert_invariant_eq!(left, right);

				});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7a2f7db50bf54b41a96c02b633f24b94
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_lrna_for_stableswap_asset(
		sell_amount in trade_amount(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
		amplification in amplification(),
		trade_fee in percent(),
		withdraw_fee in percent()
	) {
		let trade_Fee = Permill::zero();
		ExtBuilder::default()
		.with_registered_asset(asset_3.asset_id)
		.with_registered_asset(asset_4.asset_id)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
		.add_endowed_accounts((ALICE, LRNA, sell_amount))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(asset_3.asset_id);
			add_omnipool_token!(asset_4.asset_id);

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				asset_3.asset_id,
				asset_4.asset_id,
				Permill::from_percent(50),
				amplification,
				trade_fee,
				withdraw_fee,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);
			let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d_before_sell = calculate_d::<64u8>(&[asset_a_reserve, asset_b_reserve], amplification.into()).unwrap();

			let q = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			let imbalance_before_sell = Omnipool::current_imbalance();

			let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

			//Act
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				LRNA,
				asset_3.asset_id,
				sell_amount,
				0
			));

			let asset_a_reserve_after_sell = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve_after_sell = Tokens::free_balance(asset_4.asset_id, &pool_account);

			let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

			let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

			let r_s = share_asset_state_before_sell.reserve;
			let r_s_plus = share_asset_state_after_sell.reserve;

			let q_s = share_asset_state_before_sell.hub_reserve;
			let q_s_plus = share_asset_state_after_sell.hub_reserve;

			let q_plus = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			let delta_u_s = u_s.checked_sub(u_s_plus).unwrap();

			let d_after_sell = calculate_d::<64u8>(
				&[asset_a_reserve_after_sell, asset_b_reserve_after_sell],
				amplification.into(),
			)
			.unwrap();

			let delta_d = d_before_sell.checked_sub(d_after_sell).unwrap();

			let imbalance_after_sell = Omnipool::current_imbalance();

			//Assert

			// Qs+ * Rs+ > Qs * Rs
			let left = q_s_plus.checked_mul_into(&r_s_plus).unwrap();
			let right = q_s.checked_mul_into(&r_s).unwrap();
			assert_invariant_le!(right, left);

			// Rs+ + Us = Us+ + Rs
			let left = r_s_plus.checked_add(u_s).unwrap();
			let right = u_s_plus.checked_add(r_s).unwrap();
			assert_invariant_eq!(left, right);

			// Us+ * D <= Us * D+
			let left = u_s_plus.checked_mul_into(&d_before_sell).unwrap();
			let right = u_s.checked_mul_into(&d_after_sell).unwrap();
			assert_invariant_le!(left, right);

			// delta_Us * D * ( 1 - Fw ) >= Us * delta_D
			// TODO: should be flipped or not ?
			let left = delta_u_s.checked_mul_into(&d_before_sell).unwrap();
			let right = u_s.checked_mul_into(&delta_d).unwrap();
			assert_invariant_le!(right, left);

			//(Qs+ + L+ Qs+ / Q+ ) * Rs <= (Qs + L Qs/Q) * Rs+
			let left_one = q_s_plus.checked_mul_into(&r_s).unwrap();
			let left_two = left_one
				.checked_mul_inner(&imbalance_after_sell.value)
				.unwrap()
				.checked_div_inner(&q_plus)
				.unwrap();
			let left = left_one.checked_sub(left_two).unwrap();

			let right_one = q_s.checked_mul_into(&r_s_plus).unwrap();
			let right_two = right_one
				.checked_mul_inner(&imbalance_before_sell.value)
				.unwrap()
				.checked_div_inner(&q)
				.unwrap();
			let right = right_one.checked_sub(right_two).unwrap();

			assert_invariant_le!(left, right);
		});
	}
}
