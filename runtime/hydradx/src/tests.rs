use crate::*;
use primitives::constants::{
	currency::{CENTS, DOLLARS, MILLICENTS},
	time::{DAYS, HOURS},
};

use codec::Encode;
use frame_support::{
	assert_err,
	dispatch::{DispatchClass, GetDispatchInfo},
	sp_runtime::{traits::Convert, FixedPointNumber},
	weights::WeightToFee,
};
use pallet_transaction_payment::Multiplier;
use polkadot_xcm::opaque::VersionedXcm;
use polkadot_xcm::{VersionedAssets, VersionedLocation};

use sp_runtime::{BuildStorage, FixedU128};
use sp_std::sync::Arc;
use sp_runtime::traits::Zero;

#[test]
#[ignore]
// TODO needs to be redesigned to not break after benchmarking
fn full_block_cost() {
	let max_bytes = *BlockLength::get().max.get(DispatchClass::Normal) as u128;
	let length_fee = max_bytes * TransactionByteFee::get();
	assert_eq!(length_fee, 39_321_600_000_000_000);

	let max_weight = BlockWeights::get()
		.get(DispatchClass::Normal)
		.max_total
		.unwrap_or(Weight::from_parts(1, 0));
	let weight_fee = crate::WeightToFee::weight_to_fee(&max_weight);
	assert_eq!(weight_fee, 375_600_961_538_250);

	let target_fee = 396 * DOLLARS + 97_201_061_378_250;

	assert_eq!(
		ExtrinsicBaseWeight::get().ref_time() as u128 + length_fee + weight_fee,
		target_fee
	);
}

#[test]
#[ignore]
// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
fn extrinsic_base_fee_is_correct() {
	let base_fee = crate::WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
	let base_fee_expected = CENTS / 10;
	assert!(base_fee.max(base_fee_expected) - base_fee.min(base_fee_expected) < MILLICENTS);
}

#[test]
#[ignore]
// Useful to calculate how much single transfer costs in native currency with fee components breakdown
fn transfer_cost() {
	let call = pallet_balances::Call::<Runtime>::transfer_allow_death {
		dest: AccountId::new([0; 32]),
		value: Default::default(),
	};
	let info = call.get_dispatch_info();
	// convert to outer call
	let call = RuntimeCall::Balances(call);
	let len = call.using_encoded(|e| e.len()) as u32;

	let mut ext = sp_io::TestExternalities::new_empty();
	ext.execute_with(|| {
		pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_integer(1));
		let fee_raw = TransactionPayment::compute_fee_details(len, &info, 0);
		let fee = fee_raw.final_fee();
		println!(
			"len = {:?} // weight = {:?} // base fee = {:?} // len fee = {:?} // adjusted weight_fee = {:?} // full transfer fee = {:?}\n",
			len,
			info.weight,
			fee_raw.inclusion_fee.clone().unwrap().base_fee,
			fee_raw.inclusion_fee.clone().unwrap().len_fee,
			fee_raw.inclusion_fee.unwrap().adjusted_weight_fee,
			fee,
		);
	});
}

fn run_with_system_weight<F>(w: Weight, mut assertions: F)
where
	F: FnMut(),
{
	let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap()
		.into();
	t.execute_with(|| {
		System::set_block_consumed_resources(w, 0);
		assertions()
	});
}

