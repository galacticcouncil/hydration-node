#![cfg(test)]

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::traits::fungible::Mutate;
use hydradx_traits::fee::GetDynamicFee;
use pallet_dynamic_fees::types::AssetFeeConfig;
use primitives::AssetId;
use sp_runtime::Permill;

const ASSET_ID_TO_TEST: AssetId = HDX;
const TRADE_AMOUNT: Balance = 100_000_000_000;
const FIXED_FEE_PERCENT: u32 = 50; // 50% fixed fee

#[test]
fn omnipool_fixed_fees_should_override_dynamic_fees() {
	let driver = HydrationTestDriver::default().setup_hydration();

	driver.execute(|| {
        let trader = AccountId::from(CHARLIE);
        assert_ok!(Currencies::update_balance(
            hydradx_runtime::RuntimeOrigin::root(),
            trader.clone(),
            DOT,
            (10_000 * UNITS) as i128,
        ));

        let initial_hdx_balance = hydradx_runtime::Balances::free_balance(&trader);

        assert_ok!(hydradx_runtime::Omnipool::sell(
            hydradx_runtime::RuntimeOrigin::signed(trader.clone()),
            DOT,
            ASSET_ID_TO_TEST,
            TRADE_AMOUNT,
            0u128,
        ));

        let hdx_received_dynamic = hydradx_runtime::Balances::free_balance(&trader) - initial_hdx_balance;

        let fixed_fee_config = AssetFeeConfig::Fixed {
            asset_fee: Permill::from_percent(FIXED_FEE_PERCENT),
            protocol_fee: Permill::from_percent(1),
        };

        assert_ok!(hydradx_runtime::DynamicFees::set_asset_fee(
            hydradx_runtime::RuntimeOrigin::root(),
            ASSET_ID_TO_TEST,
            fixed_fee_config
        ));

        let stored_config = hydradx_runtime::DynamicFees::asset_fee_config(ASSET_ID_TO_TEST);
        assert_eq!(stored_config, Some(fixed_fee_config));

        assert_ok!(Currencies::update_balance(
            hydradx_runtime::RuntimeOrigin::root(),
            trader.clone(),
            DOT,
            (10_000 * UNITS) as i128,
        ));
        hydradx_runtime::Balances::set_balance(&trader, initial_hdx_balance);

        let initial_hdx_balance_fixed = hydradx_runtime::Balances::free_balance(&trader);

        assert_ok!(hydradx_runtime::Omnipool::sell(
            hydradx_runtime::RuntimeOrigin::signed(trader.clone()),
            DOT,
            ASSET_ID_TO_TEST,
            TRADE_AMOUNT,
            0u128,
        ));

        let hdx_received_fixed = hydradx_runtime::Balances::free_balance(&trader) - initial_hdx_balance_fixed;
        assert!(hdx_received_fixed < hdx_received_dynamic, "Should receive less HDX with 50% fixed fee");

        // With 50% fixed fee, the received amount should be approximately half of the original
        let expected_half = hdx_received_dynamic / 2;
        let tolerance = hdx_received_dynamic / 10; // 10% tolerance
        let difference = if hdx_received_fixed > expected_half {
            hdx_received_fixed - expected_half
        } else {
            expected_half - hdx_received_fixed
        };
        assert!(difference <= tolerance,
            "HDX received with 50% fee ({}) should be approximately half of dynamic fee amount ({}), expected around {}, tolerance: {}", 
            hdx_received_fixed, hdx_received_dynamic, expected_half, tolerance);

        let asset_state = hydradx_runtime::Omnipool::load_asset_state(ASSET_ID_TO_TEST).unwrap();
        let current_fees = pallet_dynamic_fees::UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((ASSET_ID_TO_TEST, asset_state.reserve));
        assert_eq!(current_fees.0, Permill::from_percent(FIXED_FEE_PERCENT), "Asset fee should be the fixed 50%");
        assert_eq!(current_fees.1, Permill::from_percent(1), "Protocol fee should be the fixed 1%");
    })
    .new_block()
    .execute(||{
        let current_fees = pallet_dynamic_fees::UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((ASSET_ID_TO_TEST, 0));
        assert_eq!(current_fees.0, Permill::from_percent(FIXED_FEE_PERCENT), "Asset fee should be the fixed 50%");
        assert_eq!(current_fees.1, Permill::from_percent(1), "Protocol fee should be the fixed 1%");
    });
}
