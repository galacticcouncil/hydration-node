use crate::*;
use primitives::constants::{
	currency::{CENTS, DOLLARS, MILLICENTS},
	time::{DAYS, HOURS},
};
use std::io::Read;

use codec::Encode;
use frame_support::{assert_err, assert_ok, dispatch::{DispatchClass, GetDispatchInfo}, sp_runtime::{traits::Convert, FixedPointNumber}, weights::WeightToFee};
use pallet_transaction_payment::Multiplier;
use polkadot_xcm::opaque::VersionedXcm;
use polkadot_xcm::{VersionedAssets, VersionedLocation};
use sp_runtime::{BuildStorage, FixedU128};
use sp_runtime::transaction_validity::TransactionSource;
use sp_std::sync::Arc;
use sp_transaction_pool::runtime_api::runtime_decl_for_tagged_transaction_queue::TaggedTransactionQueue;

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

#[test]
fn test_me() {
	sp_io::TestExternalities::new_empty().execute_with(|| {

	use pallet_transaction_payment_rpc_runtime_api::runtime_decl_for_transaction_payment_api::TransactionPaymentApiV4;
	use sp_runtime::traits::Extrinsic;

		// bad
		let signed_extra = SignedExtra::decode(&mut &*hex!["0500000000"].to_vec()).unwrap();
		let signature = Signature::decode(&mut &*hex!["01c4724c12424e4546404cc8e8c3b7d080691ae47ab29ce6edbedf9f640c2b295613da09342aec160ca459fd32529fd23bc98348e369e9b31f3f992cfc4aca3983"].to_vec()).unwrap();
		let call = RuntimeCall::decode(&mut &*hex!["470101000000e8030000000000000000000000000000"].to_vec()).unwrap();
		let acc = sp_core::crypto::AccountId32::from(hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]);
		let res = fp_self_contained::UncheckedExtrinsic::<Address, RuntimeCall, Signature, SignedExtra>::new_signed(call, acc, signature, signed_extra);
		println!(" --- - - - -  - - - - - - - - -  {:X?}", res.encode());

		// goood
		let signed_extra = SignedExtra::decode(&mut &*hex!["0500000000"].to_vec()).unwrap();
		let signature = Signature::decode(&mut &*hex!["01961c8c631034521e227a2161916e0e07cbbf90429bcc936ceb3b60e25d14f66794b02437f76f207965f0aa56bc56c7c45ecbadc851e4ce9fd6276a5cd9ccef8e"].to_vec()).unwrap();
		let call = RuntimeCall::decode(&mut &*hex!["470101000000e8030000000000000000000000000000"].to_vec()).unwrap();
		let acc = sp_core::crypto::AccountId32::from(hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]);
		let res = fp_self_contained::UncheckedExtrinsic::<Address, RuntimeCall, Signature, SignedExtra>::new_signed(call, acc, signature, signed_extra);
		println!(" --- - - - -  - - - - - - - - -  {:X?}", res.encode());


		let encoded: Vec<u8> = hex!("59028400d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d01f2c033c15c44bdaa1f90c7e5b8c5c43d720b554d2b932fdff8423e17e0f9372905cab00991393f496ae6f525098b1164e041c7016aca4ce1fe40a54b0cf0148a55030d0e00000700a05d59ba9fee5afd2533de784a79e88cec70809117d863a70a4982989716926f1b000080f64ae1c7022d15")
			.to_vec();
		let xt: fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra> = Decode::decode(&mut &*encoded).unwrap();

		let ext = xt.encode();

		println!("encoded: 0x{}", hex::encode(&ext));

		let len = ext.len() as u32;

		println!(
			"\n{:?}",
			Runtime::query_info(xt.clone(), len),
		);

		let last_blockhash = System::block_hash(System::block_number());
		assert_ok!(Runtime::validate_transaction(TransactionSource::External, xt.clone(), last_blockhash));
	});
}

#[test]
fn hmm() {
	use pallet_balances::Call as BalancesCall;
	use pallet_bonds::Call as BondsCall;
	use pallet_transaction_payment_rpc_runtime_api::runtime_decl_for_transaction_payment_api::TransactionPaymentApiV4;
	use sp_runtime::traits::Extrinsic;

	let acc =
		sp_core::crypto::AccountId32::from(hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"]);

	let call = pallet_bonds::Call::<Runtime>::redeem {
		bond_id: 1,
		amount: 1000,
	};
	let call = RuntimeCall::Bonds(call);

	let xt = UncheckedExtrinsic::new_unsigned(call);
	let ext = xt.encode();
	println!("---- {:X?}", xt);
	println!("---- {:X?}", ext.as_slice());

	let o_xt = sp_runtime::OpaqueExtrinsic::decode(&mut &ext[..]);
	println!("---- {:X?}", o_xt);
	let len = ext.len() as u32;

	sp_io::TestExternalities::new_empty().execute_with(|| {
		assert_eq!(
			Runtime::query_info(xt.clone(), len),
			pallet_transaction_payment::RuntimeDispatchInfo::default()
		);
	});
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
