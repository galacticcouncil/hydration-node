use crate::tests::mock::*;
use crate::ARBITRAGE_DIRECTION_BUY;
use frame_support::assert_ok;
use hex_literal::hex;
use hydra_dx_math::hsm::PegType;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::stableswap::AssetAmount;
use num_traits::One;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use pallet_stableswap::types::PegSource;
use proptest::prelude::*;
use sp_runtime::{FixedPointNumber, FixedU128, Perbill, Permill};
use test_utils::assert_eq_approx;

#[test]
fn arbitrage_should_work() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		.with_pool(
			pool_id,
			vec![DAI, HOLLAR],
			22,
			Permill::from_percent(0),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 1_000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 900 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_percent(0),
			FixedU128::one(),
			Permill::from_float(0.),
			Perbill::from_percent(10),
		)
		.build()
		.execute_with(|| {
			move_block();
			let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
			assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));
			// Set HSM collateral holdings
			assert_ok!(Tokens::update_balance(DAI, &HSM::account_id(), 100 * ONE as i128));

			let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
			let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
			let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
			assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI, None));

			let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);
			let arb_amount = pool_balance_dai_after - pool_balance_dai_before;

			let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_balance_dai_before - hsm_balance_dai_after, arb_amount);
			// Check final HSM balance
			assert_eq!(hsm_balance_dai_after, 100 * ONE - arb_amount);
		});
}

#[test]
fn arbitrage_should_work_when_less_hollar_in_the_pool_and_arb_amount_given() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		.with_pool(
			pool_id,
			vec![DAI, HOLLAR],
			22,
			Permill::from_percent(0),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 999_000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1_000_000 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_float(0.),
			FixedU128::from_rational(99, 100),
			Permill::from_float(0.),
			Perbill::from_float(0.0001),
		)
		.build()
		.execute_with(|| {
			move_block();
			let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
			assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));

			let opportunity = HSM::find_arbitrage_opportunity(DAI).expect("No arbitrage opportunity");
			assert_eq!(opportunity, (ARBITRAGE_DIRECTION_BUY, 499994562497366512583));

			let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
			let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
			let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
			assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI, Some(opportunity.1)));
			let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);

			let arb_amount = pool_balance_dai_before - pool_balance_dai_after;
			assert_eq!(arb_amount, 500_005_437_502_633_106_476);

			let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_balance_dai_after - hsm_balance_dai_before, arb_amount);
			assert_eq!(hsm_balance_dai_after, arb_amount);
		});
}

#[test]
fn arbitrage_should_work_when_less_hollar_in_the_pool() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		.with_pool(
			pool_id,
			vec![DAI, HOLLAR],
			22,
			Permill::from_percent(0),
			vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: 999_000 * ONE,
				},
				AssetAmount {
					asset_id: DAI,
					amount: 1_000_000 * ONE,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_float(0.),
			FixedU128::from_rational(99, 100),
			Permill::from_float(0.),
			Perbill::from_float(0.0001),
		)
		.build()
		.execute_with(|| {
			move_block();
			let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
			assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));

			let opportunity = HSM::find_arbitrage_opportunity(DAI);
			assert_eq!(opportunity, Some((ARBITRAGE_DIRECTION_BUY, 499994562497366512583)));

			let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
			let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
			let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
			assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI, None));
			let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);

			let arb_amount = pool_balance_dai_before - pool_balance_dai_after;
			assert_eq!(arb_amount, 500_005_437_502_633_106_476);

			let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
			assert_eq!(hsm_balance_dai_after - hsm_balance_dai_before, arb_amount);
			assert_eq!(hsm_balance_dai_after, arb_amount);
		});
}

fn liquidity_ratio_strategy() -> impl Strategy<Value = FixedU128> {
	(0.5f64..1.5f64).prop_map(FixedU128::from_float)
}

