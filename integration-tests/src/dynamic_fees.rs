#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_traits::fee::GetDynamicFee;
use orml_traits::MultiCurrency;
use pallet_dynamic_fees::types::FeeEntry;
use pallet_dynamic_fees::UpdateAndRetrieveFees;
use primitives::AssetId;
use sp_runtime::{FixedU128, Permill};
use test_utils::assert_eq_approx;
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

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.044636));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0025));

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
			(Permill::from_float(0.003886), Permill::from_float(0.0005))
		);
		assert_eq!(
			dot_final_fees,
			(Permill::from_float(0.0025), Permill::from_float(0.001524))
		);
		assert_eq!(
			eth_final_fees,
			(Permill::from_float(0.0025), Permill::from_float(0.0025))
		);
		assert_eq!(btc_final_fees, (Permill::from_float(0.0025), Permill::from_parts(778)));

		let dai_state = hydradx_runtime::Omnipool::load_asset_state(DAI).unwrap();

		hydradx_run_to_next_block();

		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.003908), Permill::from_float(0.0005))
		);

		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.003916), Permill::from_float(0.0005))
		);

		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));

		assert_eq_approx!(
			dai_final_fees.0,
			Permill::from_float(0.003912),
			Permill::from_float(0.000001),
			"Final fee is not correct"
		);
		assert_eq!(dai_final_fees.1, Permill::from_float(0.0005));

		hydradx_run_to_next_block();
		hydradx_run_to_next_block();
		hydradx_run_to_next_block();
		let dai_final_fees = UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));
		assert_eq!(
			dai_final_fees,
			(Permill::from_float(0.003852), Permill::from_float(0.0005))
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

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.044636));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0025));

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
			(Permill::from_float(0.004196), Permill::from_float(0.0005))
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

		assert_eq!(hdx_fee.asset_fee, Permill::from_float(0.044636));
		assert_eq!(dai_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(dot_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(eth_fee.asset_fee, Permill::from_float(0.0025));
		assert_eq!(btc_fee.asset_fee, Permill::from_float(0.0025));

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
			(Permill::from_float(0.004028), Permill::from_float(0.0005))
		);
	});
}

#[test]
fn fees_should_work_when_min_equals_max_in_dynamic_config() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		let fixed_asset_fee = Permill::from_float(0.003);
		let fixed_protocol_fee = Permill::from_float(0.001);

		// Set dynamic fee config with min = max for DOT
		// This effectively creates a "fixed" fee but using the dynamic mechanism
		assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			pallet_dynamic_fees::types::AssetFeeConfig::Dynamic {
				asset_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_asset_fee,
					max_fee: fixed_asset_fee, // Same as min - this is what we're testing
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
				protocol_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_protocol_fee,
					max_fee: fixed_protocol_fee, // Same as min - this is what we're testing
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
			},
		));

		set_balance(DAVE.into(), HDX, 10_000 * UNITS as i128);

		//Act - Execute a sell trade
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		//Assert - Verify fees are set and equal to the fixed values
		let current_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		assert_eq!(current_fee.asset_fee, fixed_asset_fee);
		assert_eq!(current_fee.protocol_fee, fixed_protocol_fee);

		//Act - Execute a buy trade in next block
		hydradx_run_to_block(13);
		assert_ok!(hydradx_runtime::Omnipool::buy(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			u128::MAX,
		));

		//Assert - Verify fees remain the same (since min = max, they can't change)
		let updated_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		assert_eq!(updated_fee.asset_fee, fixed_asset_fee);
		assert_eq!(updated_fee.protocol_fee, fixed_protocol_fee);
		assert_eq!(updated_fee.timestamp, 13_u32);

		//Act - Execute multiple trades in the same block
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			DOT_UNITS,
			0,
		));

		//Assert - Fees should still be the same
		let final_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		assert_eq!(final_fee.asset_fee, fixed_asset_fee);
		assert_eq!(final_fee.protocol_fee, fixed_protocol_fee);

		//Act - Trade in another block to ensure continued stability
		hydradx_run_to_block(14);
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			HDX,
			DOT,
			100 * UNITS,
			0,
		));

		//Assert - Fees still constant
		let final_fee_after_hdx_sell = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		assert_eq!(final_fee_after_hdx_sell.asset_fee, fixed_asset_fee);
		assert_eq!(final_fee_after_hdx_sell.protocol_fee, fixed_protocol_fee);
	});
}

