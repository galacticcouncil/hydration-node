use super::*;
use frame_support::assert_noop;
use sp_runtime::traits::One;

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
					tvl: 2_860_000_000_000_002
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

#[test]
fn full_liquidity_removal_works() {
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

			let current_position_id = <PositionInstanceSequencer<Test>>::get();

			assert_ok!(Omnipool::add_token(Origin::root(), 1_000, token_amount, token_price));
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 1_000, liq_added));

			assert!(
				get_mock_minted_position(current_position_id).is_some(),
				"Position instance was not minted"
			);

			let liq_removed = 400 * ONE;

			assert_ok!(Omnipool::remove_liquidity(
				Origin::signed(LP1),
				current_position_id,
				liq_removed
			));

			assert!(
				Positions::<Test>::get(current_position_id).is_none(),
				"Position still found"
			);

			check_state!(11_800 * ONE + 1, 24_200_000_000_000_002, SimpleImbalance::default());

			check_balance!(LP1, 1_000, 5000 * ONE);

			check_asset_state!(
				1_000,
				AssetState {
					reserve: token_amount + liq_added - liq_removed,
					hub_reserve: 1300000000000001,
					shares: 2400 * ONE - liq_removed,
					protocol_shares: 2000 * ONE,
					tvl: 2_600_000_000_000_002
				}
			);

			assert!(
				get_mock_minted_position(current_position_id).is_none(),
				"Position instance was not burned"
			);
		});
}

// Scenarios to test
// - price changes up
// - price changes down
// - remove all liquidity - check if position has been destroyed
// - scenario where add liquidty, then buy as another one, and then remove does not have neought asset

#[test]
fn remove_liquidity_by_non_owner_fails() {
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
			assert_ok!(Omnipool::add_token(
				Origin::root(),
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
			(Omnipool::protocol_account(), 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);
			assert_ok!(Omnipool::add_token(
				Origin::root(),
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
			(Omnipool::protocol_account(), 1_000, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);
			assert_ok!(Omnipool::add_token(
				Origin::root(),
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
