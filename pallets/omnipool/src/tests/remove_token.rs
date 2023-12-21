use super::*;
use crate::types::Tradability;
use frame_support::assert_noop;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn remove_token_should_fail_when_not_root() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP1),
				current_position_id
			));

			assert_noop!(Omnipool::remove_token(RuntimeOrigin::signed(LP1), 1000, LP1), BadOrigin,);
		});
}

#[test]
fn remove_token_should_fail_when_asset_is_not_frozen() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP1),
				current_position_id
			));

			assert_noop!(
				Omnipool::remove_token(RuntimeOrigin::root(), 1000, LP1),
				Error::<Test>::AssetNotFrozen
			);
		});
}

#[test]
fn remove_token_should_fail_when_lp_shares_remaining() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::FROZEN
			));
			assert_noop!(
				Omnipool::remove_token(RuntimeOrigin::root(), 1000, LP1),
				Error::<Test>::SharesRemaining
			);
		});
}

#[test]
fn remove_token_should_remove_asset_from_omnipool() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::FROZEN
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP1),
				current_position_id
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP2),
				current_position_id - 1,
			));
			assert_ok!(Omnipool::remove_token(RuntimeOrigin::root(), 1000, LP1),);

			assert!(Assets::<Test>::get(1000).is_none());
		});
}

#[test]
fn remove_token_should_transfer_remaining_asset_to_beneficiary_account() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::FROZEN
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP1),
				current_position_id
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP2),
				current_position_id - 1,
			));
			let reserve = Tokens::free_balance(1000, &Omnipool::protocol_account());
			assert_ok!(Omnipool::remove_token(RuntimeOrigin::root(), 1000, 1234),);
			assert_balance!(1234, 1_000, reserve);
			assert_balance!(Omnipool::protocol_account(), 1_000, 0);
			assert_balance!(1234, LRNA, 0);
		});
}

#[test]
fn remove_token_should_burn_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 1_000, 2000 * ONE),
			(LP2, DAI, 2000 * ONE),
			(LP1, 1_000, 5000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_min_withdrawal_fee(Permill::from_float(0.01))
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			let current_position_id = <NextPositionId<Test>>::get();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::FROZEN
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP1),
				current_position_id
			));
			assert_ok!(Omnipool::sacrifice_position(
				RuntimeOrigin::signed(LP2),
				current_position_id - 1,
			));
			let state = Assets::<Test>::get(1000).unwrap();
			let lrna_reserve = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			let lrna_issuance = Tokens::total_issuance(LRNA);
			assert_ok!(Omnipool::remove_token(RuntimeOrigin::root(), 1000, 1234),);
			let lrna_issuance_after = Tokens::total_issuance(LRNA);
			assert_balance!(Omnipool::protocol_account(), LRNA, lrna_reserve - state.hub_reserve);
			assert_eq!(lrna_issuance_after, lrna_issuance - state.hub_reserve);
		});
}
