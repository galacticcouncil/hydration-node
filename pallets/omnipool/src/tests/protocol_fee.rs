use super::*;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

/// Test that protocol fees are routed to HDX subpool hub reserve without burning
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
			// Record initial HDX hub reserve
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			// Perform a sell operation between two non-HDX assets
			// This ensures HDX hub reserve only changes due to protocol fee
			let sell_amount = 100 * ONE;
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, sell_amount, 0));

			// Get HDX state after trade
			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			// HDX hub reserve should have increased (protocol fee was added to it)
			assert!(
				final_hdx_hub_reserve > initial_hdx_hub_reserve,
				"HDX hub reserve should have increased due to protocol fee"
			);

			// Verify that no tokens were burned - total issuance should remain the same
			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged (no burning)"
			);

			// Verify the invariant: sum of all hub reserves equals hub token balance
			assert_hub_asset!();
		});
}

/// Test that protocol fees are routed to HDX subpool hub reserve on buy operations
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
			// Record initial HDX hub reserve
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;
			let initial_hub_token_supply = Tokens::total_issuance(LRNA);

			// Perform a buy operation between two non-HDX assets
			let buy_amount = 10 * ONE;
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				1000 * ONE
			));

			// Get HDX state after trade
			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			// HDX hub reserve should have increased (protocol fee was added to it)
			assert!(
				final_hdx_hub_reserve > initial_hdx_hub_reserve,
				"HDX hub reserve should have increased due to protocol fee"
			);

			// Verify that no tokens were burned - total issuance should remain the same
			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged (no burning)"
			);

			// Verify the invariant: sum of all hub reserves equals hub token balance
			assert_hub_asset!();
		});
}

/// Test that the hub reserve invariant holds after multiple trades with protocol fees
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

			// Perform multiple trades
			for _ in 0..5 {
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 10 * ONE, 0));

				// Verify invariant after each trade
				assert_hub_asset!();
			}

			// Perform some buys
			for _ in 0..3 {
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 5 * ONE, 100 * ONE));

				// Verify invariant after each trade
				assert_hub_asset!();
			}

			// Verify that no tokens were burned across all trades
			let final_hub_token_supply = Tokens::total_issuance(LRNA);
			assert_eq!(
				initial_hub_token_supply, final_hub_token_supply,
				"Hub token supply should remain unchanged after all trades (no burning)"
			);
		});
}

/// Test that protocol fee increases HDX hub reserve by the expected amount
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
			// Record initial HDX hub reserve
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;

			// Perform a sell between non-HDX assets and capture the event to get protocol fee amount
			let sell_amount = 100 * ONE;
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, sell_amount, 0));

			// Get the protocol fee from the last event
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

			// Get HDX state after trade
			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			// The increase in HDX hub reserve should equal the protocol fee amount
			let hub_reserve_increase = final_hdx_hub_reserve.saturating_sub(initial_hdx_hub_reserve);
			assert_eq!(
				hub_reserve_increase, protocol_fee_amount,
				"HDX hub reserve increase should equal the protocol fee amount"
			);
		});
}

/// Test that zero protocol fee doesn't change HDX hub reserve
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
			// Record initial HDX hub reserve
			let hdx_state_before = Assets::<Test>::get(HDX).unwrap();
			let initial_hdx_hub_reserve = hdx_state_before.hub_reserve;

			// Perform a sell operation between non-HDX assets
			let sell_amount = 100 * ONE;
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, sell_amount, 0));

			// Get HDX state after trade
			let hdx_state_after = Assets::<Test>::get(HDX).unwrap();
			let final_hdx_hub_reserve = hdx_state_after.hub_reserve;

			// HDX hub reserve should remain unchanged when protocol fee is zero
			assert_eq!(
				final_hdx_hub_reserve, initial_hdx_hub_reserve,
				"HDX hub reserve should remain unchanged when protocol fee is zero"
			);

			// Verify the invariant still holds
			assert_hub_asset!();
		});
}
