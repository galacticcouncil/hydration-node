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
		.with_slip_fee()
		.build()
		.execute_with(|| {
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
					hub_reserve_at_block_start: 1560000000000000,
					current_delta_hub_reserve: Decrease(31836734693877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1300000000000000,
					current_delta_hub_reserve: Increase(31173469387755),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 45_764_362_220_058);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_336_734_693_878
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_954_235_637_779_942
			);

			assert_pool_state!(13359336734693878, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_954_235_637_779_942,
					hub_reserve: 1331173469387755,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50000000000000,
				amount_out: 45_764_362_220_058,
				hub_amount_in: 31836734693877,
				hub_amount_out: 31173469387755,
				asset_fee_amount: 0,
				protocol_fee_amount: 663265306122,
			}
			.into()]);
		});
}

#[test]
fn two_sells_in_one_direction_should_increase_fee() {
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
		.with_slip_fee()
		.build()
		.execute_with(|| {
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
					hub_reserve_at_block_start: 1560000000000000,
					current_delta_hub_reserve: Decrease(31836734693877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1300000000000000,
					current_delta_hub_reserve: Increase(31173469387755),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 45_764_362_220_058);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_336_734_693_878
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_954_235_637_779_942
			);

			assert_pool_state!(13359336734693878, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_954_235_637_779_942,
					hub_reserve: 1331173469387755,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50000000000000,
				amount_out: 45_764_362_220_058,
				hub_amount_in: 31836734693877,
				hub_amount_out: 31173469387755,
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
					hub_reserve_at_block_start: 1560000000000000,
					current_delta_hub_reserve: Decrease(62399999999999),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1300000000000000,
					current_delta_hub_reserve: Increase(60463265306122),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50_000_000_000_000,
				amount_out: 40_241_923_587_821,
				hub_amount_in: 30_563_265_306_122,
				hub_amount_out: 29_289_795_918_367,
				asset_fee_amount: 0,
				protocol_fee_amount: 1_273_469_387_755,
			}
			.into()]);
		});
}

#[test]
fn sell_and_buy_can_cancel_out_and_bring_slip_fee_to_initial_state() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, 200, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.with_slip_fee()
		.build()
		.execute_with(|| {
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
					hub_reserve_at_block_start: 1560000000000000,
					current_delta_hub_reserve: Decrease(31836734693877),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1300000000000000,
					current_delta_hub_reserve: Increase(31173469387755),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 550 * ONE);
			assert_eq!(Tokens::free_balance(200, &LP1), 1_045_764_362_220_058);
			assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				13_359_336_734_693_878
			);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1_954_235_637_779_942
			);

			assert_pool_state!(13359336734693878, 26_720 * ONE);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1_954_235_637_779_942,
					hub_reserve: 1331173469387755,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			expect_events(vec![Event::SellExecuted {
				who: LP1,
				asset_in: 100,
				asset_out: 200,
				amount_in: 50000000000000,
				amount_out: 45764362220058,
				hub_amount_in: 31836734693877,
				hub_amount_out: 31173469387755,
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
				amount_in: 47908947224606,
				amount_out: 50000000000000,
				hub_amount_in: 31853403580016,
				hub_amount_out: 31836734693878,
				asset_fee_amount: 0,
				protocol_fee_amount: 0,
			}
			.into()]);

			let hub_asset_block_state_in = Omnipool::hub_asset_block_state(100).unwrap();
			let hub_asset_block_state_out = Omnipool::hub_asset_block_state(200).unwrap();
			assert_eq!(
				hub_asset_block_state_in,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1560000000000000,
					current_delta_hub_reserve: Increase(1),
				}
			);
			assert_eq!(
				hub_asset_block_state_out,
				HubAssetBlockState::<Balance> {
					hub_reserve_at_block_start: 1300000000000000,
					current_delta_hub_reserve: Decrease(679934192261),
				}
			);
		});
}

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
			// assert_eq!(Tokens::free_balance(100, &LP1), 547598253275108);
			// assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			// assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			// assert_eq!(
			// 	Tokens::free_balance(100, &Omnipool::protocol_account()),
			// 	2452401746724892
			// );
			// assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1950 * ONE);
			//
			// assert_pool_state!(13_360 * ONE, 26_720 * ONE);

			// assert_asset_state!(
			// 	100,
			// 	AssetReserveState {
			// 		reserve: 2452401746724892,
			// 		hub_reserve: 1526666666666666,
			// 		shares: 2400 * ONE,
			// 		protocol_shares: Balance::zero(),
			// 		cap: DEFAULT_WEIGHT_CAP,
			// 		tradable: Tradability::default(),
			// 	}
			// );
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
