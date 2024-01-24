#![cfg(test)]
#![allow(clippy::identity_op)]
use super::assert_balance;
use crate::polkadot_test_net::*;
use hydradx_adapters::OmnipoolHookAdapter;
use hydradx_runtime::{
	AssetRegistry, BlockNumber, Currencies, Omnipool, Router, RouterWeightInfo, Runtime, RuntimeOrigin, Stableswap,
	LBP, XYK,
};
use hydradx_traits::Registry;
use hydradx_traits::{
	router::{PoolType, Trade},
	AMM,
};
use pallet_lbp::weights::WeightInfo as LbpWeights;
use pallet_lbp::WeightCurveType;
use pallet_omnipool::traits::OmnipoolHooks;
use pallet_omnipool::types::Tradability;
use pallet_omnipool::weights::WeightInfo as OmnipoolWeights;
use pallet_route_executor::AmmTradeWeights;
use std::convert::Into;

use hydradx_traits::router::AssetPair as Pair;

use primitives::AssetId;

use frame_support::{assert_noop, assert_ok};
use xcm_emulator::TestExt;

use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use sp_runtime::{
	traits::{ConstU32, Zero},
	DispatchError, FixedU128, Permill,
};

use orml_traits::MultiCurrency;

pub const LBP_SALE_START: BlockNumber = 10;
pub const LBP_SALE_END: BlockNumber = 40;

#[test]
fn router_weights_should_be_non_zero() {
	assert!(!RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1).is_zero());
	assert!(!RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(1, 1).is_zero());

	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1).is_zero());
	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(1, 0).is_zero());
	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(2, 1).is_zero());
}

mod router_different_pools_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = UNITS / 100;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: LRNA,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 2_230_008_413_831;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (stable_pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
			create_lbp_pool(DAI, HDX);

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			create_xyk_pool(HDX, stable_asset_1);

			let amount_to_sell = UNITS / 100;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(stable_pool_id),
					asset_in: stable_asset_1,
					asset_out: stable_asset_2,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				stable_asset_2,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 2_783_595_233;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), stable_asset_1, 0);
			assert_balance!(BOB.into(), stable_asset_2, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: stable_asset_2,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: LRNA,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 4_370_898_989;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (stable_pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
			create_lbp_pool(DAI, HDX);

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			create_xyk_pool(HDX, stable_asset_1);

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(stable_pool_id),
					asset_in: stable_asset_1,
					asset_out: stable_asset_2,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				stable_asset_2,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 3_753_549_142_038;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), stable_asset_1, 0);
			assert_balance!(BOB.into(), stable_asset_2, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: stable_asset_2,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn router_should_work_for_hopping_from_omnipool_to_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				stable_asset_1,
				FixedU128::from_inner(25_650_000_000_000_000),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: stable_asset_2,
				},
			];

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_2,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
				ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
			);
		});
	}

	#[test]
	fn sell_router_should_add_liquidity_to_stableswap_when_selling_for_shareasset_in_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				stable_asset_1,
				FixedU128::from_inner(25_650_000_000_000_000),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				pool_id,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
				ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
			);

			assert_balance!(ALICE.into(), pool_id, 4638992258357);
		});
	}

	#[test]
	fn router_should_remove_liquidity_from_stableswap_when_selling_shareasset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;

			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_1,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_balance!(ALICE.into(), pool_id, 0);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(ALICE.into(), stable_asset_1, 2899390145403);
		});
	}

	#[test]
	fn buy_router_should_remove_liquidity_from_stableswap_when_asset_in_is_shareasset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_buy = UNITS / 1000;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_1,
				amount_to_buy,
				u128::MAX,
				trades
			));

			//Assert
			//assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_balance!(ALICE.into(), stable_asset_1, amount_to_buy);
		});
	}

	#[test]
	fn buy_router_should_add_liquidity_from_stableswap_when_asset_out_is_share_asset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: pool_id,
					asset_out: HDX,
				},
			];

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				HDX,
				amount_to_buy,
				u128::MAX,
				trades
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn trade_should_return_correct_weight() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: DAI,
					asset_out: ACA,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: ACA,
					asset_out: DOT,
				},
			];

			//Act & Assert
			assert_eq!(
				RouterWeightInfo::sell_weight(trades.as_slice()),
				hydradx_runtime::weights::omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 1)
					.checked_add(&<OmnipoolHookAdapter<
						RuntimeOrigin,
						ConstU32<HDX>,
						ConstU32<LRNA>,
						Runtime,
					> as OmnipoolHooks::<RuntimeOrigin, AccountId, AssetId, Balance>>::on_trade_weight(
					))
					.unwrap()
					.checked_add(&<OmnipoolHookAdapter<
						RuntimeOrigin,
						ConstU32<HDX>,
						ConstU32<LRNA>,
						Runtime,
					> as OmnipoolHooks::<RuntimeOrigin, AccountId, AssetId, Balance>>::on_liquidity_changed_weight(
					))
					.unwrap()
					.checked_add(&hydradx_runtime::weights::lbp::HydraWeight::<Runtime>::router_execution_sell(1, 1))
					.unwrap()
					.checked_add(
						&RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1)
							.checked_mul(2)
							.unwrap()
					)
					.unwrap()
			);
			assert_eq!(
				RouterWeightInfo::buy_weight(trades.as_slice()),
				hydradx_runtime::weights::omnipool::HydraWeight::<Runtime>::router_execution_buy(1, 1)
					.checked_add(&<OmnipoolHookAdapter<
						RuntimeOrigin,
						ConstU32<HDX>,
						ConstU32<LRNA>,
						Runtime,
					> as OmnipoolHooks::<RuntimeOrigin, AccountId, AssetId, Balance>>::on_trade_weight(
					))
					.unwrap()
					.checked_add(&<OmnipoolHookAdapter<
						RuntimeOrigin,
						ConstU32<HDX>,
						ConstU32<LRNA>,
						Runtime,
					> as OmnipoolHooks::<RuntimeOrigin, AccountId, AssetId, Balance>>::on_liquidity_changed_weight(
					))
					.unwrap()
					.checked_add(&hydradx_runtime::weights::lbp::HydraWeight::<Runtime>::router_execution_buy(1, 1))
					.unwrap()
					.checked_add(
						&RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1)
							.checked_mul(2)
							.unwrap()
					)
					.unwrap()
			);
		});
	}
}

