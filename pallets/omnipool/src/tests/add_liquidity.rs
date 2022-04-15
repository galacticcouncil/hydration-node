use super::*;
use frame_support::assert_noop;

#[test]
fn add_liquidity_works() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::root(),
				1_000,
				token_amount,
				FixedU128::from_float(0.65)
			));

			check_state!(11_800 * ONE, 23_600 * ONE, SimpleImbalance::default());

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			check_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added,
					hub_reserve: 1560 * ONE,
					shares: 2400 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 3120 * ONE
				}
			);

			let position = Positions::<Test>::get(0).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added,
				shares: liq_added,
				price: Position::<Balance, AssetId>::price_to_balance(token_price),
			};

			assert_eq!(position, expected);

			check_state!(12_060 * ONE, 24_720 * ONE, SimpleImbalance::default());

			check_balance!(LP1, 1_000, 4600 * ONE);

			let minted_position = POSITIONS.with(|v| v.borrow().get(&0).copied());

			assert_eq!(minted_position, Some(LP1));
		});
}

#[test]
fn add_liquidity_for_non_pool_token_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			assert_noop!(
				Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 2000 * ONE,),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn add_liquidity_with_insufficient_balance_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				1_000,
				2000 * ONE,
				FixedU128::from_float(0.65)
			));

			assert_noop!(
				Omnipool::add_liquidity(Origin::signed(LP3), 1_000, 2000 * ONE,),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn add_liquidity_exceeding_weight_cap_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_asset_weight_cap((1, 100))
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				1_000,
				100 * ONE,
				FixedU128::from_float(0.65)
			));

			assert_noop!(
				Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 2000 * ONE,),
				Error::<Test>::AssetWeightCapExceeded
			);
		});
}

#[test]
fn add_insufficient_liquidity_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_min_added_liquidity(5 * ONE)
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				1_000,
				2000 * ONE,
				FixedU128::from_float(0.65)
			));

			assert_noop!(
				Omnipool::add_liquidity(Origin::signed(LP3), 1_000, ONE,),
				Error::<Test>::InsufficientLiquidity
			);
		});
}
