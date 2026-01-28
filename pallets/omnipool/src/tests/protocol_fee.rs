use super::*;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

#[test]
fn protocol_fee_should_be_added_to_hdx_hub_reserve_on_sell() {
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
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 100 * ONE, 0));

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			assert!(
				final_hdx_hub_reserve > initial_hdx_hub_reserve,
				"HDX hub reserve should have increased due to protocol fee"
			);

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged (no burning)"
			);

			assert_hub_asset!();
		});
}

#[test]
fn protocol_fee_should_be_added_to_hdx_hub_reserve_on_buy() {
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
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				10 * ONE,
				1000 * ONE
			));

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			assert!(
				final_hdx_hub_reserve > initial_hdx_hub_reserve,
				"HDX hub reserve should have increased due to protocol fee"
			);

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged (no burning)"
			);

			assert_hub_asset!();
		});
}

#[test]
fn hub_reserve_invariant_should_hold_after_multiple_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 5000 * ONE),
			(LP3, 200, 5000 * ONE),
			(LP1, 100, 2000 * ONE),
			(LP1, 200, 2000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			for _ in 0..5 {
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 10 * ONE, 0));
				assert_hub_asset!();
			}

			for _ in 0..3 {
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 5 * ONE, 100 * ONE));
				assert_hub_asset!();
			}

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged after all trades (no burning)"
			);
		});
}

#[test]
fn protocol_fee_amount_should_match_hdx_hub_reserve_increase() {
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
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 100 * ONE, 0));

			let events = frame_system::Pallet::<Test>::events();
			let sell_event = events.iter().rev().find_map(|e| {
				if let RuntimeEvent::Omnipool(Event::SellExecuted {
					protocol_fee_amount, ..
				}) = &e.event
				{
					Some(*protocol_fee_amount)
				} else {
					None
				}
			});

			let protocol_fee_amount = sell_event.expect("SellExecuted event should exist");
			assert_ne!(protocol_fee_amount, 0);

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			let hub_reserve_increase = final_hdx_hub_reserve.saturating_sub(initial_hdx_hub_reserve);
			assert_eq!(
				hub_reserve_increase, protocol_fee_amount,
				"HDX hub reserve increase should equal the protocol fee amount"
			);
		});
}

#[test]
fn protocol_fee_should_be_correctly_applied_when_selling_hdx() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_reserve_before = Tokens::free_balance(HDX, &Omnipool::protocol_account());
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, 100, 100 * ONE, 0));

			let hdx_reserve_after = Tokens::free_balance(HDX, &Omnipool::protocol_account());
			assert!(
				hdx_reserve_after > hdx_reserve_before,
				"HDX reserve should increase when HDX is sold into pool"
			);

			let events = frame_system::Pallet::<Test>::events();
			let protocol_fee_amount = events
				.iter()
				.rev()
				.find_map(|e| {
					if let RuntimeEvent::Omnipool(Event::SellExecuted {
						protocol_fee_amount, ..
					}) = &e.event
					{
						Some(*protocol_fee_amount)
					} else {
						None
					}
				})
				.expect("SellExecuted event should exist");

			assert_ne!(protocol_fee_amount, 0, "Protocol fee should be non-zero");

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(initial_hub_token_supply, final_hub_token_supply);

			assert_hub_asset!();
		});
}

#[test]
fn protocol_fee_should_be_correctly_applied_when_buying_hdx() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let hdx_reserve_before = Tokens::free_balance(HDX, &Omnipool::protocol_account());
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				HDX,
				100,
				100 * ONE,
				1000 * ONE
			));

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let hdx_reserve_after = Tokens::free_balance(HDX, &Omnipool::protocol_account());

			assert!(
				hdx_reserve_after < hdx_reserve_before,
				"HDX reserve should decrease when HDX is bought from pool"
			);
			assert!(
				hdx_state_after.hub_reserve > hdx_state_before.hub_reserve,
				"HDX hub_reserve should increase from both trade and protocol fee"
			);

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(initial_hub_token_supply, final_hub_token_supply);

			assert_hub_asset!();
		});
}

#[test]
fn protocol_fee_should_be_correctly_applied_when_selling_asset_for_hdx() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let hdx_reserve_before = Tokens::free_balance(HDX, &Omnipool::protocol_account());
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 100 * ONE, 0));

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let hdx_reserve_after = Tokens::free_balance(HDX, &Omnipool::protocol_account());

			assert!(
				hdx_reserve_after < hdx_reserve_before,
				"HDX reserve should decrease when selling another asset for HDX"
			);
			assert!(
				hdx_state_after.hub_reserve > hdx_state_before.hub_reserve,
				"HDX hub_reserve should increase from both trade and protocol fee"
			);

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(initial_hub_token_supply, final_hub_token_supply);

			assert_hub_asset!();
		});
}

#[test]
fn hub_reserve_invariant_should_hold_after_multiple_hdx_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 5000 * ONE),
			(LP1, HDX, 5000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_protocol_fee(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			for _ in 0..3 {
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, 100, 50 * ONE, 0));
				assert_hub_asset!();
			}

			for _ in 0..3 {
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
				assert_hub_asset!();
			}

			for _ in 0..3 {
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP1), HDX, 100, 50 * ONE, 500 * ONE));
				assert_hub_asset!();
			}

			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(initial_hub_token_supply, final_hub_token_supply);
		});
}

#[test]
fn zero_protocol_fee_should_not_change_hdx_hub_reserve() {
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
		.with_protocol_fee(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 100 * ONE, 0));

			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			assert_eq!(
				final_hdx_hub_reserve, initial_hdx_hub_reserve,
				"HDX hub reserve should remain unchanged when protocol fee is zero"
			);

			assert_hub_asset!();
		});
}
