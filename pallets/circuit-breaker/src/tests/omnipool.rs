use crate::tests::mock::*;
use frame_support::assert_noop;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use test_case::test_case;

//As a tradeoff we implemented these tests here, but they should be converted to integration tests.

#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn add_liquidity_should_work_when_trade_volume_limit_not_exceeded(diff_from_max_limit: Balance) {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some(TEN_PERCENT))
		.build()
		.execute_with(|| {
			let liq_added =
				CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() - diff_from_max_limit;

			// Act & Assert
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
		});
}

#[test]
fn add_liquidity_should_fail_when_trade_volume_limit_exceeded() {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some(TEN_PERCENT))
		.build()
		.execute_with(|| {
			let liq_added = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() + ONE;

			// Act & Assert
			assert_noop!(
				Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added),
				pallet_circuit_breaker::Error::<Test>::MaxLiquidityLimitPerBlockReached
			);
		});
}

#[test]
fn add_liquidity_should_fail_when_consequent_calls_exceed_trade_volume_limit() {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some(TEN_PERCENT))
		.build()
		.execute_with(|| {
			let liq_added = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap() + ONE;

			// Act & Assert
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added));
			assert_noop!(
				Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_added),
				pallet_circuit_breaker::Error::<Test>::MaxLiquidityLimitPerBlockReached
			);
		});
}
#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn sell_should_work_when_trade_volume_limit_not_exceeded(diff_from_max_limit: Balance) {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;
			let sell_amount =
				CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() - diff_from_max_limit;

			// Act & Assert
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				sell_amount,
				min_limit
			));
		});
}

#[test]
fn sell_should_fail_when_trade_volume_max_limit_exceeded() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;
			let sell_amount = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() + ONE;

			// Act & Assert
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(TRADER), DOT, ACA, sell_amount, min_limit),
				pallet_circuit_breaker::Error::<Test>::TokenInfluxLimitReached
			);
		});
}

#[test]
fn sell_should_fail_when_consequent_trades_exceed_trade_volume_max_limit() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;
			let sell_amount = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap() + ONE;

			// Act & Assert
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				sell_amount,
				min_limit
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(TRADER), DOT, ACA, sell_amount, min_limit),
				pallet_circuit_breaker::Error::<Test>::TokenInfluxLimitReached
			);
		});
}

#[test]
fn sell_should_fail_when_trade_volume_min_limit_exceeded() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.50), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;
			let sell_amount = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap();

			// Act & Assert
			//Asset_out amount would be 1056_910_569_105_689 in a successful trade, but it fails due to limit
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(TRADER), DOT, ACA, sell_amount, min_limit),
				pallet_circuit_breaker::Error::<Test>::TokenOutflowLimitReached
			);
		});
}

#[test]
fn sell_should_fail_when_consequent_trades_exceed_trade_volume_min_limit() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.50), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;
			let sell_amount = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap();

			// Act & Assert
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				sell_amount,
				min_limit
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(TRADER), DOT, ACA, sell_amount, min_limit),
				pallet_circuit_breaker::Error::<Test>::TokenOutflowLimitReached
			);
		});
}

#[test]
fn trade_volume_limit_should_be_ignored_for_hub_asset() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;
	let aca_price = FixedU128::from_float(0.65);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, LRNA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, aca_price, LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let min_limit = 10 * ONE;

			let sell_amount =
				CircuitBreaker::calculate_limit(aca_price.checked_mul_int(initial_liquidity).unwrap(), TEN_PERCENT)
					.unwrap() + ONE;

			// Act & Assert
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(TRADER),
				LRNA,
				ACA,
				sell_amount,
				min_limit
			),);
		});
}

#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn buy_should_work_when_trade_volume_limit_not_exceeded(diff_from_min_limit: Balance) {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
			(TRADER, ACA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.8), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount =
				CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() - diff_from_min_limit;

			// Act & Assert
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				buy_amount,
				Balance::MAX
			));
		});
}

#[test]
fn buy_should_fail_when_trade_volume_max_limit_exceeded() {
	// Arrange
	const DOT: AssetId = 500;
	const ACA: AssetId = 600;
	const TRADER: u64 = 11u64;

	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
			(TRADER, ACA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap();

			// Act & assert
			//Asset_in amount would be 1250_000_000_000_002 in a successful trade, but it fails due to limit
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(TRADER), DOT, ACA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::TokenInfluxLimitReached
			);
		});
}