mod omnipool_router_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 266_195_070_030_573_798;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_hub_asset_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				LRNA,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 220_685_840_707_964_601_769;

			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: LRNA,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn direct_sell_should_yield_the_same_result_as_router() {
		TestNet::reset();

		let amount_to_sell = 10 * UNITS;
		let limit = 0;
		let amount_out = 266_195_070_030_573_798;

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			//Act
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
			));

			//Assert
			expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				hub_amount_in: 12014871681,
				hub_amount_out: 12008864246,
				asset_fee_amount: 667_155_563_986_401,
				protocol_fee_amount: 6_007_435,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 37_565_544;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_hub_asset_should_not_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: LRNA,
			}];

			//Act & Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_buy,
					limit,
					trades
				),
				pallet_omnipool::Error::<Runtime>::NotAllowed
			);
		});
	}

	#[test]
	fn direct_buy_should_yield_the_same_result_as_router() {
		TestNet::reset();

		let amount_to_buy = UNITS;
		let limit = 100 * UNITS;
		let amount_in = 37_565_544;

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			//Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
			));

			//Assert
			expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				hub_amount_in: 45135,
				hub_amount_out: 45113,
				asset_fee_amount: 2_506_265_665,
				protocol_fee_amount: 22,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn trade_should_fail_when_asset_is_not_in_omnipool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: DAI,
				asset_out: ACA,
			}];

			//Act & Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					ACA,
					amount_to_sell,
					limit,
					trades
				),
				pallet_omnipool::Error::<Runtime>::AssetNotFound
			);
		});
	}
}

