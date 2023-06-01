use super::*;
use frame_support::assert_noop;
use pretty_assertions::assert_eq;

#[test]
fn sacrifice_position_should_work_when_position_exists_with_correct_owner() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((LP2, asset_id, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Arrange - create a position
			let position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), asset_id, 400 * ONE));

			let lp1_asset_balance = Tokens::free_balance(asset_id, &LP1);

			// Act
			assert_ok!(Omnipool::sacrifice_position(RuntimeOrigin::signed(LP1), position_id));

			// Assert
			// - shares becomes protocol owned shares
			// - position is destroyed
			// - nft is burned
			// - LP does not receives anything
			assert_asset_state!(
				asset_id,
				AssetReserveState {
					reserve: 2400 * ONE,
					hub_reserve: 1560 * ONE,
					shares: 2400 * ONE,
					protocol_shares: 400 * ONE,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Positions::<Test>::get(position_id), None);

			assert_eq!(lp1_asset_balance, Tokens::free_balance(asset_id, &LP1));

			assert_eq!(POSITIONS.with(|v| v.borrow().get(&position_id).copied()), None);
		});
}

#[test]
fn sacrifice_position_should_fail_when_caller_is_not_position_owner() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((LP2, asset_id, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Arrange - create a position
			let position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), asset_id, 400 * ONE));

			// Act
			assert_noop!(
				Omnipool::sacrifice_position(RuntimeOrigin::signed(LP2), position_id),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn sacrifice_position_should_fail_when_position_does_not_exist() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((LP2, asset_id, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Arrange - create a position
			let position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), asset_id, 400 * ONE));

			// Act
			assert_noop!(
				Omnipool::sacrifice_position(RuntimeOrigin::signed(LP1), position_id + 1),
				Error::<Test>::PositionNotFound
			);
		});
}

#[test]
fn sacrifice_position_should_emit_event_when_succesful() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((LP2, asset_id, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			// Arrange - create a position
			let position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), asset_id, 400 * ONE));

			// Act
			assert_ok!(Omnipool::sacrifice_position(RuntimeOrigin::signed(LP1), position_id));

			// Assert
			frame_system::Pallet::<Test>::assert_last_event(
				crate::Event::PositionDestroyed {
					position_id,
					owner: LP1,
				}
				.into(),
			);
		});
}
