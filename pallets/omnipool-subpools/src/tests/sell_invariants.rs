use super::*;
use crate::types::Balance;
use crate::*;
use frame_benchmarking::Zero;
use frame_support::assert_noop;
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

			let pool_account = AccountIdConstructor::from_assets(&vec![asset_4.asset_id, asset_4.asset_id], None);

			let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
			let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d_before_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

			assert_that_imbalance_is_zero!();

			//Act
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				asset_3.asset_id,
				asset_5.asset_id,
				sell_amount,
				0
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);
			let omnipool_account = Omnipool::protocol_account();

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#22db4d7d9fbc4d6fbb718221c16e1af0
			let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
			let asset_5_reserve_with_hub_before = asset_5_state_before_sell.hub_reserve * asset_5_state_before_sell.reserve;
			let asset_5_reserve_with_hub_after = asset_5_state_after_sell.hub_reserve * asset_5_state_after_sell.reserve;
			assert!(asset_5_reserve_with_hub_after > asset_5_reserve_with_hub_before);

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#ed334c898a19431caafcba1f395b2d38
			let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
			let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
			let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
			assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#4eaedf4ad7184c90a7dd1ac3f2470cfb
			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
			assert!(share_asset_state_after_sell.reserve * d_before_sell < share_asset_state_before_sell.reserve * d_after_sell);

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#40c55e720b8e4f9081ff344f3b7cc5c7
			let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
			let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
			let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
			assert!(share_asset_state_after_sell.reserve * d_before_sell < share_asset_state_before_sell.reserve * d_after_sell);

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#5381ad0b29464bd8bd7c8596a6d98861
			assert_that_imbalance_is_zero!();

			//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7206aa9e6b2944fe91f5eb79534149f1
			let delta_lrna_of_share_asset = share_asset_state_before_sell.hub_reserve -share_asset_state_after_sell.hub_reserve;
			let delta_Q_H =  protocol_fee.mul_floor(delta_lrna_of_share_asset);
			let delta_lrna_of_omnipool_asset = asset_5_state_after_sell.hub_reserve - asset_5_state_before_sell.hub_reserve;
			assert_eq!(delta_Q_H + delta_lrna_of_omnipool_asset, delta_lrna_of_share_asset);

			//TODO: missing prop assertions, can be added after we get answer from Colin
			// https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#40c55e720b8e4f9081ff344f3b7cc5c7
			// https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#feb5cf8ec4ac4e50a7e219620653f3c7


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
			.add_endowed_accounts((ALICE, ASSET_5, sell_amount))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
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

				let pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_5.asset_id], None);
				let omnipool_account = Omnipool::protocol_account();
				let asset_5_state_before_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let share_asset_state_before_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_before_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();

				assert_that_imbalance_is_zero!();

				//Act
				assert_ok!(OmnipoolSubpools::sell(
					Origin::signed(ALICE),
					asset_5.asset_id,
					asset_3.asset_id,
					sell_amount,
					0
				));

				//Assert
				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#ea5bf14fc72c4681a946039d3f81a21b
				let asset_5_state_after_sell = Omnipool::load_asset_state(asset_5.asset_id).unwrap();
				let asset_5_reserve_with_hub_before = asset_5_state_before_sell.hub_reserve * asset_5_state_before_sell.reserve;
				let asset_5_reserve_with_hub_after = asset_5_state_after_sell.hub_reserve * asset_5_state_after_sell.reserve;
				assert!(asset_5_reserve_with_hub_after > asset_5_reserve_with_hub_before);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#bd060472cd4a42a980ced9b96dbab6e7
				let share_asset_state_after_sell = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
				let share_reserve_with_hub_before = share_asset_state_before_sell.hub_reserve * share_asset_state_before_sell.reserve;
				let share_reserve_with_hub_after = share_asset_state_after_sell.hub_reserve * share_asset_state_after_sell.reserve;
				assert!(share_reserve_with_hub_after > share_reserve_with_hub_before);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#dcc487221d2545fab842b140039edd6f
				let asset_a_reserve = Tokens::free_balance(asset_3.asset_id, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_4.asset_id, &pool_account);
				let d_after_sell = calculate_d::<128u8>(&[asset_a_reserve,asset_b_reserve], amplification.into()).unwrap();
				assert_eq!(share_asset_state_after_sell.reserve * d_before_sell,share_asset_state_before_sell.reserve * d_after_sell);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#f8f0ccafd36541878551e538a44e2725
				let delta_share_asset_reserve = share_asset_state_before_sell.reserve - share_asset_state_after_sell.reserve;
				let protocol_fee_complement = Permill::from_percent(100) - protocol_fee;
				let left = protocol_fee_complement.mul(delta_share_asset_reserve * d_before_sell);
				let right = share_asset_state_before_sell.reserve * (d_before_sell - d_after_sell);
				assert_eq!(left, right);

				//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#e02e09c412634e58b29990c0a8eaf80b
				assert_that_imbalance_is_zero!();

			   //Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#a9af611bb17e4036b36208d5cf8cbe18
				let delta_lrna_of_share_asset = share_asset_state_after_sell.hub_reserve - share_asset_state_before_sell.hub_reserve;
				let delta_q_h =  protocol_fee.mul_floor(delta_lrna_of_share_asset);
				let delta_lrna_of_omnipool_asset = asset_5_state_before_sell.hub_reserve - asset_5_state_after_sell.hub_reserve;
				assert_eq!(delta_q_h + delta_lrna_of_omnipool_asset, delta_lrna_of_share_asset);

				});
	}
}

proptest! {
	//Spec: https://www.notion.so/Trade-between-stableswap-asset-and-Omnipool-asset-6e43aeab211d4b4098659aff05c8b729#7a2f7db50bf54b41a96c02b633f24b94
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_lrna_for_stableswap_asset(sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
	) {
		ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, OMNIPOOL_INITIAL_ASSET_3_BALANCE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, OMNIPOOL_INITIAL_ASSET_4_BALANCE))
		.add_endowed_accounts((ALICE, LRNA, ALICE_INITIAL_LRNA_BALANCE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			let initial_lrna_balance_in_omnipool = 15050000000000000;
			assert_balance!(omnipool_account, LRNA, initial_lrna_balance_in_omnipool);

			//Act
			let amount_to_sell = 100 * ONE;
			assert_ok!(OmnipoolSubpools::sell(
				Origin::signed(ALICE),
				LRNA,
				ASSET_3,
				amount_to_sell,
				0
			));

			//Assert


				});
	}
}

//Q - LRNA
//R - asset in pool - shares of underlying stableswap subpool
//S - shares in pool
//Us - sum of shares held by omnipool (Rs) + those held directly by LPs so => Us = Rs + SUMMA(rs)