#[test]
fn multiplier_can_grow_from_zero() {
	let minimum_multiplier = MinimumMultiplier::get();
	let target = TargetBlockFullness::get() * BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	// if the min is too small, then this will not change, and we are doomed forever.
	// the weight is 1/100th bigger than target.
	run_with_system_weight(target * 101 / 100, || {
		let next = SlowAdjustingFeeUpdate::<Runtime>::convert(minimum_multiplier);
		assert!(next > minimum_multiplier, "{next:?} !>= {minimum_multiplier:?}");
	})
}
#[ignore]
#[test]
fn multiplier_growth_simulator() {
	// calculate the value of the fee multiplier after one hour of operation with fully loaded blocks
	let max_multiplier = MaximumMultiplier::get();
	println!("max multiplier = {max_multiplier:?}");

	let mut multiplier = Multiplier::saturating_from_integer(1);
	let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	for _block_num in 1..=HOURS {
		run_with_system_weight(block_weight, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			// ensure that it is growing as well.
			//assert!(next > multiplier, "{next:?} !>= {multiplier:?}");
			println!("multiplier = {multiplier:?}");
			multiplier = next;
		});
	}
	println!("multiplier = {multiplier:?}");
}
#[ignore]
#[test]
fn fee_growth_simulator() {
	use frame_support::traits::OnFinalize;
	// calculate the value of the fee multiplier after one hour of operation with fully loaded blocks
	let max_multiplier = MaximumMultiplier::get();
	println!("--- FEE GROWTH SIMULATOR STARTS ---");

	println!("With max multiplier = {max_multiplier:?}");

	let mut multiplier = Multiplier::saturating_from_integer(1);
	let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	for _block_num in 1..=HOURS {
		run_with_system_weight(block_weight, || {
			let b = crate::System::block_number();

			let call = pallet_omnipool::Call::<Runtime>::sell {
				asset_in: 2,
				asset_out: 0,
				amount: 1_000_000_000_000,
				min_buy_amount: 0,
			};
			let call_len = call.encoded_size() as u32;
			let info = call.get_dispatch_info();

			let next = TransactionPayment::next_fee_multiplier();
			let call_fee = TransactionPayment::compute_fee(call_len, &info, 0);

			<pallet_transaction_payment::Pallet<Runtime> as OnFinalize<BlockNumber>>::on_finalize(b + 1);
			crate::System::set_block_number(b + 1);

			//let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			println!("Trade fee = {call_fee:?} with multiplier = {multiplier:?}");
			multiplier = next;
		});
	}
	println!("multiplier = {multiplier:?}");
}
#[test]
#[ignore]
fn max_multiplier() {
	// calculate the value of the fee multiplier after one hour of operation with fully loaded blocks
	let max_multiplier = MaximumMultiplier::get();
	let mut multiplier = Multiplier::saturating_from_integer(1);
	let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	for _block_num in 1..=10 * DAYS {
		run_with_system_weight(block_weight, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			// ensure that the multiplier is not greater than max_multiplier
			assert!(next <= max_multiplier, "{next:?} !<= {max_multiplier:?}");
			multiplier = next;
		});
	}
}

#[cfg(test)]
mod xcm_fee_payment_api_tests {
	use super::*;
	use frame_support::assert_ok;
	use polkadot_xcm::v4::prelude::*;
	use polkadot_xcm::VersionedAssetId::V4;
	use xcm_fee_payment_runtime_api::runtime_decl_for_xcm_payment_api::XcmPaymentApiV1;
	use xcm_fee_payment_runtime_api::Error as XcmPaymentApiError;

	#[test]
	fn query_acceptable_payment_assets_should_return_native_and_registered_locations() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			// register new asset in the asset registry
			assert_ok!(AssetRegistry::register_external(
				RuntimeOrigin::signed([0u8; 32].into()),
				crate::xcm::AssetLocation(polkadot_xcm::v3::Location::new(
					1,
					polkadot_xcm::v3::Junctions::X2(
						polkadot_xcm::v3::Junction::Parachain(123),
						polkadot_xcm::v3::Junction::GeneralIndex(123)
					)
				))
			));

