use super::*;
use crate::slip_fee::HubAssetBlockState;
use hydra_dx_math::omnipool::types::BalanceUpdate::{Decrease, Increase};
use pretty_assertions::assert_eq;

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

#[test]
fn single_sell_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2_000 * ONE),
			(LP3, 200, 2_000 * ONE),
			(LP1, 100, 1_000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(31_836_734_693_877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(30_443_447_932_238),
				}
			);

			let received = 45_764_362_220_059;
			assert_eq!(Tokens::free_balance(100, &LP1), 500 * ONE + sell_amount);
			assert_eq!(Tokens::free_balance(200, &LP1), received);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_269_978_544_483
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2_450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				2_000 * ONE - received
			);

			assert_pool_state!(13_359_269_978_544_483, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 2_000 * ONE - received,
					hub_reserve: 1_330_443_447_932_238,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50 * ONE,
				amount_out: received,
				hub_amount_in: 31_836_734_693_877,
				hub_amount_out: 30_443_447_932_238,
				asset_fee_amount: 0,
				protocol_fee_amount: 663_265_306_122,
			}
			.into()]);
		});
}

#[test]
fn sell_hub_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5_000 * ONE),
			(LP1, 200, 5_000 * ONE),
			(LP2, 100, 1_000 * ONE),
			(LP3, 100, 1_000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP2), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP3),
				LRNA,
				200,
				sell_amount,
				min_limit
			));

			let received = 71_428_571_428_571;

			assert_eq!(Tokens::free_balance(HDX, &Omnipool::protocol_account()), NATIVE_AMOUNT);
			assert_eq!(Tokens::free_balance(2, &Omnipool::protocol_account()), 1_000 * ONE);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13_410 * ONE);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2_400 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_928_571_428_571_429
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 3_000 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 3_000 * ONE);
			assert_eq!(Tokens::free_balance(100, &LP2), 600 * ONE);
			assert_eq!(Tokens::free_balance(100, &LP3), 1_000 * ONE);
			assert_eq!(Tokens::free_balance(LRNA, &LP3), 100 * ONE - sell_amount);
			assert_eq!(Tokens::free_balance(200, &LP3), received);

			assert_asset_state!(
				2,
				AssetReserveState {
					reserve: 1_000 * ONE,
					hub_reserve: 500 * ONE,
					shares: 1_000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10_000 * ONE,
					hub_reserve: 10_000 * ONE,
					shares: 10_000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_400 * ONE,
					hub_reserve: 1_560 * ONE,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_928_571_428_571_429,
					hub_reserve: 1_350 * ONE,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(13_410 * ONE, 26_820 * ONE);

			expect_events(vec![Event::SellExecuted {
				who: LP3,
				asset_in: LRNA,
				asset_out: 200,
				amount_in: 50 * ONE,
				amount_out: 71428571428571,
				hub_amount_in: 0,
				hub_amount_out: 0,
				asset_fee_amount: 0,
				protocol_fee_amount: 0,
			}
			.into()]);
		});
}

#[test]
fn two_sells_in_one_direction_should_increase_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2_000 * ONE),
			(LP3, 200, 2_000 * ONE),
			(LP1, 100, 1_000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(31_836_734_693_877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(30_443_447_932_238),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 45_764_362_220_059);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_269_978_544_483
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2_450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_954_235_637_779_941
			);

			assert_pool_state!(13_359_269_978_544_483, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_954_235_637_779_941,
					hub_reserve: 1_330_443_447_932_238,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50 * ONE,
				amount_out: 45_764_362_220_059,
				hub_amount_in: 31_836_734_693_877,
				hub_amount_out: 30_443_447_932_238,
				asset_fee_amount: 0,
				protocol_fee_amount: 663_265_306_122,
			}
			.into()]);

			System::reset_events();

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(62_399_999_999_999),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(58_446_539_616_640),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50 * ONE,
				amount_out: 40_284_720_923_298,
				hub_amount_in: 30_563_265_306_122,
				hub_amount_out: 28_003_091_684_402,
				asset_fee_amount: 0,
				protocol_fee_amount: 1_273_469_387_755,
			}
			.into()]);
		});
}