fn peg() -> impl Strategy<Value = PegType> {
	(900u128..1100u128).prop_map(|v| (v, 1000u128))
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn test_arbitrage_opportunities(
		ratio in liquidity_ratio_strategy(),
		asset_peg in peg(),
	) {
		let pool_id = 100u32;

		let hollar_liquidity = 1_000_000 * ONE;
		let asset_liquidity = ratio.saturating_mul_int(hollar_liquidity);

		let buyback_speed = Perbill::from_float(0.0002);
		let amplification = 100;
		let pool_fee = Permill::from_float(0.001);
		let purchase_fee= Permill::from_float(0.);

		ExtBuilder::default()
			.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
			.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
			.with_pool(
				pool_id,
				vec![DAI, HOLLAR],
				amplification,
				pool_fee,
				vec![PegSource::Value(asset_peg), PegSource::Value((1, 1))],
			)
			.with_initial_pool_liquidity(
				100,
				vec![
					AssetAmount {
						asset_id: HOLLAR,
						amount: hollar_liquidity,
					},
					AssetAmount {
						asset_id: DAI,
						amount: asset_liquidity,
					},
				],
			)
			.with_collateral_buyback_limit(
				DAI,
				pool_id,
				purchase_fee,
				FixedU128::from_rational(99, 100),
				Permill::from_float(0.),
				buyback_speed,
			)
			.build()
			.execute_with(|| {
				move_block();
				let Some((direction, amount)) = HSM::find_arbitrage_opportunity(DAI) else{
					return;
				};

				if direction == ARBITRAGE_DIRECTION_BUY && amount > 0 {
					let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
					assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));

					let pool_acc = pallet_stableswap::Pallet::<Test>::pool_account(pool_id);
					let pool_balance_dai_before = Tokens::free_balance(DAI, &pool_acc);
					let hsm_balance_dai_before = Tokens::free_balance(DAI, &HSM::account_id());
					assert_ok!(HSM::execute_arbitrage(RuntimeOrigin::none(), DAI, None));
					let pool_balance_dai_after = Tokens::free_balance(DAI, &pool_acc);

					let arb_amount = pool_balance_dai_before - pool_balance_dai_after;
					assert!(arb_amount > 0 );

					let hsm_balance_dai_after = Tokens::free_balance(DAI, &HSM::account_id());
					assert_eq!(hsm_balance_dai_after - hsm_balance_dai_before, arb_amount);
					assert_eq!(hsm_balance_dai_after, arb_amount);

					let sell_price = hydra_dx_math::hsm::calculate_purchase_price(asset_peg, purchase_fee);
					let sell_price = FixedU128::from_rational(sell_price.0, sell_price.1);
					let state = pallet_stableswap::Pallet::<Test>::create_snapshot(pool_id).expect("Pool not found");

					let reserves = state
						.reserves
						.iter()
						.zip(state.assets.iter())
						.map(|(r, a)| ((*a), *r))
						.collect::<Vec<_>>();

					let after_spot = hydra_dx_math::stableswap::calculate_spot_price(
						pool_id,
						reserves,
						amplification as u128,
						HOLLAR,
						DAI,
						state.share_issuance,
						1_000_000_000_000_000_000_u128,
						Some(state.fee),
						&state.pegs,
					).expect("Pool not found");

					let after_spot = FixedU128::one().div(after_spot);

					assert_eq_approx!(sell_price,after_spot, FixedU128::from_float(0.01), "Price should converge");

				}
			});
	}
}

#[test]
fn find_opportunity() {
	let pool_id = 100u32;

	let hollar_liquidity = 1_000_000 * ONE;
	let asset_liquidity = 1391555_743691701874098498u128;

	let buyback_speed = Perbill::from_float(0.0002);
	let amplification = 100;
	let pool_fee = Permill::from_float(0.001);
	let asset_peg = PegSource::Value((1000000000000000, 1094970091267339));

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, DAI, 1_000 * ONE)])
		.with_registered_assets(vec![(DAI, 18), (HOLLAR, 18), (pool_id, 18)])
		.with_pool(
			pool_id,
			vec![DAI, HOLLAR],
			amplification,
			pool_fee,
			vec![asset_peg, PegSource::Value((1, 1))],
		)
		.with_initial_pool_liquidity(
			100,
			vec![
				AssetAmount {
					asset_id: HOLLAR,
					amount: hollar_liquidity,
				},
				AssetAmount {
					asset_id: DAI,
					amount: asset_liquidity,
				},
			],
		)
		.with_collateral_buyback_limit(
			DAI,
			pool_id,
			Permill::from_float(0.),
			FixedU128::from_rational(99, 100),
			Permill::from_float(0.),
			buyback_speed,
		)
		.build()
		.execute_with(|| {
			move_block();
			let flash_minter: EvmAddress = hex!["8F3aC7f6482ABc1A5c48a95D97F7A235186dBb68"].into();
			assert_ok!(HSM::set_flash_minter(RuntimeOrigin::root(), flash_minter,));
			let opportunity = HSM::find_arbitrage_opportunity(DAI);
			assert_eq!(opportunity, Some((1, 78321364099875978581618)));
		});
}
