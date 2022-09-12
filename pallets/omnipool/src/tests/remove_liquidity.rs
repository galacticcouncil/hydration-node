use super::*;
use crate::types::Tradability;
use frame_support::assert_noop;
use sp_runtime::traits::One;

#[test]
fn remove_liquidity_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			let liq_added = 400 * ONE;

			let current_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert_balance!(LP1, 1_000, 4600 * ONE);

			let liq_removed = 200 * ONE;
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_removed
			));

			assert_pool_state!(11_930 * ONE, 23_860_000_000_000_000, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 4600 * ONE + liq_removed);

			assert_asset_state!(
				1_000,
				AssetReserveState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1430000000000000,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: Balance::zero(),
					tvl: 2_860_000_000_000_000,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			let position = Positions::<Test>::get(current_position_id).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added - liq_removed,
				shares: liq_added - liq_removed,
				price: token_price.into_inner(),
			};

			assert_eq!(position, expected);
		});
}

#[test]
fn full_liquidity_removal_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;

			let liq_added = 400 * ONE;
			let lp1_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert!(
				get_mock_minted_position(lp1_position_id).is_some(),
				"Position instance was not minted"
			);

			let liq_removed = 400 * ONE;

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				lp1_position_id,
				liq_removed
			));

			assert!(
				Positions::<Test>::get(lp1_position_id).is_none(),
				"Position still found"
			);

			assert_pool_state!(11_800 * ONE, 23_600_000_000_000_000, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 5000 * ONE);

			assert_asset_state!(
				1_000,
				AssetReserveState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1300000000000000,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: Balance::zero(),
					tvl: 2_600_000_000_000_000,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert!(
				get_mock_minted_position(lp1_position_id).is_none(),
				"Position instance was not burned"
			);
		});
}

#[test]
fn partial_liquidity_removal_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert!(
				get_mock_minted_position(current_position_id).is_some(),
				"Position instance was not minted"
			);

			let liq_removed = 200 * ONE;

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_removed
			));

			assert!(
				Positions::<Test>::get(current_position_id).is_some(),
				"Position has been removed incorrectly"
			);

			assert_pool_state!(11_930 * ONE, 23_860_000_000_000_000, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 4800 * ONE);

			assert_asset_state!(
				1_000,
				AssetReserveState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1430000000000000,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: Balance::zero(),
					tvl: 2_860_000_000_000_000,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert!(
				get_mock_minted_position(current_position_id).is_some(),
				"Position instance was burned"
			);
			let position = Positions::<Test>::get(current_position_id).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added - liq_removed,
				shares: liq_added - liq_removed,
				price: token_price.into_inner(),
			};

			assert_eq!(position, expected);
		});
}

#[test]
fn lp_receives_lrna_when_price_is_higher() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP3, 1_000, 100 * ONE),
			(LP1, 1_000, 5000 * ONE),
			(LP2, DAI, 50000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP3, 100 * ONE)
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;

			let current_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert_ok!(Omnipool::buy(Origin::signed(LP2), 1_000, DAI, 300 * ONE, 500000 * ONE));

			assert_balance!(Omnipool::protocol_account(), 1000, 200 * ONE);
			let expected_state = AssetReserveState {
				reserve: 200 * ONE,
				hub_reserve: 812500000000001,
				shares: 500000000000000,
				protocol_shares: Balance::zero(),
				tvl: 650000000000000,
				cap: DEFAULT_WEIGHT_CAP,
				tradable: Tradability::default(),
			};
			assert_asset_state!(1_000, expected_state);

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_added
			));
			assert_balance!(Omnipool::protocol_account(), 1000, 40 * ONE);
			assert_balance!(LP1, 1000, 4_760_000_000_000_000);
			assert_balance!(LP1, LRNA, 470_689_655_172_413);

			assert_pool_state!(10175000000000000, 541000000000086413, SimpleImbalance::default());
		});
}

#[test]
fn protocol_shares_should_update_when_removing_asset_liquidity_after_price_change() {
	let asset_a: AssetId = 1_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP3, asset_a, 100 * ONE),
			(LP1, asset_a, 5000 * ONE),
			(LP2, asset_a, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_a, FixedU128::from_float(0.65), LP3, 100 * ONE)
		.build()
		.execute_with(|| {
			// Arrange
			// - init pool
			// - add asset_a with initial liquidity of 100 * ONE
			// - add more liquidity of asset a - 400 * ONE
			// - perform a sell so the price changes - adding 1000 * ONE of asset a
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), asset_a, liq_added));

			assert_ok!(Omnipool::sell(Origin::signed(LP2), asset_a, HDX, 1000 * ONE, 10 * ONE));

			// ACT
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				400 * ONE
			));

			// Assert
			// - check if balance of LP and protocol are correct
			// - check new state of asset a in the pool ( should have updated protocol shares)
			assert_balance!(Omnipool::protocol_account(), asset_a, 1259999999999997);
			assert_balance!(LP1, asset_a, 4840000000000003);

			assert_pool_state!(10807666666666667, 21182000000000002, SimpleImbalance::default());

			let expected_state = AssetReserveState {
				reserve: 1259999999999997,
				hub_reserve: 91000000000001,
				shares: 419999999999999,
				protocol_shares: 319999999999999,
				tvl: 182000000000002,
				cap: DEFAULT_WEIGHT_CAP,
				tradable: Tradability::default(),
			};
			assert_asset_state!(asset_a, expected_state);
		});
}

#[test]
fn remove_liquidity_by_non_owner_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::one(), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 500 * ONE));

			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP3), current_position_id, 100 * ONE),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn remove_liquidity_from_non_existing_position_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::one(), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 500 * ONE));

			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), 1_000_000, 100 * ONE),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn remove_liquidity_cannot_exceed_position_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::one(), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 500 * ONE));

			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), current_position_id, 500 * ONE + 1),
				Error::<Test>::InsufficientShares
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_asset_is_not_allowed_to_remove() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 400 * ONE));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				1000,
				Tradability::BUY | Tradability::ADD_LIQUIDITY
			));

			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), current_position_id, 400 * ONE),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_shares_amount_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let current_position_id = <NextPositionId<Test>>::get();
			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), current_position_id, 0u128),
				Error::<Test>::InvalidSharesAmount
			);
		});
}
