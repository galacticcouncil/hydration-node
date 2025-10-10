#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::{assert_ok, storage::with_transaction};
use hydradx_traits::{AssetKind, Create};
use polkadot_xcm::opaque::v3::{Junction as V3Junction, Junctions as V3Junctions, MultiLocation};
use polkadot_xcm::v4::prelude::*;
use primitives::AssetId;
use sp_runtime::{traits::Convert, TransactionOutcome};
use sp_std::sync::Arc;
use xcm_emulator::TestExt;

// Native asset (HDX)
#[test]
fn convert_asset_id_to_location_should_work_for_native_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let asset_id: AssetId = HDX;

		// Act - Convert CORE_ASSET_ID to location
		let location = hydradx_runtime::CurrencyIdConvert::convert(asset_id);

		// Assert - Should return local location with GeneralIndex(0)
		assert!(location.is_some());
		let loc = location.unwrap();
		assert_eq!(loc.parents, 0);

		match loc.interior {
			Junctions::X1(ref junctions) => {
				assert!(matches!(
					junctions.as_ref()[0],
					GeneralIndex(idx) if idx == HDX as u128
				));
			}
			_ => panic!("Expected X1 junction with GeneralIndex"),
		}
	});
}

#[test]
fn convert_location_to_asset_id_should_work_for_native_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange - Create location with X1(GeneralIndex(0)) and parents=0
		// For local assets, GeneralIndex directly maps to AssetId
		let location = Location {
			parents: 0,
			interior: Junctions::X1(Arc::new([GeneralIndex(HDX.into())])),
		};

		// Act
		let result = hydradx_runtime::CurrencyIdConvert::convert(location);

		// Assert
		assert!(result.is_some());
		assert_eq!(result.unwrap(), HDX);
	});
}

// Local assets (parents: 0)
#[test]
fn convert_asset_id_to_location_should_work_for_local_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		// Register a local asset with AccountKey20 Location (parents=0)
		let asset_id: AssetId = 1000;
		let _: Result<(), sp_runtime::DispatchError> = with_transaction(|| {
			assert_ok!(hydradx_runtime::AssetRegistry::register_sufficient_asset(
				Some(asset_id),
				Some(b"TKN".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1_000_000,
				None,
				None,
				Some(hydradx_runtime::AssetLocation(MultiLocation::new(
					0,
					V3Junctions::X1(V3Junction::AccountKey20 {
						network: None,
						key: [1u8; 20]
					})
				))),
				None,
			));
			TransactionOutcome::Commit(Ok(()))
		});

		// Act
		let location = hydradx_runtime::CurrencyIdConvert::convert(asset_id);

		// Assert
		// Should return GeneralIndex instead of AccountKey20 because parents=0 (local)
		assert!(location.is_some());
		let loc = location.unwrap();
		assert_eq!(loc.parents, 0);

		// Check that interior is X1 with GeneralIndex (not AccountKey20)
		match loc.interior {
			Junctions::X1(ref junctions) => {
				assert!(matches!(
					junctions.as_ref()[0],
					GeneralIndex(idx) if idx == asset_id as u128
				));
			}
			_ => panic!("Expected X1 junction with GeneralIndex"),
		}
	});
}

#[test]
fn convert_location_to_asset_id_should_work_for_local_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let asset_id: AssetId = 1000;
		let location = Location {
			parents: 0,
			interior: Junctions::X1(Arc::new([GeneralIndex(asset_id.into())])),
		};

		// Act
		let result = hydradx_runtime::CurrencyIdConvert::convert(location);

		// Assert
		assert!(result.is_some());
		assert_eq!(result.unwrap(), asset_id);
	});
}

#[test]
fn convert_location_to_asset_id_should_handle_invalid_local_location() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let invalid_location = Location {
			parents: 0,
			interior: Junctions::X1(Arc::new([Parachain(1000)])),
		};

		// Act
		let result = hydradx_runtime::CurrencyIdConvert::convert(invalid_location);

		// Assert
		assert!(result.is_none());
	});
}

