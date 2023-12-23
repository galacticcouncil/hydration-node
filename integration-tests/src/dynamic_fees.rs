#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use pallet_dynamic_fees::types::FeeEntry;
use primitives::AssetId;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const DOT_UNITS: u128 = 10_000_000_000;
const BTC_UNITS: u128 = 10_000_000;
const ETH_UNITS: u128 = 1_000_000_000_000_000_000;

#[test]
fn fees_should_work_when_oracle_not_initialized() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();

		let trader = DAVE;

		set_balance(trader.into(), DOT, 1_000 * DOT_UNITS as i128);

		assert!(hydradx_runtime::DynamicFees::current_fees(HDX).is_none());

		//Act
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		// Fees are not recalculated because nothing has  been provided by oracle ( it did not go through on init yet)
		assert!(hydradx_runtime::DynamicFees::current_fees(HDX).is_none());
		assert!(hydradx_runtime::DynamicFees::current_fees(DOT).is_none());
	});
}

#[test]
fn fees_should_initialize_lazily_to_min_value_when_first_trade_happens() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		assert!(hydradx_runtime::DynamicFees::current_fees(HDX).is_none());

		//Act
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let asset_fee_params = <hydradx_runtime::Runtime as pallet_dynamic_fees::Config>::AssetFeeParameters::get();

		//Assert
		assert_eq!(
			hydradx_runtime::DynamicFees::current_fees(HDX).unwrap(),
			FeeEntry {
				asset_fee: asset_fee_params.min_fee,
				protocol_fee: Permill::from_float(0.000788_f64),
				timestamp: 12_u32
			}
		);
	});
}

#[test]
fn fees_should_initialize_lazily_to_min_value_when_first_buy_happens() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		assert!(hydradx_runtime::DynamicFees::current_fees(HDX).is_none());

		set_balance(DAVE.into(), HDX, 1_000 * UNITS as i128);
		//Act
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		let asset_fee_params = <hydradx_runtime::Runtime as pallet_dynamic_fees::Config>::AssetFeeParameters::get();

		//Assert
		assert_eq!(
			hydradx_runtime::DynamicFees::current_fees(HDX).unwrap(),
			FeeEntry {
				asset_fee: asset_fee_params.min_fee,
				protocol_fee: Permill::from_float(0.000788_f64),
				timestamp: 12_u32
			}
		);
	});
}

#[test]
fn fees_should_change_when_buys_happen_in_different_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(10);

		set_balance(DAVE.into(), HDX, 1_000 * UNITS as i128);

		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		let old_fees = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();

		//Act
		hydradx_run_to_block(11);
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		//Assert
		let current_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_ne!(current_fee, old_fees);
		assert_eq!(
			current_fee,
			FeeEntry {
				asset_fee: Permill::from_float(0.0025_f64),
				protocol_fee: Permill::from_float(0.001_f64),
				timestamp: 11_u32
			}
		);
	});
}

#[test]
fn fees_should_change_when_sells_happen_in_different_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let old_fees = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();

		//Act
		hydradx_run_to_block(13);
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		//Assert
		let current_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_ne!(current_fee, old_fees);
		assert_eq!(
			current_fee,
			FeeEntry {
				asset_fee: Permill::from_float(0.0025_f64),
				protocol_fee: Permill::from_float(0.000926_f64),
				timestamp: 13_u32
			}
		);
	});
}

#[test]
fn fees_should_change_when_trades_happen_in_different_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let old_fees = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();

		//Act
		hydradx_run_to_block(13);
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		//Assert
		let current_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_ne!(current_fee, old_fees);
		assert_eq!(
			current_fee,
			FeeEntry {
				asset_fee: Permill::from_float(0.0025_f64),
				protocol_fee: Permill::from_float(0.000926_f64),
				timestamp: 13_u32
			}
		);
	});
}

#[test]
fn fees_should_change_only_one_when_trades_happen_in_the_same_block() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let old_fees = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		set_balance(DAVE.into(), HDX, 1_000 * UNITS as i128);

		//Act & assert
		hydradx_run_to_block(13);
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		let current_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_ne!(current_fee, old_fees);
		assert_eq!(
			current_fee,
			FeeEntry {
				asset_fee: Permill::from_float(0.0025_f64),
				protocol_fee: Permill::from_float(0.000926_f64),
				timestamp: 13_u32
			}
		);

		//NOTE: second trade in the same block should not change fees
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		assert_eq!(hydradx_runtime::DynamicFees::current_fees(HDX).unwrap(), current_fee);

		//NOTE: second trade in the same block should not change fees
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		assert_eq!(hydradx_runtime::DynamicFees::current_fees(HDX).unwrap(), current_fee);
	});
}

fn set_balance(who: hydradx_runtime::AccountId, currency: AssetId, amount: i128) {
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		who,
		currency,
		amount,
	));
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let eth_price = FixedU128::from_inner(71_145_071_145_071);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		ETH,
		eth_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let btc_price = FixedU128::from_inner(9_647_109_647_109_650_000_000_000);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		BTC,
		btc_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));
	set_zero_reward_for_referrals(HDX);
	set_zero_reward_for_referrals(DAI);
	set_zero_reward_for_referrals(DOT);
	set_zero_reward_for_referrals(ETH);
}

/// This function executes one sell and buy with HDX for all assets in the omnipool. This is necessary to
/// oracle have a prices for the assets.
/// NOTE: It's necessary to change parachain block to oracle have prices.
fn init_oracle() {
	let trader = DAVE;

	set_balance(trader.into(), HDX, 1_000 * UNITS as i128);
	set_balance(trader.into(), DOT, 1_000 * DOT_UNITS as i128);
	set_balance(trader.into(), ETH, 1_000 * ETH_UNITS as i128);
	set_balance(trader.into(), BTC, 1_000 * BTC_UNITS as i128);

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		2 * DOT_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		2 * DOT_UNITS,
		u128::MAX
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		2 * ETH_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		2 * ETH_UNITS,
		u128::MAX
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		2 * BTC_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		2 * BTC_UNITS,
		u128::MAX
	));
}