mod lbp_router_tests {
	use super::*;
	use crate::assert_balance;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 5_304_848_794_461;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_selling_distributed_asset_in_a_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 15_853_064_919_440;

			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: HDX,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_route_contains_double_trades_with_selling_accumulated_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			create_lbp_pool(DAI, DOT);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 2_894_653_623_153;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_route_contains_double_trades_with_selling_distributed_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 23_648_944_192_390;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn lbp_direct_sell_should_yield_the_same_result_as_router_sell() {
		TestNet::reset();

		let amount_to_sell = 10 * UNITS;
		let limit = 0;
		let received_amount_out = 5_304_848_794_461;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + received_amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out: received_amount_out,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			//Act
			assert_ok!(LBP::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit
			));

			//Assert
			expect_hydra_events(vec![pallet_lbp::Event::SellExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount: 9_980_000_000_000,
				sale_price: received_amount_out,
				fee_asset: HDX,
				fee_amount: 20_000_000_000,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + received_amount_out);
		});
	}

	#[test]
	fn buy_should_work_when_when_buying_distributed_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 19_944_391_321_918;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_buying_accumulated_asset_in_a_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 6_045_520_997_664;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_to_buy);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE - amount_in);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: HDX,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_having_double_trades_with_buying_distributed_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			create_lbp_pool(DAI, DOT);
			start_lbp_campaign();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 3_244_461_218_396;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_having_double_trades_with_buying_accumulated_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);
			start_lbp_campaign();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 322_733_757_240;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn lbp_direct_buy_should_yield_the_same_result_as_router_buy() {
		TestNet::reset();

		let amount_to_buy = 10 * UNITS;
		let limit = 100 * UNITS;
		let spent_amount_in = 19_944_391_321_918;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: spent_amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			//Act
			assert_ok!(LBP::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn trade_should_fail_when_asset_is_not_in_lbp() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: ACA,
			}];

			//Act & Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					ACA,
					amount_to_sell,
					limit,
					trades
				),
				pallet_lbp::Error::<Runtime>::PoolNotFound
			);
		});
	}
}

mod xyk_router_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 4_531_818_181_819_u128;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_route_contains_multiple_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, LRNA);
			create_xyk_pool(LRNA, DAI);
			create_xyk_pool(DAI, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: LRNA,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 1_054_553_059_484_u128;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_fail_when_there_is_no_pool_for_specific_asset_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::TokenPoolNotFound
			);
		});
	}

	#[test]
	fn sell_should_fail_when_balance_is_not_sufficient() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = 9999 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell * UNITS,
					0,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InsufficientBalance
			);
		});
	}

	#[test]
	fn sell_should_fail_when_trading_limit_is_below_minimum() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = hydradx_runtime::MinTradingLimit::get() - 1;
			let limit = 0;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientTradingAmount
			);
		});
	}

	#[test]
	fn sell_should_fail_when_buying_more_than_max_in_ratio_out() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = 1000 * UNITS;
			let limit = 0;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::MaxInRatioExceeded
			);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_buy = 10 * UNITS;
			let limit = 30 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 25075000000001;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_two_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);
			create_xyk_pool(DOT, DAI);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);

			let amount_to_buy = UNITS;
			let limit = 10 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: DAI,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 4_281_435_927_986;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_multiple_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);
			create_xyk_pool(DOT, LRNA);
			create_xyk_pool(LRNA, DAI);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);

			let amount_to_buy = UNITS;
			let limit = 10 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: LRNA,
					asset_out: DAI,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 9_392_858_946_762;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_fail_when_there_is_no_pool_for_specific_asset_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::TokenPoolNotFound
			);
		});
	}

	#[test]
	fn buy_should_fail_when_balance_is_not_sufficient() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(DOT, HDX);

			assert_balance!(BOB.into(), DOT, 0);
			let amount_to_buy = 10 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: DOT,
				asset_out: HDX,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					DOT,
					HDX,
					amount_to_buy,
					150 * UNITS,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientAssetBalance
			);
		});
	}

	#[test]
	fn buy_should_fail_when_trading_limit_is_below_minimum() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = hydradx_runtime::MinTradingLimit::get() - 1;
			let limit = 100 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_buy,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientTradingAmount
			);
		});
	}

	#[test]
	fn buy_should_fail_when_buying_more_than_max_ratio_out() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = 20 * UNITS;
			let limit = 100 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_buy,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::MaxOutRatioExceeded
			);
		});
	}
}

