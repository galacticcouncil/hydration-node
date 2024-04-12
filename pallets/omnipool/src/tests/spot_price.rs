#![allow(clippy::excessive_precision)]

use super::*;
use frame_support::storage::with_transaction;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::PoolType;
use hydradx_traits::router::TradeExecution;
use pretty_assertions::assert_eq;
use sp_runtime::{Permill, TransactionOutcome};

#[test]
fn compare_spot_price_with_and_without_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.with_asset_fee(Permill::from_percent(1))
		.with_protocol_fee(Permill::from_percent(1))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let asset_a = 100;
			let asset_b = 200;
			let sell_amount = ONE;

			let received = with_transaction::<Balance, DispatchError, _>(|| {
				let balance_before = Tokens::free_balance(asset_b, &LP1);
				assert_ok!(Omnipool::sell(
					RuntimeOrigin::signed(LP1),
					asset_a,
					asset_b,
					sell_amount,
					0
				));
				let balance_after = Tokens::free_balance(asset_b, &LP1);
				let received = balance_after - balance_before;

				TransactionOutcome::Rollback(Ok(received))
			})
			.unwrap();

			//Check spot price without fee
			let spot_price_without_fee = Omnipool::spot_price(asset_a, asset_b).unwrap();
			let calculated_amount_out_without_fee = spot_price_without_fee
				.reciprocal()
				.unwrap()
				.checked_mul_int(sell_amount)
				.unwrap();
			let difference = calculated_amount_out_without_fee - received;
			let relative_difference_without_fee = FixedU128::from_rational(difference, received);
			let tolerated_difference = FixedU128::from_rational(3, 100);
			assert_eq!(
				relative_difference_without_fee,
				FixedU128::from_float(0.021_234_227_801_488_840)
			);
			// The difference of the amount out calculated with spot price should be less than 3%
			assert!(relative_difference_without_fee < tolerated_difference);

			//Check spot price with fee
			let spot_price_with_fee = Omnipool::calculate_spot_price(PoolType::Omnipool, asset_a, asset_b).unwrap();
			let calculated_amount_out_with_fee = spot_price_with_fee
				.reciprocal()
				.unwrap()
				.checked_mul_int(sell_amount)
				.unwrap();
			let difference = calculated_amount_out_with_fee - received;
			let relative_difference_with_fee = FixedU128::from_rational(difference, received);
			let tolerated_difference = FixedU128::from_rational(1, 1000);

			assert_eq_approx!(
				relative_difference_with_fee,
				FixedU128::from_float(0.000911666668239212),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);
			// The difference of the amount out calculated with spot price should be less than 0.1%
			assert!(relative_difference_with_fee < tolerated_difference);

			//Compare teh two price
			assert!(relative_difference_with_fee < relative_difference_without_fee);

			assert!(
				spot_price_with_fee > spot_price_without_fee,
				"Spot price with fee should be smaller than without fee"
			);
		});
}
