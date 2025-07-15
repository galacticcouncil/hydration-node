use super::mock::*;
use crate::types::{AssetFeeConfig, FeeParams};
use crate::{AssetFeeConfiguration, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::Zero;
use sp_runtime::{DispatchError, FixedU128, Perquintill};

#[test]
fn set_fixed_fee_config_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;
		let asset_fee = Perquintill::from_percent(5);
		let protocol_fee = Perquintill::from_percent(2);

		let config = AssetFeeConfig::Fixed {
			asset_fee,
			protocol_fee,
		};

		// Should work with root origin
		assert_ok!(DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config));

		// Verify storage
		let stored_config = AssetFeeConfiguration::<Test>::get(asset_id);
		assert_eq!(stored_config, Some(config));

		// Verify event was emitted
		// System::assert_last_event(
		// 	crate::Event::AssetFeeConfigSet { asset_id }.into()
		// );
	});
}

#[test]
fn set_dynamic_fee_config_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;
		let asset_fee_params = FeeParams {
			min_fee: Perquintill::from_percent(1),
			max_fee: Perquintill::from_percent(10),
			decay: FixedU128::from_rational(1, 10),
			amplification: FixedU128::from_rational(2, 1),
		};
		let protocol_fee_params = FeeParams {
			min_fee: Perquintill::from_percent(1),
			max_fee: Perquintill::from_percent(5),
			decay: FixedU128::from_rational(1, 5),
			amplification: FixedU128::from_rational(3, 1),
		};

		let config = AssetFeeConfig::Dynamic {
			asset_fee_params,
			protocol_fee_params,
		};

		// Should work with root origin
		assert_ok!(DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config));

		// Verify storage
		let stored_config = AssetFeeConfiguration::<Test>::get(asset_id);
		assert_eq!(stored_config, Some(config));

		// Verify event was emitted
		// System::assert_last_event(
		// 	crate::Event::AssetFeeConfigSet { asset_id }.into()
		// );
	});
}

#[test]
fn set_asset_fee_config_fails_with_invalid_parameters() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;

		// Test with invalid asset fee params (min > max)
		let config = AssetFeeConfig::Dynamic {
			asset_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(10),
				max_fee: Perquintill::from_percent(5), // min > max
				decay: FixedU128::from_rational(1, 10),
				amplification: FixedU128::from_rational(2, 1),
			},
			protocol_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(5),
				decay: FixedU128::from_rational(1, 5),
				amplification: FixedU128::from_rational(3, 1),
			},
		};

		assert_noop!(
			DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config),
			Error::<Test>::InvalidFeeParameters
		);

		// Test with zero amplification
		let config = AssetFeeConfig::Dynamic {
			asset_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(5),
				decay: FixedU128::from_rational(1, 10),
				amplification: FixedU128::zero(), // zero amplification
			},
			protocol_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(5),
				decay: FixedU128::from_rational(1, 5),
				amplification: FixedU128::from_rational(3, 1),
			},
		};

		assert_noop!(
			DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config),
			Error::<Test>::InvalidFeeParameters
		);
	});
}

#[test]
fn set_asset_fee_config_fails_with_non_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;
		let config = AssetFeeConfig::Fixed {
			asset_fee: Perquintill::from_percent(5),
			protocol_fee: Perquintill::from_percent(2),
		};

		// Should fail with non-root origin
		assert_noop!(
			DynamicFees::set_asset_fee(RuntimeOrigin::signed(1), asset_id, config),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_asset_fee_config_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;
		let config = AssetFeeConfig::Fixed {
			asset_fee: Perquintill::from_percent(5),
			protocol_fee: Perquintill::from_percent(2),
		};

		// First set a config
		assert_ok!(DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config));

		// Verify it's stored
		assert!(AssetFeeConfiguration::<Test>::get(asset_id).is_some());

		// Remove it
		assert_ok!(DynamicFees::remove_asset_fee(RuntimeOrigin::root(), asset_id));

		// Verify it's removed
		assert!(AssetFeeConfiguration::<Test>::get(asset_id).is_none());

		// Verify event was emitted
		// System::assert_last_event(
		// 	crate::Event::AssetFeeConfigRemoved { asset_id }.into()
		// );
	});
}