#[test]
fn buy_should_fail_when_consequent_trades_exceed_trade_volume_max_limit() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
			(TRADER, ACA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap() + ONE;

			// Act & assert
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				buy_amount,
				Balance::MAX
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(TRADER), DOT, ACA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::TokenInfluxLimitReached
			);
		});
}

#[test]
fn buy_should_fail_when_trade_volume_min_limit_exceeded() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
			(TRADER, ACA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.8), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() + ONE;

			// Act & assert
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(TRADER), DOT, ACA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::TokenOutflowLimitReached
			);
		});
}

#[test]
fn buy_should_fail_when_consequent_trades_exceed_trade_volume_min_limit() {
	// Arrange
	let initial_liquidity = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, DOT, 2_000_000 * ONE),
			(TRADER, ACA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.8), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap() + ONE;

			// Act & assert
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(TRADER),
				DOT,
				ACA,
				buy_amount,
				Balance::MAX
			));

			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(TRADER), DOT, ACA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::TokenOutflowLimitReached
			);
		});
}

#[test]
fn trade_volume_limit_should_be_ignored_for_hub_asset_when_buying_asset_for_hub_asset() {
	// Arrange
	let initial_liquidity = 100_000 * ONE;
	let dot_price = FixedU128::from_float(0.65);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2_000_000 * ONE),
			(LP1, ACA, 2_000_000 * ONE),
			(TRADER, LRNA, 2_000_000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(ACA)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, dot_price, LP1, initial_liquidity)
		.with_token(ACA, FixedU128::from_float(0.65), LP1, initial_liquidity)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount =
				CircuitBreaker::calculate_limit(dot_price.checked_mul_int(initial_liquidity).unwrap(), TEN_PERCENT)
					.unwrap() + ONE;

			// Act & assert
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(TRADER),
				DOT,
				LRNA,
				buy_amount,
				Balance::MAX
			),);
		});
}

#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn remove_liquidity_should_work_when_liquidity_volume_limit_not_exceeded(diff_from_max_limit: Balance) {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some(TEN_PERCENT))
		.with_max_remove_liquidity_limit_per_block(Some(TEN_PERCENT))
		.build()
		.execute_with(|| {
			let liq_amount =
				CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap() - diff_from_max_limit;

			let position_id = pallet_omnipool::Pallet::<Test>::next_position_id();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_amount));

			// Act & Assert
			assert_ok!(Omnipool::remove_liquidity(
				RuntimeOrigin::signed(LP1),
				position_id,
				liq_amount
			));
		});
}

#[test]
fn remove_liquidity_should_fail_when_liquidity_volume_limit_exceeded() {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some(TEN_PERCENT))
		.with_max_remove_liquidity_limit_per_block(Some(FIVE_PERCENT))
		.build()
		.execute_with(|| {
			let liq_amount = CircuitBreaker::calculate_limit(initial_liquidity, TEN_PERCENT).unwrap();

			let position_id = pallet_omnipool::Pallet::<Test>::next_position_id();
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 1_000, liq_amount),);

			// Act & Assert
			assert_noop!(
				Omnipool::remove_liquidity(RuntimeOrigin::signed(LP1), position_id, liq_amount),
				pallet_circuit_breaker::Error::<Test>::MaxLiquidityLimitPerBlockReached
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_consequent_calls_exceed_liquidity_volume_limit() {
	// Arrange
	let initial_liquidity = 1_000_000 * ONE;
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 2_000_000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2_000_000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, initial_liquidity)
		.with_max_add_liquidity_limit_per_block(Some((2_000, 10_000)))
		.with_max_remove_liquidity_limit_per_block(Some(TEN_PERCENT))
		.build()
		.execute_with(|| {
			let liq_amount = CircuitBreaker::calculate_limit(initial_liquidity, FIVE_PERCENT).unwrap() + ONE;

			let position_id = pallet_omnipool::Pallet::<Test>::next_position_id();
			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(LP1),
				1_000,
				liq_amount.checked_mul(3).unwrap()
			));

			// Act & Assert
			assert_ok!(Omnipool::remove_liquidity(
				RuntimeOrigin::signed(LP1),
				position_id,
				liq_amount
			));
			assert_noop!(
				Omnipool::remove_liquidity(RuntimeOrigin::signed(LP1), position_id, liq_amount),
				pallet_circuit_breaker::Error::<Test>::MaxLiquidityLimitPerBlockReached
			);
		});
}
