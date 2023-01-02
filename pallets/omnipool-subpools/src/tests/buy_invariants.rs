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
				let d_before_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				assert_that_imbalance_is_zero!();

				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					asset_3.asset_id,
					asset_5.asset_id,
					amount_to_buy,
					500000 * ONE
				));

				//Assert
				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);

				// Qi+ * Ri+ >= Qi * Ri
				let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();

				let q_i_plus = asset_5_state_after_sell.hub_reserve;
				let r_i_plus = asset_5_state_after_sell.reserve;
				let q_i = asset_5_state_before_sell.hub_reserve;
				let r_i = asset_5_state_before_sell.reserve;
				let left = q_i_plus.checked_mul(r_i_plus).unwrap();
				let right = q_i.checked_mul(r_i).unwrap();
				assert_invariant_ge!(left, right);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#79f6a3c7e9544978a6c5922f33b7bf52
				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
				let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
				assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);
				let q_s_plus = share_asset_state_after_sell.hub_reserve;
				let r_s_plus = share_asset_state_after_sell.reserve;
				let q_s = share_asset_state_before_sell.hub_reserve;
				let r_s = share_asset_state_before_sell.reserve;
				let left = q_s_plus.checked_mul(r_s_plus).unwrap();
				let right = q_s.checked_mul(r_s).unwrap();
				assert_invariant_ge!(left, right);
			   continue here, remove duplications

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#81917d2af66549ea80c047c1f1d547f2
				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				assert!(share_asset_state_after_sell.reserve * d_before_sell < share_asset_state_before_sell.reserve * d_after_sell);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#c1f01a542f6549f1b4519824959c57fe
				let delta_share_asset_reserve = share_asset_state_before_sell.reserve - share_asset_state_after_sell.reserve;
				let withdraw_fee_complement = Permill::from_float(1.0) - withdraw_fee;
				let left = withdraw_fee_complement.mul(delta_share_asset_reserve * d_before_sell);
				let right = share_asset_state_before_sell.reserve * (d_after_sell - d_before_sell);
				assert!(left < right || left == right);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#ce08b3ff242d4f26a1f4525716cf9cab
				let share_asset_balance_after = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());
				assert_eq!(share_asset_state_after_sell.reserve + share_asset_balance_before, share_asset_state_before_sell.reserve + share_asset_balance_after);

				//https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#c727317fa94548baad28070852996959
				assert_that_imbalance_is_zero!();

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#bc9a8b9c61a440b1b9b48ea7d6fc7cac
				let delta_lrna_of_share_asset = share_asset_state_after_sell.hub_reserve - share_asset_state_before_sell.hub_reserve;
				let delta_q_h =  protocol_fee.mul_floor(delta_lrna_of_share_asset);
				let delta_lrna_of_omnipool_asset = asset_5_state_before_sell.hub_reserve - asset_5_state_after_sell.hub_reserve;
				#[cfg(feature = "all-invariants")]
				assert_eq!(delta_q_h + delta_lrna_of_share_asset, delta_lrna_of_omnipool_asset);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#9dba0a10933646c8963ea1a074cddde1
				assert!(d_after_sell >= d_before_sell);
				#[cfg(feature = "all-invariants")]
				assert!(d_after_sell - d_before_sell <= 10u128); //TODO: once this has been checked by Martin, we need to add it to other buy tests

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
				let d_before_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				assert_that_imbalance_is_zero!();

				//Act
				//let amount_to_buy = 100 * ONE;
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					asset_5.asset_id,
					asset_3.asset_id,
					amount_to_buy,
					alice_initial_asset_3_balance
				));

				//Assert
				let omnipool_account = Omnipool::protocol_account();

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#dc9628676f224f2e8086bccba8b68b1a
				let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let asset_5_reserve_with_hub_before = asset_5_state_before_sell.hub_reserve * asset_5_state_before_sell.reserve;
				let asset_5_reserve_with_hub_after = asset_5_state_after_sell.hub_reserve * asset_5_state_after_sell.reserve;
				assert!(asset_5_reserve_with_hub_after > asset_5_reserve_with_hub_before);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#fcce5412ec844ba9bdf0e3b00b6ba70e
				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
				let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
				assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7f2635b3a67b44eaa4cb95315ae1a83b
				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				#[cfg(feature = "all-invariants")]
				assert!(share_asset_state_after_sell.reserve * d_before_sell < share_asset_state_before_sell.reserve * d_after_sell);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#847cd7908760415b9748f7fa0b1c2234
				let share_asset_balance_after = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());
				assert_eq!(share_asset_state_after_sell.reserve + share_asset_balance_before, share_asset_state_before_sell.reserve + share_asset_balance_after);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#dfe80fdf75074b42bffdf9becd38e0c2
				assert_that_imbalance_is_zero!();

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#2bd2c5b327054019bbaa69d905cdca1b
				let delta_lrna_of_share_asset = share_asset_state_before_sell.hub_reserve - share_asset_state_after_sell.hub_reserve;
				let delta_q_h =  protocol_fee.mul_floor(delta_lrna_of_share_asset);
				let delta_lrna_of_omnipool_asset = asset_5_state_after_sell.hub_reserve - asset_5_state_before_sell.hub_reserve;
				#[cfg(feature = "all-invariants")]
				assert_eq!(delta_q_h  + delta_lrna_of_omnipool_asset, delta_lrna_of_share_asset);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#317792fa75ef4b20b80b7137a427c21c
				assert!(d_after_sell >= d_before_sell);
				#[cfg(feature = "all-invariants")]
				assert!(d_before_sell - d_after_sell <= 10u128);

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
				let d_before_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				let share_asset_balance_before = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());

				let imbalance_before_sell = Omnipool::current_imbalance();
				let omnipool_lrna_balance_before_sell = Tokens::free_balance(LRNA, &Omnipool::protocol_account());


				//Act
				assert_ok!(OmnipoolSubpools::buy(
					Origin::signed(ALICE),
					ASSET_3,
					LRNA,
					amount_to_buy,
					amount_to_buy * 100
				));

				//Assert
				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#fbaa3b124e27422fa3733ed842e43949
				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
				let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
				assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#72f0860d7959451cafec3bfd5ed3163f
				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				#[cfg(feature = "all-invariants")]
				assert!(share_asset_state_after_sell.reserve * d_before_sell < share_asset_state_before_sell.reserve * d_after_sell);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#c775af2e83434b29a6dee4f75c078802
				let delta_share_asset_reserve = share_asset_state_before_sell.reserve - share_asset_state_after_sell.reserve;
				let withdraw_fee_complement = Permill::from_float(1.0) - withdraw_fee;
				let left = withdraw_fee_complement.mul(delta_share_asset_reserve * d_before_sell);
				let right = share_asset_state_before_sell.reserve * (d_before_sell - d_after_sell);
				#[cfg(feature = "all-invariants")]
				assert!(left <= right);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#56141f77dc424c29a93dcd946a39e9d0
				let share_asset_balance_after = Tokens::free_balance(SHARE_ASSET_AS_POOL_ID, &Omnipool::protocol_account());
				assert_eq!(share_asset_state_after_sell.reserve + share_asset_balance_before, share_asset_state_before_sell.reserve + share_asset_balance_after);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#38e19891855e4b24941fe50c7ce9ac5a
				let imbalance_after_sell = Omnipool::current_imbalance();
				let omnipool_lrna_balance_after_sell = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let left = (share_asset_state_after_sell.hub_reserve + (imbalance_after_sell.value * share_asset_state_after_sell.hub_reserve/omnipool_lrna_balance_after_sell)) * share_asset_state_before_sell.reserve;
				let right = (share_asset_state_before_sell.hub_reserve + (imbalance_before_sell.value * share_asset_state_before_sell.hub_reserve/omnipool_lrna_balance_before_sell)) * share_asset_state_after_sell.reserve;
				//TODO: check with Martin
				#[cfg(feature = "all-invariants")]
				assert!(left < right || left == right, "The invariant does not hold, left side: {}, right side: {}",left, right);
			});
	}
}
