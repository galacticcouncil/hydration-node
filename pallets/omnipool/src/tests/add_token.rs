use super::*;
use frame_support::assert_noop;

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
	ExtBuilder::default().build().execute_with(|| {
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

#[test]
fn cannot_add_existing_asset() {
	ExtBuilder::default().build().execute_with(|| {
		init_omnipool(1000 * ONE, FixedU128::from_float(0.5));
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			1_000,
			2000 * ONE,
			FixedU128::from_float(0.5)
		));

		assert_noop!(
			Omnipool::add_token(Origin::root(), 1_000, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::AssetAlreadyAdded
		);
		assert_noop!(
			Omnipool::add_token(Origin::root(), DAI, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::AssetAlreadyAdded
		);
		assert_noop!(
			Omnipool::add_token(Origin::root(), HDX, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::AssetAlreadyAdded
		);
	});
}

#[test]
fn first_assset_must_be_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::add_token(Origin::root(), HDX, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::NoStableCoinInPool
		);
		assert_noop!(
			Omnipool::add_token(Origin::root(), 1_000, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::NoNativeAssetInPool
		);
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			DAI,
			1000 * ONE,
			FixedU128::from_float(0.6)
		));
	});
}

#[test]
fn second_assset_must_be_native_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			DAI,
			1000 * ONE,
			FixedU128::from_float(0.6)
		));
		assert_noop!(
			Omnipool::add_token(Origin::root(), 1_000, 2000 * ONE, FixedU128::from_float(0.5)),
			Error::<Test>::NoNativeAssetInPool
		);
	});
}

#[test]
fn add_hub_assset_as_protocol_must_have_correct_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![])
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_token(Origin::root(), DAI, 1000 * ONE, FixedU128::from_float(0.6)),
				Error::<Test>::MissingBalance
			);
		});
}

#[test]
fn add_token_with_insufficient_balance_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP3, 1_000, 100 * ONE))
		.build()
		.execute_with(|| {
			init_omnipool(1000 * ONE, FixedU128::from_float(0.5));
			assert_noop!(
				Omnipool::add_token(Origin::signed(LP3), 1_000, 1000 * ONE, FixedU128::from_float(0.6)),
				orml_tokens::Error::<Test>::BalanceTooLow
			);
		});
}