#[test]
fn single_buy_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2_000 * ONE),
			(LP3, 200, 2_000 * ONE),
			(LP1, 100, 1_000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2_000 * ONE)
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

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

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(35_014_477_305_990),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(33_333_333_333_334),
				}
			);

			// Assert
			let sold = 55_105_274_301_832;
			assert_eq!(Tokens::free_balance(100, &LP1), 600 * ONE - sold);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_360_000_000_000_000
			);
			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				2_455_105_274_301_832
			);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1_950 * ONE);

			assert_pool_state!(13_360 * ONE, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_400 * ONE + sold,
					hub_reserve: 1_524_985_522_694_010,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 2_000 * ONE - buy_amount,
					hub_reserve: 1_333_333_333_333_334,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::BuyExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: sold,
				amount_out: buy_amount,
				hub_amount_in: 35_014_477_305_990,
				hub_amount_out: 33_333_333_333_334,
				asset_fee_amount: 0,
				protocol_fee_amount: 1_681_143_972_656,
			}
			.into()]);
		});
}

#[test]
fn buy_for_hub_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1_000 * ONE),
			(LP1, 100, 5_000 * ONE),
			(LP1, 200, 5_000 * ONE),
			(LP2, 100, 1_000 * ONE),
			(LP3, 100, 1_000 * ONE),
			(LP3, 1, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP2), 100, 400 * ONE));

			let buy_amount = 50 * ONE;
			let max_limit = 50 * ONE;

			assert_ok!(Omnipool::buy(RuntimeOrigin::signed(LP3), 200, 1, buy_amount, max_limit,));

			let sold = 34_210_526_315_790;

			assert_eq!(Tokens::free_balance(HDX, &Omnipool::protocol_account()), NATIVE_AMOUNT);
			assert_eq!(Tokens::free_balance(2, &Omnipool::protocol_account()), 1_000 * ONE);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_394_210_526_315_790
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2_400 * ONE);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1_950 * ONE);

			assert_eq!(Tokens::free_balance(100, &LP1), 3_000 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 3_000 * ONE);
			assert_eq!(Tokens::free_balance(100, &LP2), 600 * ONE);
			assert_eq!(Tokens::free_balance(100, &LP3), 1_000 * ONE);
			assert_eq!(Tokens::free_balance(LRNA, &LP3), 100 * ONE - sold);
			assert_eq!(Tokens::free_balance(200, &LP3), buy_amount);

			assert_asset_state!(
				2,
				AssetReserveState {
					reserve: 1_000 * ONE,
					hub_reserve: 500 * ONE,
					shares: 1_000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10_000 * ONE,
					hub_reserve: 10_000 * ONE,
					shares: 10_000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_400 * ONE,
					hub_reserve: 1_560 * ONE,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_950 * ONE,
					hub_reserve: 1_300 * ONE + sold,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(13_360 * ONE + sold, 26_786_666_666_666_668);

			expect_events(vec![Event::BuyExecuted {
				who: LP3,
				asset_in: 1,
				asset_out: 200,
				amount_in: sold,
				amount_out: buy_amount,
				hub_amount_in: 0,
				hub_amount_out: 0,
				asset_fee_amount: 0,
				protocol_fee_amount: 0,
			}
			.into()]);
		});
}

