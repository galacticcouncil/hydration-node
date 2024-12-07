use super::*;
use frame_support::{assert_ok, assert_noop};
use sp_runtime::FixedU128;
use crate::Tradability;
 



const LRNA: AssetId = 1; 
const NON_HUB_ASSET: AssetId = 42;
const SOME_ASSET_ID: AssetId = 999;


fn all_flags() -> Tradability {
    Tradability::BUY
        | Tradability::SELL
        | Tradability::ADD_LIQUIDITY
        | Tradability::REMOVE_LIQUIDITY
}

	#[test]
	fn sell_asset_tradable_state_should_work_when_hub_asset_new_state_contains_sell_or_buy() {
		ExtBuilder::default()
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL
				));
				assert_ok!(Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::BUY
				));
				assert_ok!(Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL | Tradability::BUY
				));
			});
	}
#[test]
fn sell_asset_tradable_state_should_fail_when_hub_asset_new_state_contains_liquidity_operations() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL | Tradability::ADD_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL | Tradability::REMOVE_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
		});
}
#[test]
fn set_asset_tradable_state_should_work_with_operational_dispatch() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			// Define expected state for clarity
			let expected_state =
				Tradability::SELL | Tradability::BUY | Tradability::REMOVE_LIQUIDITY | Tradability::ADD_LIQUIDITY;

			// Check if function works as expected with Operational class
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),                // Using root as the high-level origin
				LRNA,                                 // Example asset ID
				Tradability::SELL | Tradability::BUY  // New state
			));

			// Validate the new state in storage
			assert_eq!(Omnipool::tradable_state(LRNA), expected_state);
		});
}

#[test]
fn set_asset_tradable_state_should_fail_for_invalid_asset() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let invalid_asset_id = 9999; // Example of an invalid asset ID

			// Ensure the function fails for invalid asset
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(), // Root origin
					invalid_asset_id,      // Invalid asset ID
					Tradability::SELL      // New state
				),
				Error::<Test>::AssetNotFound // Expected error for missing asset
			);
		});
}
#[test]
fn set_asset_tradable_state_should_work_with_no_state_change() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			// Set initial tradable state
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::SELL | Tradability::BUY
			));

			// Attempt to set the same state
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::SELL | Tradability::BUY
			));

			// Validate no additional flags were added
			assert_eq!(
				Omnipool::tradable_state(LRNA),
				Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
			);
		});
}


#[test]
fn set_asset_tradable_state_should_allow_all_flags() {
    ExtBuilder::default()
        .with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
        .with_registered_asset(SOME_ASSET_ID)
        // Give LP1 the asset first
        .with_asset_balance_for(LP1, SOME_ASSET_ID, 1_000_000_000_000)
        // Now add the token from LP1 to Omnipool
        .with_token(SOME_ASSET_ID, FixedU128::from(1), LP1, 1_000_000_000_000)
        .build()
        .execute_with(|| {
            assert_ok!(Omnipool::set_asset_tradable_state(
                RuntimeOrigin::root(),
                SOME_ASSET_ID,
                all_flags()
            ));
        });
}


#[test]
fn set_asset_tradable_state_should_allow_all_flags_for_non_hub_asset() {
    const NON_HUB_ASSET: AssetId = 42;

    ExtBuilder::default()
        .with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
        .with_registered_asset(NON_HUB_ASSET)
        .with_asset_balance_for(LP1, NON_HUB_ASSET, 1_000_000_000_000)
        .with_token(NON_HUB_ASSET, FixedU128::from(1), LP1, 1_000_000_000_000)
        .build()
        .execute_with(|| {
            assert_ok!(Omnipool::set_asset_tradable_state(
                RuntimeOrigin::root(),
                NON_HUB_ASSET,
                all_flags()
            ));
        });
}


#[test]
fn set_asset_tradable_state_should_restrict_hub_asset_flags() {
    // If you're testing the hub asset (LRNA), you already have it added via .with_initial_pool()
    // If you're testing another asset instead, ensure it's also added similar to above tests.
    ExtBuilder::default()
        .with_initial_pool(FixedU128::from(1), FixedU128::from(1))
        .build()
        .execute_with(|| {
            // For hub asset (LRNA), it's already known and added. Just set flags:
            assert_ok!(Omnipool::set_asset_tradable_state(
                RuntimeOrigin::root(),
                LRNA,
                Tradability::SELL | Tradability::BUY
            ));

            // Attempting to set liquidity flags for the hub asset should fail.
            assert_noop!(
                Omnipool::set_asset_tradable_state(
                    RuntimeOrigin::root(),
                    LRNA,
                    Tradability::SELL | Tradability::ADD_LIQUIDITY
                ),
                Error::<Test>::InvalidHubAssetTradableState
            );
        });
}
