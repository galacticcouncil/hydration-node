use crate::mock::*;
use crate::types::SimpleImbalance;
use crate::*;
use frame_support::assert_ok;
use sp_runtime::{FixedPointNumber, FixedU128};

const ONE: Balance = 1_000_000_000_000;

fn check_state(hub_asset_liquidity: Balance, tvl: Balance, imbalance: SimpleImbalance<Balance>) {
	assert_eq!(HubAssetLiquidity::<Test>::get(), hub_asset_liquidity);
	assert_eq!(TotalTVL::<Test>::get(), tvl);
	assert_eq!(HubAssetImbalance::<Test>::get(), imbalance);
}

fn init_omnipool(dai_amount: Balance, price: FixedU128) {
	assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, price));
	check_state(
		price.checked_mul_int(dai_amount).unwrap(),
		0,
		SimpleImbalance::default(),
	);
}

#[test]
fn add_stable_asset_works() {
	new_test_ext().execute_with(|| {
		let dai_amount = 100 * ONE;

		assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, FixedU128::from(1)));

		check_state(dai_amount, 0, SimpleImbalance::default());
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
		check_state(
			price.checked_mul_int(token_amount).unwrap() + dai_amount,
			token_amount,
			SimpleImbalance::default(),
		);
	});
}