#[test]
fn two_buys_in_one_direction_should_increase_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2_000 * ONE),
			(LP3, 200, 2_000 * ONE),
			(LP1, 100, 1_000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(35_014_477_305_990),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(33_333_333_333_334),
				}
			);

			let sold = 55_105_274_301_832;
			assert_eq!(Tokens::free_balance(100, &LP1), 600 * ONE - sold);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_360_000_000_000_000
			);
			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				2_455_105_274_301_832
			);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1_950 * ONE);

			assert_pool_state!(13_360 * ONE, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_400 * ONE + sold,
					hub_reserve: 1_524_985_522_694_010,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 2_000 * ONE - buy_amount,
					hub_reserve: 1_333_333_333_333_334,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::BuyExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: sold,
				amount_out: buy_amount,
				hub_amount_in: 35_014_477_305_990,
				hub_amount_out: 33_333_333_333_334,
				asset_fee_amount: 0,
				protocol_fee_amount: 1_681_143_972_656,
			}
			.into()]);

			System::reset_events();

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				max_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(73_881_142_035_403),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(68_421_052_631_580),
				}
			);

			expect_events(vec![Event::BuyExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 64_208_695_731_372,
				amount_out: 50 * ONE,
				hub_amount_in: 38_866_664_729_413,
				hub_amount_out: 35_087_719_298_246,
				asset_fee_amount: 0,
				protocol_fee_amount: 3_778_945_431_167,
			}
			.into()]);
		});
}

#[test]
fn sell_and_buy_can_cancel_out_and_bring_slip_fee_to_initial_state() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1_000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2_000 * ONE),
			(LP3, 200, 2_000 * ONE),
			(LP1, 100, 1_000 * ONE),
			(LP1, 200, 1_000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2_000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2_000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_slip_fee(
				RuntimeOrigin::root(),
				true,
				FixedU128::from_rational(5, 100)
			));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Decrease(31_836_734_693_877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Increase(30_443_447_932_238),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 1_045_764_362_220_059);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_269_978_544_483
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2_450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_954_235_637_779_941
			);

			assert_pool_state!(13_359_269_978_544_483, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2_450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2_400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_954_235_637_779_941,
					hub_reserve: 1_330_443_447_932_238,
					shares: 2_000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50 * ONE,
				amount_out: 45_764_362_220_059,
				hub_amount_in: 31_836_734_693_877,
				hub_amount_out: 30_443_447_932_238,
				asset_fee_amount: 0,
				protocol_fee_amount: 663_265_306_122,
			}
			.into()]);

			System::reset_events();

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			// Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				buy_amount,
				max_limit
			));

			expect_events(vec![Event::BuyExecuted {
				who: LP1,
				asset_in: 200,
				asset_out: 100,
				amount_in: 47_964_231_029_356,
				amount_out: 50 * ONE,
				hub_amount_in: 31_871_791_573_967,
				hub_amount_out: 31_836_734_693_878,
				asset_fee_amount: 0,
				protocol_fee_amount: 35_056_880_089,
			}
			.into()]);

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_560 * ONE,
					current_delta_hub_reserve: Increase(1),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1_300 * ONE,
					current_delta_hub_reserve: Decrease(1_428_343_641_729),
				}
			);
		});
}

#[test]
fn set_slip_fee_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// enable slip fee
		assert_ok!(Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			true,
			FixedU128::from_rational(5, 100)
		));

		expect_events(vec![Event::SlipFactorUpdated {
			slip_factor: FixedU128::one(),
			max_slip_fee: FixedU128::from_rational(5, 100),
		}
		.into()]);

		// disable slip fee
		assert_ok!(Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			false,
			FixedU128::from_rational(5, 100)
		));

		expect_events(vec![Event::SlipFactorUpdated {
			slip_factor: FixedU128::one(),
			max_slip_fee: FixedU128::from_rational(5, 100),
		}
		.into()]);

		// enable slip fee
		assert_ok!(Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			true,
			FixedU128::from_rational(1, 100)
		));

		expect_events(vec![Event::SlipFactorUpdated {
			slip_factor: FixedU128::one(),
			max_slip_fee: FixedU128::from_rational(1, 100),
		}
		.into()]);

		// modify slip fee
		assert_ok!(Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			true,
			FixedU128::from_rational(7, 100)
		));

		expect_events(vec![Event::SlipFactorUpdated {
			slip_factor: FixedU128::one(),
			max_slip_fee: FixedU128::from_rational(7, 100),
		}
		.into()]);
	});
}
