#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_traits::fee::GetDynamicFee;
use pallet_dynamic_fees::types::FeeEntry;
use pallet_dynamic_fees::UpdateAndRetrieveFees;
use primitives::AssetId;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const DOT_UNITS: u128 = 10_000_000_000;
const BTC_UNITS: u128 = 1_000_000;
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
fn fees_should_change_when_buys_happen_in_different_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

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
				asset_fee: Permill::from_float(0.05),
				protocol_fee: Permill::from_float(0.0005),
				timestamp: 13_u32
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
				asset_fee: Permill::from_float(0.05),
				protocol_fee: Permill::from_float(0.0005),
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
				asset_fee: Permill::from_float(0.05),
				protocol_fee: Permill::from_float(0.0005),
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
				asset_fee: Permill::from_float(0.05),
				protocol_fee: Permill::from_float(0.0005),
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
	set_zero_reward_for_referrals(BTC);
}

/// This function executes one sell and buy with HDX for all assets in the omnipool. This is necessary to
/// oracle have a prices for the assets.
/// NOTE: It's necessary to change parachain block to oracle have prices.
fn init_oracle() {
	let trader = DAVE;

	set_balance(trader.into(), HDX, 10_000_000 * UNITS as i128);
	set_balance(trader.into(), DOT, 1_000 * DOT_UNITS as i128);
	set_balance(trader.into(), ETH, 1_000 * ETH_UNITS as i128);
	set_balance(trader.into(), BTC, 1_000 * BTC_UNITS as i128);

	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		20 * DOT_UNITS,
		0,
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		DAI,
		20 * DOT_UNITS,
		0,
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		20 * DOT_UNITS,
		u128::MAX
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		2 * ETH_UNITS,
		0,
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		ETH_UNITS,
		u128::MAX
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		2 * BTC_UNITS,
		0,
	));
	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		BTC_UNITS,
		u128::MAX
	));
	hydradx_run_to_next_block();
}

#[test]
fn test_fees_update_in_multi_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_next_block();
		hydradx_run_to_next_block();

		let hdx_state = hydradx_runtime::Omnipool::load_asset_state(HDX).unwrap();
		let dai_state = hydradx_runtime::Omnipool::load_asset_state(DAI).unwrap();
		let dot_state = hydradx_runtime::Omnipool::load_asset_state(DOT).unwrap();
		let eth_state = hydradx_runtime::Omnipool::load_asset_state(ETH).unwrap();
		let btc_state = hydradx_runtime::Omnipool::load_asset_state(BTC).unwrap();

		let hdx_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		let dai_fee = hydradx_runtime::DynamicFees::current_fees(DAI).unwrap();
		let dot_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		let eth_fee = hydradx_runtime::DynamicFees::current_fees(ETH).unwrap();
		let btc_fee = hydradx_runtime::DynamicFees::current_fees(BTC).unwrap();

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.04364));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0015));

		assert_eq!(hdx_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dai_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dot_fee.protocol_fee, Permill::from_float(0.00108));
		assert_eq!(eth_fee.protocol_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.protocol_fee, Permill::from_float(0.000665));

		//ACT
		let hdx_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((HDX, hdx_state.reserve));
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		let dot_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DOT, dot_state.reserve));
		let eth_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((ETH, eth_state.reserve));
		let btc_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((BTC, btc_state.reserve));

		//ASSERT
		assert_eq!(hdx_final_fees, (Permill::from_float(0.05), Permill::from_float(0.0005)));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.002888), Permill::from_float(0.0005))
		);
		assert_eq!(
			dot_final_fees,
			(Permill::from_float(0.0015), Permill::from_float(0.001524))
		);
		assert_eq!(
			eth_final_fees,
			(Permill::from_float(0.0015), Permill::from_float(0.0025))
		);
		assert_eq!(btc_final_fees, (Permill::from_float(0.0015), Permill::from_parts(778)));

		let dai_state = hydradx_runtime::Omnipool::load_asset_state(DAI).unwrap();

		hydradx_run_to_next_block();

		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.00291), Permill::from_float(0.0005))
		);

		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.002918), Permill::from_float(0.0005))
		);

		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.002914), Permill::from_float(0.0005))
		);

		hydradx_run_to_next_block();
		hydradx_run_to_next_block();
		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.002854), Permill::from_float(0.0005))
		);
	});
}

#[test]
fn test_fees_update_after_selling_lrna_in_multi_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_next_block();
		hydradx_run_to_next_block();

		let hdx_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		let dai_fee = hydradx_runtime::DynamicFees::current_fees(DAI).unwrap();
		let dot_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		let eth_fee = hydradx_runtime::DynamicFees::current_fees(ETH).unwrap();
		let btc_fee = hydradx_runtime::DynamicFees::current_fees(BTC).unwrap();

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.04364));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0015));

		assert_eq!(hdx_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dai_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dot_fee.protocol_fee, Permill::from_float(0.00108));
		assert_eq!(eth_fee.protocol_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.protocol_fee, Permill::from_float(0.000665));

		//ACT
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			LRNA,
			DAI,
			2 * UNITS,
			0,
		));

		hydradx_run_to_next_block();
		let dai_state = hydradx_runtime::Omnipool::load_asset_state(DAI).unwrap();
		let dai_fee = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get_and_store((DAI, dai_state.reserve));
		//ASSERT
		assert_eq!(
			(dai_fee.0, dai_fee.1),
			(Permill::from_float(0.003199), Permill::from_float(0.0005))
		);
	});
}

#[test]
fn test_fees_update_after_buying_with_lrna_in_multi_blocks() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_next_block();
		hydradx_run_to_next_block();

		let hdx_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		let dai_fee = hydradx_runtime::DynamicFees::current_fees(DAI).unwrap();
		let dot_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		let eth_fee = hydradx_runtime::DynamicFees::current_fees(ETH).unwrap();
		let btc_fee = hydradx_runtime::DynamicFees::current_fees(BTC).unwrap();

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.04364));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0015));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0015));

		assert_eq!(hdx_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dai_fee.protocol_fee, Permill::from_float(0.0005));
		assert_eq!(dot_fee.protocol_fee, Permill::from_float(0.00108));
		assert_eq!(eth_fee.protocol_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.protocol_fee, Permill::from_float(0.000665));

		//ACT
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DAI,
			LRNA,
			2 * UNITS,
			u128::MAX,
		));

		hydradx_run_to_next_block();
		let dai_state = hydradx_runtime::Omnipool::load_asset_state(DAI).unwrap();
		let dai_fee = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get_and_store((DAI, dai_state.reserve));
		//ASSERT
		assert_eq!(
			(dai_fee.0, dai_fee.1),
			(Permill::from_float(0.003031), Permill::from_float(0.0005))
		);
	});
}
