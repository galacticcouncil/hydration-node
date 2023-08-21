#![cfg(test)]

use crate::assert_balance;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Currencies;
use hydradx_runtime::Omnipool;
use hydradx_runtime::Router;
use hydradx_runtime::Stableswap;
use hydradx_traits::router::PoolType;
use hydradx_traits::Registry;
use orml_traits::MultiCurrency;
use pallet_route_executor::Trade;
use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use sp_runtime::Permill;
use sp_runtime::{DispatchError, FixedU128};
use xcm_emulator::TestExt;

//NOTE: XYK pool is not supported in HydraDX. If you want to support it, also adjust router and dca benchmarking
#[test]
fn router_should_not_support_xyk() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let trades = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				trades.clone()
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);

		assert_noop!(
			Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				u128::MAX,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

//NOTE: LBP pool is not supported in HydraDX. If you want to support it, also adjust router and dca benchmarking
#[test]
fn router_should_not_support_lbp() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let trades = vec![Trade {
			pool: PoolType::LBP,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				trades.clone()
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);

		assert_noop!(
			Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				u128::MAX,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

#[test]
fn router_should_work_with_only_omnipool() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: DAI,
		}];

		//ACt
		let amount_to_sell = 100 * UNITS;
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DAI,
			amount_to_sell,
			0,
			trades
		),);

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
		);
	});
}

#[test]
fn router_should_work_for_hopping_from_omniool_to_stableswap() {
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
			FixedU128::from_inner(25_650_000_000_000_000_000),
			Permill::from_percent(100),
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
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
		);
	});
}

#[test]
fn router_should_add_liquidity_to_stableswap_when_wanting_shareasset_in_stableswap() {
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
			FixedU128::from_inner(25_650_000_000_000_000_000),
			Permill::from_percent(100),
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
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
		);

		assert_balance!(ALICE.into(), pool_id, 4669657738);
	});
}

#[test]
fn router_should_remove_liquidity_from_stableswap_when_selling_shareasset_in_stable() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

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
			FixedU128::from_inner(25_650_000_000_000_000_000),
			Permill::from_percent(100),
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
		assert_balance!(ALICE.into(), stable_asset_1, 2903943404);
	});
}

#[test]
fn stableswap_buy_is_not_supported_when_asset_in_is_shareasset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

		let trades = vec![Trade {
			pool: PoolType::Stableswap(pool_id),
			asset_in: pool_id,
			asset_out: stable_asset_1,
		}];

		//Act and assert
		let amount_to_buy = 1 * UNITS / 1000;

		assert_noop!(
			Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_1,
				amount_to_buy,
				u128::MAX,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

#[test]
fn stableswap_buy_is_not_supported_when_asset_out_is_shareasset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

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

		//Act and assert
		let amount_to_buy = 1 * UNITS / 1000;

		assert_noop!(
			Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				pool_id,
				amount_to_buy,
				u128::MAX,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

pub fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
		hydradx_runtime::RuntimeOrigin::root(),
		u128::MAX,
	));

	assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(10)
	));
}

pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	let mut initial: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> =
		vec![];

	let mut asset_ids: Vec<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		//let asset_id = regi_asset(name.clone(), 1_000_000, 10000 + idx as u32)?;
		let asset_id = AssetRegistry::create_asset(&name, 1u128)?;
		AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), asset_id, b"xDUM".to_vec(), 18u8)?;
		asset_ids.push(asset_id);
		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(BOB.clone()),
			asset_id,
			1_000_000_000_000_000i128,
		)?;
		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(CHARLIE.clone()),
			asset_id,
			1_000_000_000_000_000_000_000i128,
		)?;
		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let pool_id = AssetRegistry::create_asset(&b"pool".to_vec(), 1u128)?;

	let amplification = 100u16;
	let trade_fee = Permill::from_percent(1);
	let withdraw_fee = Permill::from_percent(1);

	let asset_in: AssetId = *asset_ids.last().unwrap();
	let asset_out: AssetId = *asset_ids.first().unwrap();

	Stableswap::create_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		pool_id,
		asset_ids,
		amplification,
		trade_fee,
		withdraw_fee,
	)?;

	Stableswap::add_liquidity(hydradx_runtime::RuntimeOrigin::signed(BOB.into()), pool_id, initial)?;

	Ok((pool_id, asset_in, asset_out))
}
