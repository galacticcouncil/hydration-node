use super::*;

#[test]
fn add_liquidity_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
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
			assert_ok!(Omnipool::add_liquidity(Origin::signed(1), 1_000, liq_added));

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

			let position = Positions::<Test>::get(PositionId(0)).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added,
				shares: liq_added,
				price: Position::<Balance, AssetId>::price_to_balance(token_price),
			};

			assert_eq!(position, expected);

			check_state!(12_060 * ONE, 24_720 * ONE, SimpleImbalance::default());

			check_balance!(LP1, 1_000, 4600 * ONE)
		});
}