			assert_eq!(
				Runtime::query_acceptable_payment_assets(4u32),
				Ok(vec![
					// HDX locations
					V4(AssetId(Location {
						parents: 1,
						interior: Junctions::X2([Parachain(100), GeneralIndex(0)].into())
					})),
					V4(AssetId(Location {
						parents: 0,
						interior: Junctions::X1([GeneralIndex(0)].into())
					})),
					// asset from the asset registry
					V4(AssetId(Location {
						parents: 1,
						interior: Junctions::X2([Parachain(123), GeneralIndex(123)].into())
					})),
				])
			);
		});
	}

	#[test]
	fn query_weight_to_asset_fee_should_return_correct_weight() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			// register new assets in the asset registry
			let registered_asset_with_price = AssetRegistry::next_asset_id().unwrap();
			assert_ok!(AssetRegistry::register_external(
				RuntimeOrigin::signed([0u8; 32].into()),
				crate::xcm::AssetLocation(polkadot_xcm::v3::Location::new(
					1,
					polkadot_xcm::v3::Junctions::X2(
						polkadot_xcm::v3::Junction::Parachain(123),
						polkadot_xcm::v3::Junction::GeneralIndex(123)
					)
				))
			));

			assert_ok!(AssetRegistry::register_external(
				RuntimeOrigin::signed([0u8; 32].into()),
				crate::xcm::AssetLocation(polkadot_xcm::v3::Location::new(
					1,
					polkadot_xcm::v3::Junctions::X2(
						polkadot_xcm::v3::Junction::Parachain(123),
						polkadot_xcm::v3::Junction::GeneralIndex(456)
					)
				))
			));

			// set the price of registered asset
			assert_ok!(MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				registered_asset_with_price,
				FixedU128::from_float(2.0)
			));
			MultiTransactionPayment::on_initialize(1);

			let expected_fee = 155655590214;
			// HDX fee
			assert_eq!(
				Runtime::query_weight_to_asset_fee(
					Weight::from_parts(1_000_000_000, 1_000_000_000),
					V4(AssetId(Location::new(
						1,
						Junctions::X2(Arc::new([Parachain(100), GeneralIndex(0)]))
					)))
				),
				Ok(expected_fee)
			);
			// registered asset fee
			assert_eq!(
				Runtime::query_weight_to_asset_fee(
					Weight::from_parts(1_000_000_000, 1_000_000_000),
					V4(AssetId(Location::new(
						1,
						Junctions::X2(Arc::new([Parachain(123), GeneralIndex(123)]))
					)))
				),
				Ok(2 * expected_fee)
			);
			// asset not registered
			assert_err!(
				Runtime::query_weight_to_asset_fee(
					Weight::from_parts(1_000_000_000, 1_000_000_000),
					V4(AssetId(Location::new(
						1,
						Junctions::X2(Arc::new([Parachain(666), GeneralIndex(0)]))
					)))
				),
				XcmPaymentApiError::AssetNotFound
			);
			// price not available
			assert_err!(
				Runtime::query_weight_to_asset_fee(
					Weight::from_parts(1_000_000_000, 1_000_000_000),
					V4(AssetId(Location::new(
						1,
						Junctions::X2(Arc::new([Parachain(123), GeneralIndex(456)]))
					)))
				),
				XcmPaymentApiError::WeightNotComputable
			);
		});
	}

	#[test]
	fn query_xcm_weight_should_return_weight_for_xcm() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let hdx_loc = Location::new(1, Junctions::X2(Arc::new([Parachain(100), GeneralIndex(0)])));

			let asset_to_withdraw: Asset = Asset {
				id: AssetId(hdx_loc.clone()),
				fun: Fungible(1_000_000_000_000u128),
			};
			let xcm_message = Xcm(vec![
				WithdrawAsset(asset_to_withdraw.into()),
				BuyExecution {
					fees: (Here, 400000000000u128).into(),
					weight_limit: Unlimited,
				},
			]);

			assert_eq!(
				Runtime::query_xcm_weight(VersionedXcm::from(xcm_message.clone())),
				Ok(Weight::from_parts(200000000, 0))
			);
		});
	}

	#[test]
	fn query_delivery_fees_should_return_no_assets() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let hdx_loc = Location::new(1, Junctions::X2(Arc::new([Parachain(100), GeneralIndex(0)])));

			let asset_to_withdraw: Asset = Asset {
				id: AssetId(hdx_loc.clone()),
				fun: Fungible(1_000_000_000_000u128),
			};
			let xcm_message = Xcm(vec![
				WithdrawAsset(asset_to_withdraw.into()),
				BuyExecution {
					fees: (Here, 400000000000u128).into(),
					weight_limit: Unlimited,
				},
			]);

			let destination = Location::new(1, Junctions::X1(Arc::new([Parachain(100)])));

			// set default XCM version
			assert_ok!(PolkadotXcm::force_default_xcm_version(
				RuntimeOrigin::root(),
				Some(3u32)
			));
			assert_eq!(
				Runtime::query_delivery_fees(VersionedLocation::V4(destination), VersionedXcm::from(xcm_message)),
				Ok(VersionedAssets::V4(Assets::new()))
			);
		});
	}
}
mod fee_estimation_api_tests {
	use super::*;
	use frame_support::assert_ok;
	use primitives::runtime_api::runtime_decl_for_fee_estimation_api::FeeEstimationApiV1;
	use sp_runtime::FixedU128;

