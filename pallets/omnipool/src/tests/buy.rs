use super::*;
use frame_support::assert_noop;
use pretty_assertions::assert_eq;

#[test]
fn simple_buy_works() {
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
		.build()
		.execute_with(|| {
			// Arrange
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			// Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			// Assert
			assert_eq!(Tokens::free_balance(100, &LP1), 547598253275108);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				2452401746724892
			);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1950 * ONE);

			assert_pool_state!(13_360 * ONE, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2452401746724892,
					hub_reserve: 1526666666666666,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950 * ONE,
					hub_reserve: 1333333333333334,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn hub_asset_buy_fails() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), LRNA, HDX, 100 * ONE, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_insufficient_amount_fails() {
	ExtBuilder::default()
		.with_min_trade_amount(5 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), LRNA, HDX, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 1000, HDX, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);
		});
}

#[test]
fn buy_assets_not_in_pool_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::buy(RuntimeOrigin::signed(LP1), 1000, 2000, 100 * ONE, 0),
			Error::<Test>::AssetNotFound
		);

		assert_noop!(
			Omnipool::buy(RuntimeOrigin::signed(LP1), 2000, 1000, 100 * ONE, 0),
			Error::<Test>::AssetNotFound
		);
	});
}

#[test]
fn buy_with_insufficient_balance_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 500 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, HDX, 100 * ONE, 10 * ONE),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn buy_exceeding_limit_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 500 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, HDX, 100 * ONE, 10 * ONE),
				Error::<Test>::SellLimitExceeded
			);
		});
}

#[test]
fn buy_not_allowed_assets_fails() {
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
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::SELL
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::BUY
			));

			assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE));

			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				100,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				100,
				Tradability::BUY
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				100,
				Tradability::SELL
			));

			assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE));
		});
}

#[test]
fn buy_for_hub_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100_000_000_000_000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(LP2),
				100,
				400000000000000
			));

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				200,
				1,
				50_000_000_000_000,
				50_000_000_000_000
			));

			assert_balance_approx!(Omnipool::protocol_account(), 0, 10000000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 1000000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 13393333333333334u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 2400000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1950000000000000u128, 1);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 1);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 1);
			assert_balance_approx!(LP2, 100, 600000000000000u128, 1);
			assert_balance_approx!(LP3, 100, 1000000000000000u128, 1);
			assert_balance_approx!(LP3, 1, 66_666_666_666_667u128, 1);
			assert_balance_approx!(LP3, 200, 50000000000000u128, 1);

			assert_asset_state!(
				2,
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				0,
				AssetReserveState {
					reserve: 10000000000000000,
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2400000000000000,
					hub_reserve: 1560000000000000,
					shares: 2400000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950000000000000,
					hub_reserve: 1333333333333334,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(13393333333333334, 26786666666666668);
		});
}

#[test]
fn simple_buy_with_fee_works() {
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
		.with_asset_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;

			assert_eq!(Tokens::free_balance(200, &LP1), 0u128);

			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), token_amount);

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			let expected_zero_fee: Balance = 52_631_578_947_370;
			let expected_10_percent_fee: Balance = 58_823_529_411_766;

			assert!(expected_zero_fee < expected_10_percent_fee); // note: dont make much sense as values are constants, but good to see the diff for further verification

			let expect_sold_amount = expected_10_percent_fee;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			assert_eq!(Tokens::free_balance(100, &LP1), 1000 * ONE - expect_sold_amount);

			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);

			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				token_amount + expect_sold_amount
			);
		});
}

