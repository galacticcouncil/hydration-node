use super::*;
use proptest::prelude::*;

proptest! {
	//Spec: https://www.notion.so/Add-Liquidity-to-stableswap-subpool-d3983e19dd7c4de9b284c74c317be02c#da9e063badf5428bbce53a798df14e48
	#![proptest_config(ProptestConfig::with_cases(1))]
	#[test]
	fn add_liquidity_invariants(
		new_liquidity_amount in asset_reserve(),
		asset_3 in pool_token(ASSET_3),
		asset_4 in pool_token(ASSET_4),
	) {
			ExtBuilder::default()
				.with_registered_asset(asset_3.asset_id)
				.with_registered_asset(asset_4.asset_id)
				.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
				.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
				.add_endowed_accounts((Omnipool::protocol_account(), asset_3.asset_id, asset_3.amount))
				.add_endowed_accounts((Omnipool::protocol_account(), asset_4.asset_id, asset_4.amount))
				.add_endowed_accounts((ALICE, ASSET_3, new_liquidity_amount))
				.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
				.build()
				.execute_with(|| {
					assert_ok!(Omnipool::add_token(Origin::root(), asset_3.asset_id, FixedU128::from_float(0.65),Permill::from_percent(100),LP1));
					assert_ok!(Omnipool::add_token(Origin::root(), asset_4.asset_id, FixedU128::from_float(0.65),Permill::from_percent(100),LP1));

					create_subpool!(SHARE_ASSET_AS_POOL_ID, asset_3.asset_id, asset_4.asset_id, 100u32);

					let _pool_account = AccountIdConstructor::from_assets(&vec![asset_3.asset_id, asset_4.asset_id], None);
					let _omnipool_account = Omnipool::protocol_account();

					let share_state = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();

					let r_s = share_state.reserve;
					let u_s = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);

					let subpool = Stableswap::get_pool(SHARE_ASSET_AS_POOL_ID).unwrap();
					let reserve_total: Balance = subpool.balances::<Test>().into_iter().sum();

					let _position_id: u32 = Omnipool::next_position_id();
					assert_ok!(OmnipoolSubpools::add_liquidity(
						Origin::signed(ALICE),
						ASSET_3,
						new_liquidity_amount
					));

					let share_state_plus = Omnipool::load_asset_state(SHARE_ASSET_AS_POOL_ID).unwrap();
					let r_s_plus = share_state_plus.reserve;

					let u_s_plus = Tokens::total_issuance(SHARE_ASSET_AS_POOL_ID);
					let subpool_plus = Stableswap::get_pool(SHARE_ASSET_AS_POOL_ID).unwrap();

					let reserve_total_plus: Balance = subpool_plus.balances::<Test>().into_iter().sum();

					// R_s+ - U_s+ = R_s - U;
					let left = r_s_plus.checked_sub(u_s_plus).unwrap();
					let right = r_s.checked_sub(u_s).unwrap();
					assert_eq!(left, right);

					assert_eq!(reserve_total_plus, reserve_total + new_liquidity_amount);
			});
	}
}