mod omnipool_stableswap_router_tests {
	use super::*;

	#[test]
	fn sell_single_router_should_add_liquidity_to_stableswap_when_asset_out_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: stable_asset_1,
				asset_out: pool_id,
			}];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				pool_id,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)),
				3000 * UNITS - amount_to_sell
			);
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_out_is_share_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: stable_asset_1,
				asset_out: pool_id,
			}];

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				pool_id,
				amount_to_buy,
				u128::MAX,
				trades
			));
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_in_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: pool_id,
				asset_out: stable_asset_1,
			}];

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				pool_id,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				stable_asset_1,
				amount_to_buy,
				u128::MAX,
				trades
			));
		});
	}
}

mod set_route {
	use super::*;
	use frame_support::storage::with_transaction;
	use hydradx_traits::router::inverse_route;
	use hydradx_traits::router::PoolType;
	use sp_runtime::TransactionOutcome;

	#[test]
	fn set_route_should_work_with_all_pools_involved() {
		{
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				let (pool_id, stable_asset_1, _) =
					init_stableswap_with_liquidity(1_000_000_000_000_000_000u128, 300_000_000_000_000_000u128).unwrap();

				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					60000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, stable_asset_1, 20000 * UNITS);

				create_lbp_pool_with_amounts(DOT, 1000000 * UNITS, stable_asset_1, 20000 * UNITS);
				//Start lbp campaign
				set_relaychain_block_number(LBP_SALE_START + 15);

				let route1 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_1,
						asset_out: DOT,
					},
				];

				let route2_cheaper = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::LBP,
						asset_in: stable_asset_1,
						asset_out: DOT,
					},
				];

				let asset_pair = Pair::new(HDX, DOT);

				//Verify if the cheaper route is indeed cheaper in both ways
				let amount_to_sell = 100 * UNITS;

				//Check for normal route
				let dot_amount_out = with_transaction::<_, _, _>(|| {
					assert_ok!(Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						HDX,
						DOT,
						amount_to_sell,
						0,
						route1.clone()
					));
					let alice_received_dot =
						Currencies::free_balance(DOT, &AccountId::from(ALICE)) - ALICE_INITIAL_DOT_BALANCE;

					TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
				})
				.unwrap();

				//Check for normal route
				let dot_amout_out_for_cheaper_route = with_transaction::<_, _, _>(|| {
					assert_ok!(Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						HDX,
						DOT,
						amount_to_sell,
						0,
						route2_cheaper.clone()
					));
					let alice_received_dot =
						Currencies::free_balance(DOT, &AccountId::from(ALICE)) - ALICE_INITIAL_DOT_BALANCE;

					TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
				})
				.unwrap();

				assert!(dot_amout_out_for_cheaper_route > dot_amount_out);

				// Check for inverse route
				let amount_out_for_inverse = with_transaction::<_, _, _>(|| {
					let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
					assert_ok!(Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						DOT,
						HDX,
						amount_to_sell,
						0,
						inverse_route(route1.clone())
					));
					let alice_received_hdx = Currencies::free_balance(HDX, &AccountId::from(ALICE)) - alice_hdx_balance;

					TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_hdx))
				})
				.unwrap();

				let amount_out_for_inverse_with_chaper_route = with_transaction::<_, _, _>(|| {
					let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
					assert_ok!(Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						DOT,
						HDX,
						amount_to_sell,
						0,
						inverse_route(route2_cheaper.clone())
					));
					let alice_received_hdx = Currencies::free_balance(HDX, &AccountId::from(ALICE)) - alice_hdx_balance;

					TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_hdx))
				})
				.unwrap();

				assert!(amount_out_for_inverse_with_chaper_route > amount_out_for_inverse);

				//ACT AND ASSERT

				//We set first the more expensive route
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), route1);

				//We set the cheaper one so it should replace
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route2_cheaper.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), route2_cheaper);

				//We try to set back the more expensive but did not replace
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
				);
				assert_eq!(Router::route(asset_pair).unwrap(), route2_cheaper);
			});
		}
	}

	#[test]
	fn set_route_should_fail_with_invalid_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(ETH, 1000 * UNITS, BTC, 1000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: ETH,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			//Act and assert
			assert_noop!(
				Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
			);
		});
	}

	#[test]
	fn set_route_should_fail_with_trying_to_override_with_invalid_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 1000 * UNITS, BTC, 1000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			//Act and assert
			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			));

			create_xyk_pool_with_amounts(ETH, 1000 * UNITS, BTC, 1000 * UNITS);

			let invalid_route = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: ETH,
					asset_out: BTC,
				},
			];
			assert_noop!(
				Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					invalid_route
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
			);
		});
	}

	#[test]
	fn set_route_should_work_when_stored_route_is_broken_due_to_not_existing_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				1000000000000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 50000 * UNITS, BTC, 4000000 * UNITS);

			let asset_pair = Pair::new(HDX, BTC);

			let route2 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			//Act and assert
			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route2
			),);
		});
	}

	#[test]
	fn set_route_should_not_work_when_no_existing_and_reversed_route_is_not_valid_for_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
			create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, BTC, 1000000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			));

			assert_ok!(XYK::remove_liquidity(
				RuntimeOrigin::signed(DAVE.into()),
				HDX,
				DOT,
				1000000 * UNITS
			));

			let route2 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			//Act and assert
			assert_noop!(
				Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route2),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
			);
		});
	}

	#[test]
	fn set_route_should_not_work_when_reversed_route_is_not_valid_due_to_maxout_ratio() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
			create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, BTC, 1000000 * UNITS);

			create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DAI, 1000000 * UNITS);
			create_xyk_pool_with_amounts(DAI, 1000000 * UNITS, DOT, 1000000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			));

			let route2 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			//Act and assert
			assert_noop!(
				Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route2),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
			);
		});
	}

	#[test]
	fn set_route_should_work_when_stored_route_is_broken_due_to_frozen_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				1000000000000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
			create_xyk_pool_with_amounts(DOT, 50000 * UNITS, BTC, 4000000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			));

			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				DOT,
				Tradability::FROZEN
			));

			let route2 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			//Act and assert
			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route2
			),);
		});
	}

	#[test]
	fn set_should_should_work_when_omnipool_route_does_not_exist_for_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
			create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, BTC, 1000000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: BTC,
				},
			];

			let asset_pair = Pair::new(HDX, BTC);

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			));
		});
	}

	#[test]
	fn set_route_should_not_work_when_setting_default_omni_route_again() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				1000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			let route1 = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DOT,
			}];

			let asset_pair = Pair::new(HDX, DOT);

			assert_noop!(
				Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
			);
		});
	}

	#[test]
	fn set_route_should_not_work_when_existing_inverse_is_broken() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				DOT,
				1000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 100000 * UNITS, HDX, 100000 * UNITS);
			create_xyk_pool_with_amounts(HDX, 100000 * UNITS, BTC, 100000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: BTC,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			let asset_pair = Pair::new(BTC, DOT);

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1
			),);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				900 * UNITS,
				Balance::MAX,
			));

			let route1 = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: BTC,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			assert_noop!(
				Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
			);
		});
	}
}