#[test]
fn buy_should_emit_event_with_correct_asset_fee_amount() {
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
		.with_asset_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;
			let expected_sold_amount = 58_823_529_411_766;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			expect_last_events(vec![
				Event::BuyExecuted {
					who: LP1,
					asset_in: 100,
					asset_out: 200,
					amount_in: expected_sold_amount,
					amount_out: buy_amount,
					hub_amount_in: 57142857142858,
					hub_amount_out: 63020408163266,
					asset_fee_amount: 5_555_555_555_556,
					protocol_fee_amount: 0,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(100, expected_sold_amount)],
					outputs: vec![Asset::new(1, 57142857142858)],
					fees: vec![
						Fee::new(LRNA, 0, Destination::Burned),
						Fee::new(LRNA, 0, Destination::Account(PROTOCOL_FEE_COLLECTOR)),
					],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(1, 63020408163266)],
					outputs: vec![Asset::new(200, buy_amount)],
					fees: vec![Fee::new(
						200,
						5555555555556,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
			]);

			let other_buy_amount = buy_amount + 100;
			//We check again to see if the operation id is correct
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				other_buy_amount,
				max_limit
			));

			expect_last_events(vec![
				Event::BuyExecuted {
					who: LP1,
					asset_in: 100,
					asset_out: 200,
					amount_in: 66170747640117,
					amount_out: other_buy_amount,
					hub_amount_in: 60499132204326,
					hub_amount_out: 66726462234742,
					asset_fee_amount: 5555555555567,
					protocol_fee_amount: 0,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(100, 66170747640117)],
					outputs: vec![Asset::new(1, 60499132204326)],
					fees: vec![
						Fee::new(LRNA, 0, Destination::Burned),
						Fee::new(LRNA, 0, Destination::Account(PROTOCOL_FEE_COLLECTOR)),
					],
					operation_stack: vec![ExecutionType::Omnipool(1)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(1, 66726462234742)],
					outputs: vec![Asset::new(200, other_buy_amount)],
					fees: vec![Fee::new(
						200,
						5555555555567,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Omnipool(1)],
				}
				.into(),
			]);
		});
}

#[test]
fn buy_should_emit_event_with_correct_protocol_fee_amount() {
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
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;
			let expected_sold_amount = 58_651_026_392_962;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			expect_last_events(vec![
				Event::BuyExecuted {
					who: LP1,
					asset_in: 100,
					asset_out: 200,
					amount_in: expected_sold_amount,
					amount_out: buy_amount,
					hub_amount_in: 56980056980057,
					hub_amount_out: 51282051282052,
					asset_fee_amount: 0,
					protocol_fee_amount: 5698005698005,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(100, expected_sold_amount)],
					outputs: vec![Asset::new(1, 56980056980057)],
					fees: vec![
						Fee::new(LRNA, 0, Destination::Burned),
						Fee::new(LRNA, 5698005698005, Destination::Account(PROTOCOL_FEE_COLLECTOR)),
					],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(1, 51282051282052)],
					outputs: vec![Asset::new(200, buy_amount)],
					fees: vec![Fee::new(200, 0, Destination::Account(Omnipool::protocol_account()))],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
			]);
		});
}

#[test]
fn buy_should_emit_event_with_correct_protocol_fee_amount_and_burn_fee() {
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
		.with_protocol_fee(Permill::from_percent(10))
		.with_burn_fee(Permill::from_percent(50))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;
			let expected_sold_amount = 58_651_026_392_962;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			expect_last_events(vec![
				Event::BuyExecuted {
					who: LP1,
					asset_in: 100,
					asset_out: 200,
					amount_in: expected_sold_amount,
					amount_out: buy_amount,
					hub_amount_in: 56980056980057,
					hub_amount_out: 51282051282052,
					asset_fee_amount: 0,
					protocol_fee_amount: 5698005698005,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(100, expected_sold_amount)],
					outputs: vec![Asset::new(1, 56980056980057)],
					fees: vec![
						Fee::new(LRNA, 2849002849002, Destination::Burned),
						Fee::new(LRNA, 2849002849003, Destination::Account(PROTOCOL_FEE_COLLECTOR)),
					],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: LP1,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(1, 51282051282052)],
					outputs: vec![Asset::new(200, buy_amount)],
					fees: vec![Fee::new(200, 0, Destination::Account(Omnipool::protocol_account()))],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
			]);
		});
}

#[test]
fn buy_should_fail_when_buying_more_than_in_pool() {
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
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 200, 100, 3000 * ONE, 100 * ONE),
				Error::<Test>::InsufficientLiquidity
			);
		});
}