#[test]
fn roundtrip_conversion_should_work_for_local_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let original_asset_id: AssetId = 1000;
		let _: Result<(), sp_runtime::DispatchError> = with_transaction(|| {
			assert_ok!(hydradx_runtime::AssetRegistry::register_sufficient_asset(
				Some(original_asset_id),
				Some(b"TST".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1_000_000,
				None,
				None,
				Some(hydradx_runtime::AssetLocation(MultiLocation::new(
					0,
					V3Junctions::X1(V3Junction::AccountKey20 {
						network: None,
						key: [1u8; 20]
					})
				))),
				None,
			));
			TransactionOutcome::Commit(Ok(()))
		});

		// Act
		let location = hydradx_runtime::CurrencyIdConvert::convert(original_asset_id);
		assert!(location.is_some());
		let converted_asset_id = hydradx_runtime::CurrencyIdConvert::convert(location.unwrap());

		// Assert
		// It should get back the original asset ID
		assert!(converted_asset_id.is_some());
		assert_eq!(converted_asset_id.unwrap(), original_asset_id);
	});
}

// Foreign assets (parents: 1)
#[test]
fn convert_asset_id_to_location_should_work_for_foreign_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let asset_id: AssetId = ACA;
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			asset_id,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				V3Junctions::X2(
					V3Junction::Parachain(ACALA_PARA_ID),
					V3Junction::GeneralIndex(0)
				)
			))
		));

		// Act
		let location = hydradx_runtime::CurrencyIdConvert::convert(asset_id);

		// Assert
		// It should return the stored foreign location
		assert!(location.is_some());
		let loc = location.unwrap();
		assert_eq!(loc.parents, 1);

		// Check that it's the Acala location
		match loc.interior {
			Junctions::X2(ref junctions) => {
				let junctions_slice = junctions.as_ref();
				assert!(matches!(
					junctions_slice[0],
					Parachain(id) if id == ACALA_PARA_ID
				));
				assert!(matches!(junctions_slice[1], GeneralIndex(0)));
			}
			_ => panic!("Expected X2 junction"),
		}
	});
}

#[test]
fn convert_location_to_asset_id_should_work_for_foreign_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let asset_id: AssetId = ACA;
		let foreign_location = MultiLocation::new(
			1,
			V3Junctions::X2(V3Junction::Parachain(ACALA_PARA_ID), V3Junction::GeneralIndex(0)),
		);
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			asset_id,
			hydradx_runtime::AssetLocation(foreign_location.clone())
		));

		// Convert v3 MultiLocation to v4 Location
		let location_v4 = Location::try_from(foreign_location).expect("should convert");

		// Act
		// Convert v4 location to asset ID
		let result = hydradx_runtime::CurrencyIdConvert::convert(location_v4);

		// Assert
		assert!(result.is_some());
		assert_eq!(result.unwrap(), asset_id);
	});
}

#[test]
fn convert_asset_id_to_location_should_return_none_for_unregistered_foreign_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		// Use an unregistered asset ID
		let asset_id: AssetId = 9999;

		// Act
		let location = hydradx_runtime::CurrencyIdConvert::convert(asset_id);

		// Assert
		assert!(location.is_none());
	});
}

#[test]
fn convert_location_to_asset_id_should_return_none_for_unregistered_foreign_location() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let unregistered_location = Location {
			parents: 1,
			interior: Junctions::X2(Arc::new([Junction::Parachain(9999), Junction::GeneralIndex(0)])),
		};

		// Act
		let result = hydradx_runtime::CurrencyIdConvert::convert(unregistered_location);

		// Assert
		assert!(result.is_none());
	});
}

#[test]
fn roundtrip_conversion_should_work_for_foreign_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		let original_asset_id: AssetId = ACA;
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			original_asset_id,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				V3Junctions::X2(V3Junction::Parachain(ACALA_PARA_ID), V3Junction::GeneralIndex(0))
			))
		));

		// Act
		let location = hydradx_runtime::CurrencyIdConvert::convert(original_asset_id);
		assert!(location.is_some());
		let converted_asset_id = hydradx_runtime::CurrencyIdConvert::convert(location.unwrap());

		// Assert
		// It should get back the original asset ID
		assert!(converted_asset_id.is_some());
		assert_eq!(converted_asset_id.unwrap(), original_asset_id);
	});
}
