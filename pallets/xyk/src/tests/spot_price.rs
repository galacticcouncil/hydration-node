use super::mock::*;
use crate::types::{AssetPair, Price};
use crate::XYKSpotPrice;
use crate::*;
use frame_support::assert_ok;
use frame_support::dispatch::RawOrigin;
use frame_support::storage::with_transaction;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::TradeExecution;
use hydradx_traits::router::{PoolType, TradeType};
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::TransactionOutcome;
#[test]
fn spot_price_provider_should_return_correct_price_when_pool_exists() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial = 99_000_000_000_000u128;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial), (ALICE, asset_b, initial)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial,
				asset_b,
				39_600_000_000_000
			));

			let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

			assert_eq!(price, Some(Price::from_float(2.5))); // 99_000 / 39_600 = 2.5
		});
}

#[test]
fn spot_price_provider_should_return_none_when_pool_does_not_exist() {
	let asset_a = ACA;
	let asset_b = DOT;

	ExtBuilder::default().build().execute_with(|| {
		let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

		assert_eq!(price, None);
	});
}

#[test]
fn spot_price_provider_should_return_none_when_asset_reserve_is_zero() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial = 99_000_000_000_000u128;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial), (ALICE, asset_b, initial)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial,
				asset_b,
				39_600_000_000_000
			));

			let pool_account = XYK::get_pair_id(AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			});

			// Force the pool balance to be zero in this test
			assert_ok!(Currency::set_balance(
				RawOrigin::Root.into(),
				pool_account,
				asset_b,
				0u128,
				0u128
			));

			let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

			assert_eq!(price, None);
		});
}

#[test]
fn compare_sell_spot_price_with_and_without_fee() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial_a = 1000 * ONE;
	let initial_b = 500 * ONE;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial_a * 2), (ALICE, asset_b, initial_b)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial_a,
				asset_b,
				initial_b
			));

			let spot_price_without_fee = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b).unwrap();

			let sell_amount = ONE;
			let received = with_transaction::<Balance, DispatchError, _>(|| {
				assert_eq!(Currency::free_balance(asset_b, &ALICE), 0);
				assert_ok!(XYK::sell(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					asset_b,
					sell_amount,
					0,
					false,
				));
				let received = Currency::free_balance(asset_b, &ALICE);
				assert_eq!(received, 498501498502);
				TransactionOutcome::Rollback(Ok(received))
			})
			.unwrap();

			//Check spot price without fee
			let calculated_amount_out_without_fee = spot_price_without_fee
				.reciprocal()
				.unwrap()
				.checked_mul_int(sell_amount)
				.unwrap();
			let difference = calculated_amount_out_without_fee - received;
			let relative_difference_without_fee = FixedU128::from_rational(difference, received);
			let tolerated_difference = FixedU128::from_rational(1, 100);
			// The difference of the amount out calculated with spot price should be less than 1%
			assert_eq!(
				relative_difference_without_fee,
				FixedU128::from_float(0.003006012023039060)
			);
			assert!(relative_difference_without_fee < tolerated_difference);

			//Check spot price with fee
			let spot_price_with_fee =
				XYK::calculate_spot_price(PoolType::XYK, TradeType::Sell, asset_a, asset_b).unwrap();
			let calculated_amount_out_with_fee = spot_price_with_fee
				.reciprocal()
				.unwrap()
				.checked_mul_int(sell_amount)
				.unwrap();
			let difference = calculated_amount_out_with_fee - received;
			let relative_difference_with_fee = FixedU128::from_rational(difference, received);
			let tolerated_difference = FixedU128::from_rational(1, 100);

			assert_eq_approx!(
				relative_difference_with_fee,
				FixedU128::from_float(0.000999999998992982),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);
			assert!(relative_difference_with_fee < tolerated_difference);

			//Compare the two
			assert!(relative_difference_with_fee < relative_difference_without_fee);

			assert!(
				spot_price_with_fee > spot_price_without_fee,
				"Spot price with fee should be smaller than without fee"
			);
		});
}

#[test]
fn compare_buy_spot_price_with_and_without_fee() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial_a = 1000 * ONE;
	let initial_b = 500 * ONE;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial_a * 2), (ALICE, asset_b, initial_b)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial_a,
				asset_b,
				initial_b
			));

			let buy_amount = ONE;
			let spent = with_transaction::<Balance, DispatchError, _>(|| {
				assert_eq!(Currency::free_balance(asset_b, &ALICE), 0);
				let init_asset_a_balance = Currency::free_balance(asset_a, &ALICE);

				assert_ok!(XYK::buy(
					RuntimeOrigin::signed(ALICE),
					asset_b,
					asset_a,
					buy_amount,
					Balance::MAX,
					false,
				));
				let spent_amount = init_asset_a_balance - Currency::free_balance(asset_a, &ALICE);
				assert_eq!(spent_amount, 2008016032065);
				TransactionOutcome::Rollback(Ok(spent_amount))
			})
			.unwrap();

			//Check spot price without fee
			let spot_price_without_fee = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b).unwrap();
			let calculated_amount_in_without_fee = spot_price_without_fee.checked_mul_int(buy_amount).unwrap();
			let difference = spent - calculated_amount_in_without_fee;
			let relative_difference_without_fee = FixedU128::from_rational(difference, spent);
			let tolerated_difference = FixedU128::from_rational(1, 100);
			// The difference of the amount out calculated with spot price should be less than 1%
			assert_eq!(
				relative_difference_without_fee,
				FixedU128::from_float(0.003992015968496271)
			);
			assert!(relative_difference_without_fee < tolerated_difference);

			//Check spot price with fee
			let spot_price_with_fee =
				XYK::calculate_spot_price(PoolType::XYK, TradeType::Buy, asset_a, asset_b).unwrap();
			let calculated_amount_out_with_fee = spot_price_with_fee.checked_mul_int(buy_amount).unwrap();
			let difference = spent - calculated_amount_out_with_fee;
			let relative_difference_with_fee = FixedU128::from_rational(difference, spent);
			let tolerated_difference = FixedU128::from_rational(1, 100);

			assert_eq_approx!(
				relative_difference_with_fee,
				FixedU128::from_float(0.002000000000433263),
				FixedU128::from((2, (ONE / 10_000))),
				"the relative difference is not as expected"
			);
			assert!(relative_difference_with_fee < tolerated_difference);

			//Compare the two
			assert!(relative_difference_with_fee < relative_difference_without_fee);

			assert!(
				spot_price_with_fee > spot_price_without_fee,
				"Spot price with fee should be smaller than without fee"
			);
		});
}
