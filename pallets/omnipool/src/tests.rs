use crate::mock::*;
use crate::types::SimpleImbalance;
use crate::*;
use frame_support::assert_ok;
use sp_runtime::{FixedPointNumber, FixedU128};

const ONE: Balance = 1_000_000_000_000;

#[macro_export]
macro_rules! check_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

#[macro_export]
macro_rules! check_state {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(HubAssetLiquidity::<Test>::get(), $x);
		assert_eq!(TotalTVL::<Test>::get(), $y);
		assert_eq!(HubAssetImbalance::<Test>::get(), $z);
	}};
}

fn init_omnipool(dai_amount: Balance, price: FixedU128) {
	assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, price));
	check_state!(
		price.checked_mul_int(dai_amount).unwrap(),
		0,
		SimpleImbalance::default()
	);
}

#[test]
fn add_stable_asset_works() {
	new_test_ext().execute_with(|| {
		let dai_amount = 100 * ONE;

		assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, FixedU128::from(1)));

		check_state!(dai_amount, 0, SimpleImbalance::default());
	});
}

#[test]
fn add_token_works() {
	new_test_ext().execute_with(|| {
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
			price.checked_mul_int(token_amount).unwrap() + dai_amount,
			token_amount,
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
				price.checked_mul_int(token_amount).unwrap() + dai_amount,
				token_amount,
				SimpleImbalance::default()
			);

			assert_ok!(Omnipool::add_liquidity(Origin::signed(1), 1_000, token_amount));

			check_state!(700 * ONE, 600 * ONE, SimpleImbalance::default());

			check_balance!(LP1, 1_000, 700 * ONE)
		});
}