#[test]
fn remove_asset_fee_config_fails_with_non_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;

		// Should fail with non-root origin
		assert_noop!(
			DynamicFees::remove_asset_fee(RuntimeOrigin::signed(1), asset_id),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn fixed_fee_config_returns_fixed_values() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;
		let asset_fee = Perquintill::from_percent(5);
		let protocol_fee = Perquintill::from_percent(2);

		let config = AssetFeeConfig::Fixed {
			asset_fee,
			protocol_fee,
		};

		// Set fixed fee config
		assert_ok!(DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config));

		// Retrieve fees - should return the fixed values
		let (retrieved_asset_fee, retrieved_protocol_fee) = retrieve_fee_entry(asset_id, 1000 * ONE);
		assert_eq!(retrieved_asset_fee, asset_fee);
		assert_eq!(retrieved_protocol_fee, protocol_fee);

		// Should return the same values regardless of liquidity
		let (retrieved_asset_fee2, retrieved_protocol_fee2) = retrieve_fee_entry(asset_id, 100 * ONE);
		assert_eq!(retrieved_asset_fee2, asset_fee);
		assert_eq!(retrieved_protocol_fee2, protocol_fee);
	});
}

#[test]
fn dynamic_fee_config_uses_custom_parameters() {
	ExtBuilder::default()
		.with_asset_fee_params(
			Perquintill::from_percent(2),
			Perquintill::from_percent(20),
			FixedU128::from_rational(1, 10),
			FixedU128::from_rational(1, 1),
		)
		.build()
		.execute_with(|| {
			let asset_id = HDX;
			let custom_asset_fee_params = FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(10),
				decay: FixedU128::from_rational(1, 10),
				amplification: FixedU128::from_rational(2, 1),
			};
			let custom_protocol_fee_params = FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(5),
				decay: FixedU128::from_rational(1, 5),
				amplification: FixedU128::from_rational(3, 1),
			};

			let config = AssetFeeConfig::Dynamic {
				asset_fee_params: custom_asset_fee_params,
				protocol_fee_params: custom_protocol_fee_params,
			};

			// Set custom dynamic fee config
			assert_ok!(DynamicFees::set_asset_fee(RuntimeOrigin::root(), asset_id, config));

			// Retrieve fees - should use custom parameters, not default ones
			let (retrieved_asset_fee, retrieved_protocol_fee) = retrieve_fee_entry(asset_id, 1000 * ONE);

			// The fee should be within the custom min/max range, not the default range
			assert!(retrieved_asset_fee >= custom_asset_fee_params.min_fee);
			assert!(retrieved_asset_fee <= custom_asset_fee_params.max_fee);
			assert!(retrieved_protocol_fee >= custom_protocol_fee_params.min_fee);
			assert!(retrieved_protocol_fee <= custom_protocol_fee_params.max_fee);
		});
}

#[test]
fn no_config_uses_default_parameters() {
	ExtBuilder::default()
		.with_asset_fee_params(
			Perquintill::from_percent(2),
			Perquintill::from_percent(20),
			FixedU128::from_rational(1, 10),
			FixedU128::from_rational(1, 1),
		)
		.build()
		.execute_with(|| {
			let asset_id = HDX;
			let default_asset_fee_params = AssetFeeParams::get();
			let default_protocol_fee_params = ProtocolFeeParams::get();

			// No config set - should use default parameters
			let (retrieved_asset_fee, retrieved_protocol_fee) = retrieve_fee_entry(asset_id, 1000 * ONE);

			// The fee should be within the default min/max range
			assert!(retrieved_asset_fee >= default_asset_fee_params.min_fee);
			assert!(retrieved_asset_fee <= default_asset_fee_params.max_fee);
			assert!(retrieved_protocol_fee >= default_protocol_fee_params.min_fee);
			assert!(retrieved_protocol_fee <= default_protocol_fee_params.max_fee);
		});
}

#[test]
fn switching_from_dynamic_to_fixed_works() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = HDX;

		// First set dynamic config
		let dynamic_config = AssetFeeConfig::Dynamic {
			asset_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(10),
				decay: FixedU128::from_rational(1, 10),
				amplification: FixedU128::from_rational(2, 1),
			},
			protocol_fee_params: FeeParams {
				min_fee: Perquintill::from_percent(1),
				max_fee: Perquintill::from_percent(5),
				decay: FixedU128::from_rational(1, 5),
				amplification: FixedU128::from_rational(3, 1),
			},
		};

		assert_ok!(DynamicFees::set_asset_fee(
			RuntimeOrigin::root(),
			asset_id,
			dynamic_config
		));

		// Now switch to fixed config
		let fixed_asset_fee = Perquintill::from_percent(7);
		let fixed_protocol_fee = Perquintill::from_percent(3);
		let fixed_config = AssetFeeConfig::Fixed {
			asset_fee: fixed_asset_fee,
			protocol_fee: fixed_protocol_fee,
		};

		assert_ok!(DynamicFees::set_asset_fee(
			RuntimeOrigin::root(),
			asset_id,
			fixed_config
		));

		// Should now return fixed values
		let (retrieved_asset_fee, retrieved_protocol_fee) = retrieve_fee_entry(asset_id, 1000 * ONE);
		assert_eq!(retrieved_asset_fee, fixed_asset_fee);
		assert_eq!(retrieved_protocol_fee, fixed_protocol_fee);
	});
}