mod with_on_chain_and_default_route {
	use super::*;

	#[test]
	fn buy_should_work_with_onchain_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: stable_asset_1,
					asset_out: DOT,
				},
			];

			let asset_pair = Pair::new(HDX, DOT);
			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1.clone()
			));
			assert_eq!(Router::route(asset_pair).unwrap(), route1);

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				HDX,
				100000 * UNITS as i128,
			));

			assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE);
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				amount_to_buy,
				u128::MAX,
				vec![],
			));

			assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn sell_should_work_with_onchain_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: stable_asset_1,
					asset_out: DOT,
				},
			];

			let asset_pair = Pair::new(HDX, DOT);
			let amount_to_sell = 100 * UNITS;

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1.clone()
			));
			assert_eq!(Router::route(asset_pair).unwrap(), route1);

			//Act
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				amount_to_sell,
				0,
				vec![],
			));

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
		});
	}

	#[test]
	fn sell_should_work_with_onchain_route_but_used_in_reversed_order() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_rational(1, 2),
				Permill::from_percent(1),
				AccountId::from(BOB),
			));

			create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

			let route1 = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: stable_asset_1,
					asset_out: DOT,
				},
			];

			let asset_pair = Pair::new(HDX, DOT);
			let amount_to_sell = 100 * UNITS;

			assert_ok!(Router::set_route(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset_pair,
				route1.clone()
			));
			assert_eq!(Router::route(asset_pair).unwrap(), route1);

			//Act
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				DOT,
				HDX,
				amount_to_sell,
				0,
				vec![],
			));

			assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE - amount_to_sell);
		});
	}

	#[test]
	fn sell_should_work_with_default_omnipool_when_no_onchain_or_specified_route_present() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				vec![]
			));

			//Assert
			let amount_out = 266_195_070_030_573_798;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_default_omni_route_when_no_onchain_or_specified_route_present() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				vec![]
			));

			//Assert
			let amount_in = 37_565_544;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}
}