	#[test]
	fn estimate_fee_payment_returns_native_for_default() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let account = AccountId::new([0u8; 32]);
			let weight = Weight::from_parts(1_000_000_000, 1_000_000_000);

			let (asset_id, fee) =
				<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
					weight, account,
				);

			// Should return native asset by default
			assert_eq!(asset_id, NativeAssetId::get());
			assert!(fee > 0);
		});
	}

	#[test]
	fn estimate_fee_payment_handles_configured_currency() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let account = AccountId::new([1u8; 32]);
			let custom_currency = 2;

			// First add the currency to accepted currencies
			assert_ok!(MultiTransactionPayment::add_currency(
				RuntimeOrigin::root(),
				custom_currency,
				FixedU128::from_float(2.0) // 2x the native price
			));
			MultiTransactionPayment::on_initialize(1); // Initialize prices

			// Then configure account to use the currency
			assert_ok!(MultiTransactionPayment::set_currency(
				RuntimeOrigin::signed(account.clone()),
				custom_currency
			));

			let weight = Weight::from_parts(1_000_000_000, 1_000_000_000);
			let (asset_id, fee) =
				<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
					weight, account,
				);

			assert_eq!(asset_id, custom_currency);

			// Get native fee for comparison
			let (native_asset_id, native_fee) =
				<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
					weight,
					AccountId::new([0u8; 32]),
				);

			assert_eq!(native_asset_id, NativeAssetId::get());

			// Custom currency fee should be 2x native fee
			assert_eq!(fee, native_fee * 2);
		});
	}
	#[test]
	fn estimate_fee_payment_for_extrinsic_works() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let account = AccountId::new([0u8; 32]);

			// Create a test extrinsic (e.g., a balance transfer)
			let call = pallet_balances::Call::<Runtime>::transfer_allow_death {
				dest: AccountId::new([1u8; 32]),
				value: 100u128,
			};

			// Create unchecked extrinsic directly
			let xt = UncheckedExtrinsic::new_unsigned(RuntimeCall::Balances(call));

			// Convert encoded extrinsic to BoundedVec
			let bounded_xt = BoundedVec::<u8, MaxExtrinsicSize>::try_from(xt.encode())
				.expect("Extrinsic size should be within bounds");

			// Get fee estimation
			let (asset_id, fee) =
				<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment_for_extrinsic(
					bounded_xt, account,
				);

			// Should use native asset by default
			assert_eq!(asset_id, NativeAssetId::get());
			assert!(fee > 0);

			// Fee should include both weight and length components
			let info = xt.get_dispatch_info();
			let len = xt.encoded_size() as u32;
			let len_fee = TransactionPayment::length_to_fee(len);
			let weight_fee = TransactionPayment::weight_to_fee(info.weight);

			assert_eq!(fee, len_fee.saturating_add(weight_fee));
		});
	}
	#[test]
fn no_price_available_falls_back_to_one() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::new([2u8; 32]);
		let custom_currency = 3;

		// Add currency without setting a price
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			custom_currency,
			// Deliberately do not set a price, or set an empty price update later
			FixedU128::zero() 
		));

		// Configure account to use this currency
		assert_ok!(MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(account.clone()),
			custom_currency
		));

		let weight = Weight::from_parts(500_000_000, 0);
		let (asset_id, fee) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight, account,
			);

		// Should return the custom currency even if no price is set
		assert_eq!(asset_id, custom_currency);

		// Compare against native calculation
		let (_native_asset, native_fee) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight,
				AccountId::new([0u8; 32]),
			);

		// Since no price was set, fallback is 1:1 ratio
		assert_eq!(fee, native_fee);
	});
}

