use super::*;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

#[test]
fn imbalance_should_update_correctly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_protocol_fee(Permill::from_percent(1))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 400000000000000));

			assert_pool_state!(
				13360000000000000,
				26720000000000000,
				SimpleImbalance {
					value: 0,
					negative: true
				}
			);

			let old_imbalance = HubAssetImbalance::<Test>::get();
			assert_ok!(Omnipool::sell(
				Origin::signed(LP3),
				1,
				200,
				50000000000000,
				10000000000000
			));

			let updated_imbalance = HubAssetImbalance::<Test>::get();

			// After lrna is sold to pool, imbalance should increase (more negative)
			assert!(updated_imbalance.value > old_imbalance.value);

			let q = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			let old_imbalance = HubAssetImbalance::<Test>::get();
			assert_ok!(Omnipool::sell(Origin::signed(LP3), 200, 100, 1000000000000, 1,));
			let updated_imbalance = HubAssetImbalance::<Test>::get();
			let q_plus = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			// After non-lrna trade - sell, imbalance should decrease ( less negative )
			assert!(updated_imbalance.value < old_imbalance.value);
			assert_eq!(
				q.checked_sub(old_imbalance.value).unwrap(),
				q_plus.checked_sub(updated_imbalance.value).unwrap()
			);

			let position_id = <NextPositionId<Test>>::get();
			let old_imbalance = HubAssetImbalance::<Test>::get();
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 400000000000000));
			let updated_imbalance = HubAssetImbalance::<Test>::get();

			// After add additional liquidity , imbalance should increase ( more negative )
			assert!(updated_imbalance.value > old_imbalance.value);

			let position = Positions::<Test>::get(position_id).unwrap();
			let old_imbalance = HubAssetImbalance::<Test>::get();
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP2),
				position_id,
				position.shares
			));
			let updated_imbalance = HubAssetImbalance::<Test>::get();

			// After remove additional liquidity , imbalance should decrease( less negative )
			assert!(updated_imbalance.value < old_imbalance.value);

			let q = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			let old_imbalance = HubAssetImbalance::<Test>::get();
			assert_ok!(Omnipool::buy(Origin::signed(LP3), 200, 100, 1000000000000, u128::MAX,));
			let updated_imbalance = HubAssetImbalance::<Test>::get();
			let q_plus = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			// After non-lrna trade - buy, imbalance should decrease ( less negative )
			assert!(updated_imbalance.value < old_imbalance.value);
			assert_eq!(
				q.checked_sub(old_imbalance.value).unwrap(),
				q_plus.checked_sub(updated_imbalance.value).unwrap()
			);
		});
}
