use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
use hydra_dx_math::stableswap::calculate_d;
use pallet_omnipool::types::AssetReserveState;
use pallet_omnipool::types::SimpleImbalance;
use pallet_omnipool::types::Tradability;
use primitive_types::U256;
use proptest::prelude::*;
use std::ops::Mul;
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
	fn buy_stableswap_asset_with_omnipool_asset(
		amount_to_buy in trade_amount(),
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

				create_subpool!(SHARE_ASSET_AS_POOL_ID, asset_3.asset_id, asset_4.asset_id);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_4.asset_id, asset_4.asset_id], None);

				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				let l_before = get_imbalance_value!();

				assert_that_imbalance_is_zero!();

				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					asset_3.asset_id,
					asset_5.asset_id,
					amount_to_buy,
					500000 * ONE
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
				let d_plus = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				let u_s_plus =  share_asset_state_after_sell.reserve;
				let u_s =  share_asset_state_before_sell.reserve;

				let delta_u_s = share_asset_state_before_sell.reserve - share_asset_state_after_sell.reserve;
				let f_w = withdraw_fee;
				let one_minus_fw = Permill::from_float(1.0) - f_w;
				let delta_d = d_plus - d;

				let delta_l = l - l_before;
				let delta_q_i = share_asset_state_after_sell.hub_reserve.checked_sub(share_asset_state_before_sell.hub_reserve).unwrap();
				let f_p = protocol_fee;
				let delta_q_h =  f_p.mul_floor(delta_q_i);
				let delta_q_s = asset_5_state_before_sell.hub_reserve.checked_sub(asset_5_state_after_sell.hub_reserve).unwrap();

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

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
				let left = delta_q_h.checked_add(delta_q_i).unwrap();
				let right = delta_q_s;
				#[cfg(feature = "all-invariants")]
				assert_invariant_eq!(delta_q_h + delta_q_i, delta_q_s);

				// Stableswap equations
				assert!(d_plus >= d);
				#[cfg(feature = "all-invariants")]
				assert!(d_plus - d <= 10u128); //TODO: once this has been checked by Martin, we need to add it to other buy tests
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
		trade_fee in percent(),
		withdraw_fee in percent(),
		protocol_fee in percent()
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

				create_subpool!(SHARE_ASSET_AS_POOL_ID, asset_3.asset_id, asset_4.asset_id);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				assert_that_imbalance_is_zero!();
				let l_before = get_imbalance_value!();


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
				let u_s = share_asset_state_before_sell.reserve;
				let u_s_plus = share_asset_state_after_sell.reserve;

				let delta_l = l - l_before;
				let delta_q_s = share_asset_state_before_sell.hub_reserve.checked_sub(share_asset_state_after_sell.hub_reserve).unwrap();
				let f_p = protocol_fee;
				let delta_q_h =  f_p.mul_floor(delta_q_s);
				let delta_q_j = asset_5_state_after_sell.hub_reserve.checked_sub(asset_5_state_before_sell.hub_reserve).unwrap();

				//Assert
				let omnipool_account = Omnipool::protocol_account();

				// Qj+ * Rj+ >= Qj * Rj
				let left = q_j_plus.checked_mul(r_j_plus).unwrap();
				let right = q_j.checked_mul(r_j).unwrap();
				assert_invariant_ge!(left, right);

				// Qs+ * Rs+ >= Qs * Rs
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);


				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7f2635b3a67b44eaa4cb95315ae1a83b
				let left = u_s_plus.checked_mul(d).unwrap();
				let right = u_s.checked_mul(d_plus).unwrap();
				#[cfg(feature = "all-invariants")]
				assert_invariant_le!(left, right);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#847cd7908760415b9748f7fa0b1c2234
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
				#[cfg(feature = "all-invariants")]
				assert_invariant_eq!(left, right);

				//Stableswap equations
				assert!(d_plus >= d);
				#[cfg(feature = "all-invariants")]
				assert!(d - d_plus <= 10u128);

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
		trade_fee in percent(),
		withdraw_fee in percent(),
		protocol_fee in percent()
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
				let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
				let omnipool_account = Omnipool::protocol_account();

				add_omnipool_token!(ASSET_3);
				add_omnipool_token!(ASSET_4);

				create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				let l = Omnipool::current_imbalance();
				let q = Tokens::free_balance(LRNA, &Omnipool::protocol_account());


				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_3,
					LRNA,
					amount_to_buy,
					amount_to_buy * 100
				));

				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let q_s = share_asset_state_before_sell.hub_reserve;
				let r_s = share_asset_state_before_sell.reserve;
				let q_s_plus = share_asset_state_after_sell.hub_reserve;
				let r_s_plus = share_asset_state_after_sell.reserve;

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_plus = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				let u_s_plus = share_asset_state_after_sell.reserve;
				let u_s = share_asset_state_before_sell.reserve;

				let delta_u_s_plus = share_asset_state_before_sell.reserve - share_asset_state_after_sell.reserve;
				let f_w = withdraw_fee;
				let one_minus_fw = Permill::from_float(1.0) - withdraw_fee;
				let u_s =share_asset_state_before_sell.reserve;
				let delta_d = d - d_plus;

				let l_plus = Omnipool::current_imbalance();
				let q_plus = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				//Assert

				//Qs+ * Rs+ >= Qs * Rs
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);

				// Us+ * D <= Us * D+
				let left = u_s_plus.checked_mul(d).unwrap();
				let right = u_s.checked_mul(d_plus).unwrap();
				#[cfg(feature = "all-invariants")]
				assert_invariant_le!(left, right);

				//delta_Us * D * (1 - fw) <= Us * delta_D
				let left = one_minus_fw.mul(delta_u_s_plus.checked_mul(d).unwrap());
				let right = u_s.checked_mul(delta_d).unwrap();
				#[cfg(feature = "all-invariants")]
				 assert_invariant_le!(left, right);

				//Rs+ + Us = Us+ + Rs
				let left = r_s_plus.checked_add(u_s).unwrap();
				let right = u_s_plus.checked_add(r_s).unwrap();
				 assert_invariant_eq!(left, right);

				// (Qs+ + L+ * (Qs+/Q+)) * Rs <= (Qs + L * Qs/Q) * Rs+
				let left_inner_part = l_plus.value.checked_mul(q_s_plus.checked_div(q_plus).unwrap()).unwrap();
				let left = match l_plus.negative {
					false =>  (q_s_plus.checked_add(left_inner_part).unwrap()).checked_mul(r_s).unwrap(),
					true =>  (q_s_plus.checked_sub(l_plus.value.checked_mul(q_s_plus.checked_div(q_plus).unwrap()).unwrap()).unwrap()).checked_mul(r_s).unwrap(),
				};

				let right_inner_part = l.value.checked_mul(q_s.checked_div(q).unwrap()).unwrap();
				let right = match l.negative {
					false => (q_s.checked_add(right_inner_part).unwrap()).checked_mul(r_s_plus).unwrap(),
					true => (q_s.checked_sub(right_inner_part).unwrap()).checked_mul(r_s_plus).unwrap(),
				};

				#[cfg(feature = "all-invariants")]
				assert_invariant_le!(left, right);

			});
	}
}
