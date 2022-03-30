use super::*;

#[test]
fn add_stable_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Omnipool::protocol_account(), DAI, 100 * ONE)])
		.build()
		.execute_with(|| {
			let dai_amount = 100 * ONE;

			assert_ok!(Omnipool::add_token(Origin::root(), DAI, dai_amount, FixedU128::from(1)));

			check_state!(dai_amount, dai_amount, SimpleImbalance::default());
		});
}

#[test]
fn add_token_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 1_000, 2000 * ONE),
		])
		.build()
		.execute_with(|| {
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

			check_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount,
					hub_reserve: 1300 * ONE,
					shares: token_amount,
					protocol_shares: token_amount,
					tvl: token_amount
				}
			)
		});
}