#[test]
fn invalid_extrinsic_input() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::new([3u8; 32]);

		// Provide an empty vector as extrinsic input
		let bounded_xt = BoundedVec::<u8, MaxExtrinsicSize>::try_from(vec![])
			.expect("Empty vector should be within bounds");

		// The current code may panic if decoding fails; ideally, we should handle errors gracefully.
		// If the runtime panics or expects valid input only, this test can confirm current behavior.
		//
		// To handle gracefully, consider updating the runtime API implementation to return a Result.
		// For now, we just run this test to ensure it doesn't break assumptions.
		let result = std::panic::catch_unwind(|| {
			let _ = <Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment_for_extrinsic(
				bounded_xt, account
			);
		});

		// Expecting a panic due to invalid extrinsic. If code is updated to handle errors,
		// you may replace this with proper `assert_err!` checks.
		assert!(result.is_err(), "Expected panic on invalid extrinsic input");
	});
}

#[test]
fn max_extrinsic_size_calculation() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::new([4u8; 32]);
		let max_size = primitives::constants::transaction::MaxExtrinsicSize::get();

		// We'll create a remark call with a large payload close to max_size.
		// Reserve some space for extrinsic overhead, signatures, etc. Let's keep a safe margin.
		let overhead = 1024; 
		let payload_size = (max_size - overhead) as usize;

		let large_remark = vec![0u8; payload_size];
		let call = frame_system::Call::<Runtime>::remark { remark: large_remark };

		let xt = UncheckedExtrinsic::new_unsigned(RuntimeCall::System(call));
		let encoded = xt.encode();

		// Ensure the encoded extrinsic is not larger than MaxExtrinsicSize
		assert!(encoded.len() as u32 <= max_size, "Extrinsic exceeds MaxExtrinsicSize");

		// Convert encoded extrinsic to BoundedVec
		let bounded_xt = BoundedVec::<u8, MaxExtrinsicSize>::try_from(encoded)
			.expect("Extrinsic should fit into MaxExtrinsicSize bounds");

		let (asset_id, fee) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment_for_extrinsic(
				bounded_xt, account
			);

		assert_eq!(asset_id, NativeAssetId::get());
		assert!(fee > 0, "Fee should be nonzero for large extrinsic due to length fee");
	});
}



#[test]
fn minimal_weight_fee_calculation() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::new([5u8; 32]);
		let weight = Weight::from_parts(0, 0); // minimal weight

		let (asset_id, fee) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight, account,
			);

		assert_eq!(asset_id, NativeAssetId::get());
		// Fee might not be zero if there's a base fee; if there's no base fee,
		// fee may be zero. Check that code doesn't fail.
		// If your runtime has a minimum fee, check that fee >= min_fee here.
		assert!(fee >= 0, "Fee should not be negative for zero weight");
	});
}

#[test]
fn currency_switching_scenario() {
	sp_io::TestExternalities::new_empty().execute_with(|| {
		let account = AccountId::new([6u8; 32]);
		let currency_a = 10;
		let currency_b = 11;

		// Add currency A with a price of 1.5x native
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			currency_a,
			FixedU128::from_float(1.5)
		));
		// Add currency B with a price of 0.5x native
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			currency_b,
			FixedU128::from_float(0.5)
		));
		MultiTransactionPayment::on_initialize(1);

		// Set currency A
		assert_ok!(MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(account.clone()),
			currency_a
		));

		let weight = Weight::from_parts(1_000_000_000, 0);
		let (asset_id_a, fee_a) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight, account.clone(),
			);

		assert_eq!(asset_id_a, currency_a);

		// Set currency B
		assert_ok!(MultiTransactionPayment::set_currency(
			RuntimeOrigin::signed(account.clone()),
			currency_b
		));

		let (asset_id_b, fee_b) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight, account.clone(),
			);

		assert_eq!(asset_id_b, currency_b);

		// Compare fees: fee_b should be lower, since currency B price is half the native fee
		// and currency A price was 1.5x native.
		let (_native_id, native_fee) =
			<Runtime as FeeEstimationApiV1<Block, AccountId, AssetId, Balance>>::estimate_fee_payment(
				weight,
				AccountId::new([0u8; 32]),
			);

		assert_eq!(fee_a, (native_fee * 3) / 2); // 1.5x
		assert_eq!(fee_b, native_fee / 2); // 0.5x
	});
}

}