#[test]
fn fees_should_be_applied_correctly_when_min_equals_max_in_dynamic_config() {
	TestNet::reset();

	// First scenario: zero fees
	let amount_out_with_zero_fee = Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		// Set dynamic fee config with min = max = 0 for DOT (asset out - asset fee applies)
		assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			pallet_dynamic_fees::types::AssetFeeConfig::Dynamic {
				asset_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: Permill::zero(),
					max_fee: Permill::zero(),
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
				protocol_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: Permill::zero(),
					max_fee: Permill::zero(),
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
			},
		));

		// Set dynamic fee config with min = max = 0 for HDX (asset in - protocol fee applies)
		assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			pallet_dynamic_fees::types::AssetFeeConfig::Dynamic {
				asset_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: Permill::zero(),
					max_fee: Permill::zero(),
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
				protocol_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: Permill::zero(),
					max_fee: Permill::zero(),
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
			},
		));

		let dave_hdx_before = hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(DAVE));

		//Act - Execute a sell trade (DOT -> HDX)
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let dave_hdx_after = hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(DAVE));

		// Verify fees are indeed zero
		let dot_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		let hdx_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_eq!(dot_fee.asset_fee, Permill::zero());
		assert_eq!(hdx_fee.protocol_fee, Permill::zero());

		dave_hdx_after - dave_hdx_before
	});

	TestNet::reset();

	// Second scenario: non-zero fees (min = max)
	let amount_out_with_fee = Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(12);

		let fixed_asset_fee = Permill::from_percent(1); // 1% asset fee (applies to DOT being sold)
		let fixed_protocol_fee = Permill::from_rational(5u32, 1000u32); // 0.5% protocol fee (applies to HDX being received)

		// Set dynamic fee config with min = max for DOT (asset out - asset fee applies)
		assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			pallet_dynamic_fees::types::AssetFeeConfig::Dynamic {
				asset_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_asset_fee,
					max_fee: fixed_asset_fee,
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
				protocol_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_protocol_fee,
					max_fee: fixed_protocol_fee,
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
			},
		));

		// Set dynamic fee config with min = max for HDX (asset in - protocol fee applies)
		assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			pallet_dynamic_fees::types::AssetFeeConfig::Dynamic {
				asset_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_asset_fee,
					max_fee: fixed_asset_fee,
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
				protocol_fee_params: pallet_dynamic_fees::types::FeeParams {
					min_fee: fixed_protocol_fee,
					max_fee: fixed_protocol_fee,
					decay: FixedU128::from_float(0.05),
					amplification: FixedU128::from_float(1.0),
				},
			},
		));

		let dave_hdx_before = hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(DAVE));

		//Act - Execute the same sell trade (DOT -> HDX)
		assert_ok!(hydradx_runtime::Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DOT,
			HDX,
			2 * DOT_UNITS,
			0,
		));

		let dave_hdx_after = hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(DAVE));

		// Verify fees are set correctly for both assets
		let dot_fee = hydradx_runtime::DynamicFees::current_fees(DOT).unwrap();
		let hdx_fee = hydradx_runtime::DynamicFees::current_fees(HDX).unwrap();
		assert_eq!(dot_fee.asset_fee, fixed_asset_fee);
		assert_eq!(hdx_fee.protocol_fee, fixed_protocol_fee);

		dave_hdx_after - dave_hdx_before
	});

	// Assert - Calculate the actual percentage difference
	let actual_difference = amount_out_with_zero_fee - amount_out_with_fee;

	// Calculate the actual percentage: (difference / amount_with_zero_fee) * 100
	// Use FixedU128 for precise calculation
	let actual_percentage = FixedU128::from_rational(actual_difference, amount_out_with_zero_fee);
	let actual_percentage_value = actual_percentage.into_inner() as f64 / 1_000_000_000_000_000_000.0 * 100.0;

	println!("\n=== Fee Application Test Results ===");
	println!("Trade: Sell 2 DOT -> HDX");
	println!("DOT asset fee: 1% (applied on DOT being sold)");
	println!("HDX protocol fee: 0.5% (applied on HDX being received)");
	println!("---");
	println!("Amount received with zero fees: {}", amount_out_with_zero_fee);
	println!("Amount received with fees: {}", amount_out_with_fee);
	println!("Difference: {}", actual_difference);
	println!("Actual percentage difference: {:.4}%", actual_percentage_value);
	println!("====================================\n");

	// Verify the fees were applied (should see some difference)
	assert!(actual_difference > 0, "Fees should result in less amount received");

	// Expected combined fee effect: 1% asset fee + 0.5% protocol fee
	// The actual effect depends on how fees compound, so we allow some tolerance
	let expected_percentage = 1.5; // 1% + 0.5%
	let tolerance = 0.1; // Allow 0.1% deviation for rounding and compounding effects

	assert!(
		actual_percentage_value >= expected_percentage - tolerance,
		"Actual fee percentage ({:.4}%) is lower than expected ({:.2}% - {:.2}%)",
		actual_percentage_value,
		expected_percentage,
		tolerance
	);
	assert!(
		actual_percentage_value <= expected_percentage + tolerance,
		"Actual fee percentage ({:.4}%) is higher than expected ({:.2}% + {:.2}%)",
		actual_percentage_value,
		expected_percentage,
		tolerance
	);
}