#[test]
fn buy_for_hub_asset_should_fail_when_asset_out_is_not_allowed_to_sell() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100_000_000_000_000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::SELL | Tradability::ADD_LIQUIDITY
			));

			assert_noop!(
				Omnipool::buy(
					RuntimeOrigin::signed(LP3),
					200,
					1,
					50_000_000_000_000,
					50_000_000_000_000
				),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_for_hub_asset_should_fail_when_limit_exceeds() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100_000_000_000_000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(
					RuntimeOrigin::signed(LP3),
					200,
					1,
					20_000_000_000_000,
					30_000_000_000_000
				),
				Error::<Test>::SellLimitExceeded
			);
		});
}

#[test]
fn buy_should_fail_when_trading_same_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100_000_000_000_000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(
					RuntimeOrigin::signed(LP3),
					200,
					200,
					50_000_000_000_000,
					100_000_000_000
				),
				Error::<Test>::SameAssetTradeNotAllowed
			);
		});
}

#[test]
fn buy_should_work_when_trading_native_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(20))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;
			let hub_reserves: Balance = Assets::<Test>::iter().map(|v| v.1.hub_reserve).sum();
			let hub_balance = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			assert_eq!(hub_reserves, hub_balance);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				HDX,
				buy_amount,
				max_limit
			));

			assert_eq!(Tokens::free_balance(HDX, &LP1), 953354861858628);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13354534693877551
			);
			assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				10046645138141372
			);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1950000000000000
			);

			let hub_reserves: Balance = Assets::<Test>::iter().map(|v| v.1.hub_reserve).sum();
			let hub_balance = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			assert_eq!(hub_reserves, hub_balance);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950000000000000,
					hub_reserve: 1340963265306123,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10046645138141372,
					hub_reserve: 9953571428571428,
					shares: 10000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn buy_should_fail_when_exceeds_max_out_ratio() {
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
		.with_max_out_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, 200, 1000 * ONE, 0u128),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}

#[test]
fn buy_should_fail_when_exceeds_max_in_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 200, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.00), LP3, 500 * ONE)
		.with_max_in_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, 200, 200 * ONE, Balance::MAX),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn buy_for_lrna_should_fail_when_exceeds_max_in_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_max_in_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, LRNA, 1000 * ONE, Balance::MAX),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn buy_for_lrna_should_fail_when_exceeds_max_out_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, LRNA, 1500 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_max_out_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(LP1), 100, LRNA, 1500 * ONE, Balance::MAX),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}

#[test]
fn spot_price_after_buy_should_be_identical_when_protocol_fee_is_nonzero() {
	let mut spot_price_1 = FixedU128::zero();
	let mut spot_price_2 = FixedU128::zero();

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
		.with_asset_fee(Permill::from_percent(0))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				u128::MAX,
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_1 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

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
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				u128::MAX,
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_2 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	assert_eq_approx!(
		spot_price_1,
		spot_price_2,
		FixedU128::from_float(0.000000001),
		"spot price afters sells"
	);
}

#[test]
fn buy_with_all_fees_and_extra_withdrawal_works() {
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
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(3))
		.with_burn_fee(Permill::from_percent(50))
		.with_on_trade_withdrawal(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::one(), LP2, 2000 * ONE)
		.with_token(200, FixedU128::one(), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let buy_amount = 10 * ONE;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				u128::MAX,
			));

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2011585471818340,
					hub_reserve: 1988481253239648,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1989888888888889,
					hub_reserve: 2012184388751912,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 988414528181660);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(Tokens::free_balance(200, &TRADE_FEE_COLLECTOR), 111111111111);
			assert_eq!(Tokens::free_balance(LRNA, &PROTOCOL_FEE_COLLECTOR), 172781201405);

			// Account for 200 asset
			let initial_reserve = 2000 * ONE;
			let omnipool_200_reserve = Tokens::free_balance(200, &Omnipool::protocol_account());
			let fee_collector = Tokens::free_balance(200, &TRADE_FEE_COLLECTOR);
			assert_eq!(initial_reserve, omnipool_200_reserve + buy_amount + fee_collector);
		});
}
