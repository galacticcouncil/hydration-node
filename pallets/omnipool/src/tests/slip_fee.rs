use super::*;
use pretty_assertions::assert_eq;

#[test]
fn simple_sell_works() {
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
		.with_slip_fee()
        .build()
        .execute_with(|| {
            let liq_added = 400 * ONE;
            assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

            let sell_amount = 50 * ONE;
            let min_limit = 10 * ONE;

            assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

            assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
            assert_eq!(Tokens::free_balance(200, &LP1), 47_808_764_940_238);
            assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
            assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
            assert_eq!(
                Tokens::free_balance(200, &Omnipool::protocol_account()),
                1952_191_235_059_762
            );

            assert_pool_state!(13_360 * ONE, 26_720 * ONE);

            assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
            assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1952191235059762,
					hub_reserve: 1331836734693877,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
        });
}