use crate::mock::*;
use crate::types::{AssetState, Position, SimpleImbalance};
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

#[macro_export]
macro_rules! check_asset_state {
	( $x:expr, $y:expr) => {{
		let actual = Assets::<Test>::get($x).unwrap();
		assert_eq!(actual, $y);
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

#[test]
fn simple_sell_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 100, 2000 * ONE),
			(Omnipool::protocol_account(), 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(Origin::root(), 100, token_amount, token_price,));

			assert_ok!(Omnipool::add_token(Origin::root(), 200, token_amount, token_price,));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(Origin::signed(LP1), 100, 200, sell_amount, min_limit));

			assert_eq!(Tokens::free_balance(100, &LP1), 550000000000000);
			assert_eq!(Tokens::free_balance(200, &LP1), 47808764940238);
			check_asset_state!(
				100,
				AssetState {
					reserve: 2450 * ONE,
					hub_reserve: 1528163265306123,
					shares: 2400 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 3120 * ONE
				}
			);
			check_asset_state!(
				200,
				AssetState {
					reserve: 1952191235059762,
					hub_reserve: 1331836734693877,
					shares: 2000 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 2000 * ONE
				}
			);
		});
}
