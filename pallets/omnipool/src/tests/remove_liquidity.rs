use super::*;
use crate::types::Tradable;
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				token_amount,
				token_price
			));
			let liq_added = 400 * ONE;

			let current_position_id = <PositionInstanceSequencer<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert_balance!(LP1, 1_000, 4600 * ONE);

			let liq_removed = 200 * ONE;
			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_removed
			));

			assert_pool_state!(11_930 * ONE + 1, 23_860_000_000_000_002, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 4600 * ONE + liq_removed);

			assert_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1430000000000001, // TODO: check why 1 at the end ?!!
					shares: 2400 * ONE - liq_removed,
					protocol_shares: 2000 * ONE, // no change, price has not changed
					tvl: 2_860_000_000_000_002,
					tradable: Tradable::default(),
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				token_amount,
				token_price
			));
			let liq_added = 400 * ONE;
			let lp1_position_id = <PositionInstanceSequencer<Test>>::get();
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

			assert_pool_state!(11_800 * ONE + 1, 23_600_000_000_000_002, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 5000 * ONE);

			assert_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1300000000000001,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: 2000 * ONE,
					tvl: 2_600_000_000_000_002,
					tradable: Tradable::default(),
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				token_amount,
				token_price
			));
			let liq_added = 400 * ONE;
			let current_position_id = <PositionInstanceSequencer<Test>>::get();

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

			assert_pool_state!(11_930 * ONE + 1, 23_860_000_000_000_002, SimpleImbalance::default());

			assert_balance!(LP1, 1_000, 4800 * ONE);

			assert_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1430000000000001,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: 2000 * ONE,
					tvl: 2_860_000_000_000_002,
					tradable: Tradable::default(),
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			let token_amount = 100 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP3),
				1_000,
				token_amount,
				token_price
			));

			let liq_added = 400 * ONE;

			let current_position_id = <PositionInstanceSequencer<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert_ok!(Omnipool::buy(Origin::signed(LP2), 1_000, DAI, 300 * ONE, 500000 * ONE));

			assert_balance!(Omnipool::protocol_account(), 1000, 200 * ONE);
			let expected_state = AssetState {
				reserve: 200 * ONE,
				hub_reserve: 812500000000001,
				shares: 500000000000000,
				protocol_shares: 100 * ONE,
				tvl: 650000000000000,
				tradable: Tradable::default(),
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

			assert_pool_state!(9704310344827587, 541000000000086413, SimpleImbalance::default());
		});
}

#[test]
fn protocol_shares_update_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP3, 1_000, 100 * ONE),
			(LP1, 1_000, 5000 * ONE),
			(LP2, 1_000, 5000 * ONE),
		])
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			let token_amount = 100 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP3),
				1_000,
				token_amount,
				token_price
			));

			let liq_added = 400 * ONE;
			let current_position_id = <PositionInstanceSequencer<Test>>::get();

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert_ok!(Omnipool::sell(Origin::signed(LP2), 1_000, HDX, 1000 * ONE, 10 * ONE));

			assert_balance!(Omnipool::protocol_account(), 1000, 1500 * ONE);

			let expected_state = AssetState {
				reserve: 1500 * ONE,
				hub_reserve: 108333333333334,
				shares: 500000000000000,
				protocol_shares: 100 * ONE,
				tvl: 650000000000000,
				tradable: Tradable::default(),
			};
			assert_asset_state!(1_000, expected_state);

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_added
			));
			assert_balance!(Omnipool::protocol_account(), 1000, 1259999999999997);
			assert_balance!(LP1, 1000, 4840000000000003);

			assert_pool_state!(10807666666666667, 21182000000000002, SimpleImbalance::default());

			let expected_state = AssetState {
				reserve: 1259999999999997,
				hub_reserve: 91000000000001,
				shares: 419999999999999,
				protocol_shares: 419999999999999,
				tvl: 182000000000002,
				tradable: Tradable::default(),
			};
			assert_asset_state!(1_000, expected_state);
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				2_000 * ONE,
				FixedU128::one()
			));
			let current_position_id = <PositionInstanceSequencer<Test>>::get();
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				2_000 * ONE,
				FixedU128::one()
			));
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP2),
				1_000,
				2_000 * ONE,
				FixedU128::one()
			));
			let current_position_id = <PositionInstanceSequencer<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, 500 * ONE));

			assert_noop!(
				Omnipool::remove_liquidity(Origin::signed(LP1), current_position_id, 500 * ONE + 1),
				Error::<Test>::InsufficientShares
			);
		});
}
