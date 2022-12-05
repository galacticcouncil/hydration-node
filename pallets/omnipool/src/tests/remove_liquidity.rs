use super::*;
use crate::types::Tradability;
use frame_support::assert_noop;
use sp_runtime::traits::One;
use test_case::test_case;

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
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			let position = Positions::<Test>::get(current_position_id).unwrap();

			let expected = Position::<Balance, AssetId> {
				asset_id: 1_000,
				amount: liq_added - liq_removed,
				shares: liq_added - liq_removed,
				price: (1560 * ONE, 2400 * ONE),
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
				price: (1560 * ONE, 2400 * ONE),
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

			assert_ok!(Omnipool::buy(Origin::signed(LP2), 1_000, DAI, 200 * ONE, 500000 * ONE));

			assert_balance!(Omnipool::protocol_account(), 1000, 300 * ONE);
			let expected_state = AssetReserveState {
				reserve: 300 * ONE,
				hub_reserve: 541666666666667,
				shares: 500000000000000,
				protocol_shares: Balance::zero(),
				cap: DEFAULT_WEIGHT_CAP,
				tradable: Tradability::default(),
			};
			assert_asset_state!(1_000, expected_state);

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_added
			));
			assert_balance!(Omnipool::protocol_account(), 1000, 60 * ONE);
			assert_balance!(LP1, 1000, 4_840_000_000_000_000);
			assert_balance!(LP1, LRNA, 203_921_568_627_449);

			assert_pool_state!(10391666666666667, 64723183391003641, SimpleImbalance::default());
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

			let expected_state = AssetReserveState::<Balance> {
				reserve: 500000000000000,
				hub_reserve: 325000000000000,
				shares: 500000000000000,
				protocol_shares: 0,
				cap: DEFAULT_WEIGHT_CAP,
				tradable: Tradability::default(),
			};
			assert_asset_state!(asset_a, expected_state);

			assert_ok!(Omnipool::sell(Origin::signed(LP2), asset_a, HDX, 100 * ONE, 10 * ONE));

			// ACT
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				400 * ONE
			));

			// Assert
			// - check if balance of LP and protocol are correct
			// - check new state of asset a in the pool ( should have updated protocol shares)
			assert_balance!(Omnipool::protocol_account(), asset_a, 206557377049181);
			assert_balance!(LP1, asset_a, 4993442622950819);

			assert_pool_state!(10647404371584700, 21294808743169400, SimpleImbalance::default());

			let expected_state = AssetReserveState {
				reserve: 206557377049181,
				hub_reserve: 93237704918034,
				shares: 172131147540984,
				protocol_shares: 72131147540984,
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

#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn remove_liquidity_should_work_when_trade_volume_not_exceeded(diff_from_max_limit: Balance) {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2_000_000 * ONE),
			(LP1, 1_000, 2_000_000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 1_000_000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let liq_added = TEN_PERCENT.mul_floor(initial_liquidity);
			let current_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			let liq_removed = TEN_PERCENT.mul_floor(initial_liquidity) - diff_from_max_limit;

			// Act & Assert
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_removed
			));
		});
}

use frame_support::traits::OnFinalize;
#[test]
fn remove_liquidity_should_fail_when_trade_volume_exceeded() {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2_000_000 * ONE),
			(LP1, 1_000, 2_000_000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let liq_added = TEN_PERCENT.mul_floor(initial_liquidity);
			let first_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			// reset the trade limit
			CircuitBreaker::on_finalize(1);

			let second_position_id = <NextPositionId<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			// reset the trade limit
			CircuitBreaker::on_finalize(2);

			let liq_removed = TEN_PERCENT.mul_floor(initial_liquidity);

			// Act & Assert
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				first_position_id,
				liq_removed
			));

			let liq_removed = FIVE_PERCENT.mul_floor(initial_liquidity);

			// Act & Assert
			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), second_position_id, liq_removed),
				pallet_circuit_breaker::Error::<Test>::MinTradeVolumePerBlockReached
			);
		});
}
