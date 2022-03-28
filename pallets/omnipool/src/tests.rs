use crate::mock::*;
use crate::types::SimpleImbalance;
use crate::*;
use frame_support::assert_ok;
use sp_runtime::{FixedPointNumber, FixedU128};

const ONE: Balance = 1_000_000_000_000;

const NATIVE_AMOUNT: Balance = 10_000 * ONE;

#[macro_export]
macro_rules! check_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

#[macro_export]
macro_rules! check_state {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(HubAssetLiquidity::<Test>::get(), $x, "Hub liquidity incorrect");
		assert_eq!(TotalTVL::<Test>::get(), $y, "Total tvl incorrect");
		assert_eq!(HubAssetImbalance::<Test>::get(), $z, "Imbalance incorrect");
	}};
}

fn init_omnipool(dai_amount: Balance, price: FixedU128) {
	assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, price));
	assert_ok!(Omnipool::add_token(
		Origin::root(),
		HDX,
		NATIVE_AMOUNT,
		FixedU128::from(1)
	));

	check_state!(
		price.checked_mul_int(dai_amount).unwrap() + NATIVE_AMOUNT,
		NATIVE_AMOUNT * (dai_amount / price.checked_mul_int(dai_amount).unwrap()) + dai_amount,
		SimpleImbalance::default()
	);
}

#[test]
fn add_stable_asset_works() {
	new_test_ext().execute_with(|| {
		let dai_amount = 100 * ONE;

		assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, FixedU128::from(1)));

		check_state!(dai_amount, dai_amount, SimpleImbalance::default());
	});
}

#[test]
fn add_token_works() {
	new_test_ext().execute_with(|| {
		let dai_amount = 1000 * ONE;
		let price = FixedU128::from_float(0.5);
		init_omnipool(dai_amount, price);

		let token_price = FixedU128::from_float(0.65);

		let token_amount = 2000 * ONE;

		assert_ok!(Omnipool::add_token(Origin::root(), 1_000, token_amount, token_price));

		// Note: using exact values to make sure that it is same as in python's simulations.
		check_state!(
			11_800 * ONE, //token_price.checked_mul_int(token_amount).unwrap() + dai_amount / 2 + NATIVE_AMOUNT,
			23_600 * ONE,
			SimpleImbalance::default()
		);
	});
}

#[test]
fn add_liquidity_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(LP1, 1_000, 1000 * ONE)])
		.build()
		.execute_with(|| {
			let dai_amount = 100 * ONE;
			let price = FixedU128::from(1);
			init_omnipool(dai_amount, price);

			let token_amount = 300 * ONE;

			assert_ok!(Omnipool::add_token(
				Origin::root(),
				1_000,
				token_amount,
				FixedU128::from(1)
			));

			check_state!(
				price.checked_mul_int(token_amount).unwrap() + dai_amount + NATIVE_AMOUNT,
				token_amount + NATIVE_AMOUNT + dai_amount,
				SimpleImbalance::default()
			);

			assert_ok!(Omnipool::add_liquidity(Origin::signed(1), 1_000, token_amount));

			check_state!(
				700 * ONE + NATIVE_AMOUNT,
				600 * ONE + NATIVE_AMOUNT + dai_amount,
				SimpleImbalance::default()
			);

			check_balance!(LP1, 1_000, 700 * ONE)
		});
}