fn create_lbp_pool(accumulated_asset: u32, distributed_asset: u32) {
	assert_ok!(LBP::create_pool(
		RuntimeOrigin::root(),
		ALICE.into(),
		accumulated_asset,
		100 * UNITS,
		distributed_asset,
		200 * UNITS,
		20_000_000,
		80_000_000,
		WeightCurveType::Linear,
		(2, 1_000),
		CHARLIE.into(),
		0,
	));

	let account_id = get_lbp_pair_account_id(accumulated_asset, distributed_asset);

	assert_ok!(LBP::update_pool_data(
		RuntimeOrigin::signed(ALICE.into()),
		account_id,
		None,
		Some(LBP_SALE_START),
		Some(LBP_SALE_END),
		None,
		None,
		None,
		None,
		None,
	));
}

fn create_lbp_pool_with_amounts(accumulated_asset: u32, amount_a: u128, distributed_asset: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		accumulated_asset,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		distributed_asset,
		amount_b as i128,
	));

	assert_ok!(LBP::create_pool(
		RuntimeOrigin::root(),
		DAVE.into(),
		accumulated_asset,
		amount_a,
		distributed_asset,
		amount_b,
		20_000_000,
		80_000_000,
		WeightCurveType::Linear,
		(2, 1_000),
		CHARLIE.into(),
		0,
	));

	let account_id = get_lbp_pair_account_id(accumulated_asset, distributed_asset);

	assert_ok!(LBP::update_pool_data(
		RuntimeOrigin::signed(DAVE.into()),
		account_id,
		None,
		Some(LBP_SALE_START),
		Some(LBP_SALE_END),
		None,
		None,
		None,
		None,
		None,
	));
}

fn get_lbp_pair_account_id(asset_a: AssetId, asset_b: AssetId) -> AccountId {
	let asset_pair = pallet_lbp::AssetPair {
		asset_in: asset_a,
		asset_out: asset_b,
	};
	LBP::get_pair_id(asset_pair)
}

fn start_lbp_campaign() {
	set_relaychain_block_number(LBP_SALE_START + 1);
}

fn create_xyk_pool(asset_a: u32, asset_b: u32) {
	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(ALICE.into()),
		asset_a,
		100 * UNITS,
		asset_b,
		50 * UNITS,
	));
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	init_stableswap_with_liquidity(initial_liquidity, liquidity_added)
}

pub fn init_stableswap_with_liquidity(
	initial_liquidity: Balance,
	liquidity_added: Balance,
) -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let mut initial: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> =
		vec![];

	let mut asset_ids: Vec<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = AssetRegistry::create_asset(&name, 1u128)?;
		AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), asset_id, b"xDUM".to_vec(), 18u8)?;
		asset_ids.push(asset_id);

		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		)?;
		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(CHARLIE),
			asset_id,
			initial_liquidity as i128,
		)?;

		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let pool_id = AssetRegistry::create_asset(&b"pool".to_vec(), 1u128)?;

	let amplification = 100u16;
	let fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		pool_id,
		asset_ids,
		amplification,
		fee,
	)?;

	Stableswap::add_liquidity(hydradx_runtime::RuntimeOrigin::signed(BOB.into()), pool_id, initial)?;

	Ok((pool_id, asset_in, asset_out))
}
