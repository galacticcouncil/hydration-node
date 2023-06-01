use super::*;
use frame_support::assert_noop;
use pretty_assertions::assert_eq;

#[test]
fn refund_refused_asset_should_work_when_asset_not_in_pool() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let lp1_asset_balance = Tokens::free_balance(asset_id, &LP1);
			let pool_asset_balance = Tokens::free_balance(asset_id, &Omnipool::protocol_account());

			// Act
			assert_ok!(Omnipool::refund_refused_asset(
				RuntimeOrigin::root(),
				asset_id,
				1000 * ONE,
				LP1
			));

			// Assert
			assert_eq!(Tokens::free_balance(asset_id, &LP1), lp1_asset_balance + 1000 * ONE);
			assert_eq!(
				Tokens::free_balance(asset_id, &Omnipool::protocol_account()),
				pool_asset_balance - 1000 * ONE
			);
		});
}

#[test]
fn refund_refused_asset_should_work_when_refund_partial_amount() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let lp1_asset_balance = Tokens::free_balance(asset_id, &LP1);
			let pool_asset_balance = Tokens::free_balance(asset_id, &Omnipool::protocol_account());

			// Act
			assert_ok!(Omnipool::refund_refused_asset(
				RuntimeOrigin::root(),
				asset_id,
				500 * ONE,
				LP1
			));

			// Assert
			assert_eq!(Tokens::free_balance(asset_id, &LP1), lp1_asset_balance + 500 * ONE);
			assert_eq!(
				Tokens::free_balance(asset_id, &Omnipool::protocol_account()),
				pool_asset_balance - 500 * ONE
			);
		});
}

#[test]
fn refund_refused_asset_should_fail_when_refund_asset_in_pool() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP1, 1000 * ONE)
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				Omnipool::refund_refused_asset(RuntimeOrigin::root(), asset_id, 1000 * ONE, LP1),
				Error::<Test>::AssetAlreadyAdded
			);
		});
}

#[test]
fn refund_refused_asset_should_fail_when_refund_more_than_in_pool_account() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				Omnipool::refund_refused_asset(RuntimeOrigin::root(), asset_id, 2000 * ONE, LP1),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn refund_refused_asset_should_emit_correct_event_when_succesful() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			// Act
			assert_ok!(Omnipool::refund_refused_asset(
				RuntimeOrigin::root(),
				asset_id,
				1000 * ONE,
				LP1
			));

			// Assert
			frame_system::Pallet::<Test>::assert_last_event(
				crate::Event::AssetRefunded {
					asset_id,
					amount: 1000 * ONE,
					recipient: LP1,
				}
				.into(),
			);
		});
}

#[test]
fn refund_refused_asset_should_fail_when_refund_asset_is_hub_asset() {
	let asset_id: AssetId = 1_000;

	ExtBuilder::default()
		.add_endowed_accounts((LP1, asset_id, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), asset_id, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(asset_id, FixedU128::from_float(0.65), LP1, 1000 * ONE)
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				Omnipool::refund_refused_asset(RuntimeOrigin::root(), LRNA, 1000 * ONE, LP1),
				Error::<Test>::AssetRefundNotAllowed
			);
		});
}
