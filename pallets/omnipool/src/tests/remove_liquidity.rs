use super::*;

#[test]
fn remove_liquidity_works() {
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

			assert_ok!(Omnipool::add_token(Origin::root(), 1_000, token_amount, token_price));
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			check_balance!(LP1, 1_000, 4600 * ONE);

			let liq_removed = 200 * ONE;
			assert_ok!(Omnipool::remove_liquidity(Origin::signed(LP1), 0, liq_removed));

			check_state!(11_930 * ONE + 1, 24_460_000_000_000_002, SimpleImbalance::default());

			check_balance!(LP1, 1_000, 4600 * ONE + liq_removed);

			check_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1430000000000001, // TODO: check why 1 at the end ?!!
					shares: 2400 * ONE - liq_removed,
					protocol_shares: 2000 * ONE, // no change, price has not changed
					tvl: 2860_000_000_000_002
				}
			);

			let position = Positions::<Test>::get(0).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added - liq_removed,
				shares: liq_added - liq_removed,
				price: Position::<Balance, AssetId>::price_to_balance(token_price),
			};

			assert_eq!(position, expected);
		});
}

// Scenarios to test
// - price changes up
// - price changes down
// - remove all liquidity - check if position has been destroyed
