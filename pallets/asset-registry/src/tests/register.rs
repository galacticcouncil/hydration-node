use super::*;

use crate::types::{AssetType, Metadata};
use mock::Registry;
use polkadot_xcm::v3::{
	Junction::{self, Parachain},
	Junctions::X2,
	MultiLocation,
};
use pretty_assertions::assert_eq;

#[test]
fn register_should_work_when_all_params_are_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		assert_eq!(
			Registry::assets(asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit),
				metadata: Some(AssetMetadata {
					symbol: Registry::to_bounded_name(metadata.symbol.clone()).unwrap(),
					decimals: metadata.decimals
				})
			})
		);

		assert_eq!(Registry::asset_ids(bounded_name.clone()), Some(asset_id));

		assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
		assert_eq!(Registry::locations(asset_id), Some(asset_location.clone()));

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::MetadataSet {
				asset_id,
				metadata: Some(metadata)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::LocationSet {
				asset_id,
				location: asset_location
			}
			.into()
		));
	});
}

#[test]
fn register_should_work_when_asset_id_is_non_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 1 * UNIT;

		let expected_asset_id = crate::NextAssetId::<Test>::get() + <Test as Config>::SequentialIdStartAt::get() + 1;
		let key = Junction::from(BoundedVec::try_from(expected_asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			None,
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		assert_eq!(
			Registry::assets(expected_asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit),
				metadata: Some(AssetMetadata {
					symbol: Registry::to_bounded_name(metadata.symbol.clone()).unwrap(),
					decimals: metadata.decimals
				})
			})
		);

		assert_eq!(Registry::asset_ids(bounded_name.clone()), Some(expected_asset_id));

		assert_eq!(
			Registry::location_assets(asset_location.clone()),
			Some(expected_asset_id)
		);
		assert_eq!(Registry::locations(expected_asset_id), Some(asset_location.clone()));

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id: expected_asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::MetadataSet {
				asset_id: expected_asset_id,
				metadata: Some(metadata)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::LocationSet {
				asset_id: expected_asset_id,
				location: asset_location
			}
			.into()
		));
	});
}

#[test]
fn register_should_work_when_metadata_is_not_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let name = b"Test asset".to_vec();
		let xcm_rate_limit = 1_000;
		let existential_deposit = 1 * UNIT;

		let expected_asset_id = crate::NextAssetId::<Test>::get() + <Test as Config>::SequentialIdStartAt::get() + 1;
		let key = Junction::from(BoundedVec::try_from(expected_asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			None,
			None,
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		assert_eq!(
			Registry::assets(expected_asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit),
				metadata: None,
			})
		);

		assert_eq!(Registry::asset_ids(bounded_name.clone()), Some(expected_asset_id));

		assert_eq!(
			Registry::location_assets(asset_location.clone()),
			Some(expected_asset_id)
		);
		assert_eq!(Registry::locations(expected_asset_id), Some(asset_location.clone()));

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id: expected_asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit)
			}
			.into()
		));

		assert!(!has_event(
			Event::<Test>::MetadataSet {
				asset_id: expected_asset_id,
				metadata: None
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::LocationSet {
				asset_id: expected_asset_id,
				location: asset_location
			}
			.into()
		));
	});
}

#[test]
fn register_should_work_when_location_is_not_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			None,
			Some(xcm_rate_limit)
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		assert_eq!(
			Registry::assets(asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit),
				metadata: Some(AssetMetadata {
					symbol: Registry::to_bounded_name(metadata.symbol.clone()).unwrap(),
					decimals: metadata.decimals
				})
			})
		);

		assert_eq!(Registry::asset_ids(bounded_name.clone()), Some(asset_id));

		assert_eq!(Registry::locations(asset_id), None);

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: Some(xcm_rate_limit)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::MetadataSet {
				asset_id,
				metadata: Some(metadata)
			}
			.into()
		));
	});
}

#[test]
fn register_should_work_when_xmc_rate_limit_is_not_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			None
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		assert_eq!(
			Registry::assets(asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: None,
				metadata: Some(AssetMetadata {
					symbol: Registry::to_bounded_name(metadata.symbol.clone()).unwrap(),
					decimals: metadata.decimals
				})
			})
		);

		assert_eq!(Registry::asset_ids(bounded_name.clone()), Some(asset_id));

		assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
		assert_eq!(Registry::locations(asset_id), Some(asset_location.clone()));

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: None
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::MetadataSet {
				asset_id,
				metadata: Some(metadata)
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::LocationSet {
				asset_id,
				location: asset_location
			}
			.into()
		));
	});
}

#[test]
fn register_should_fail_when_asset_name_is_already_used() {
	ExtBuilder::default()
		.with_assets(vec![(Some(b"Test asset".to_vec()), 1 * UNIT, Some(2))])
		.build()
		.execute_with(|| {
			let asset_id = 1;
			let name = b"Test asset".to_vec();
			let metadata = Metadata {
				symbol: b"TKN".to_vec(),
				decimals: 12,
			};
			let xcm_rate_limit = 1_000;
			let existential_deposit = 10_000;

			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act & assert
			assert_noop!(
				Registry::register(
					RuntimeOrigin::root(),
					Some(name.clone()),
					AssetType::Token,
					existential_deposit,
					Some(asset_id),
					Some(metadata.clone()),
					Some(asset_location.clone()),
					Some(xcm_rate_limit)
				),
				Error::<Test>::AssetAlreadyRegistered
			);
		});
}

#[test]
fn register_should_fail_when_asset_id_is_already_used() {
	ExtBuilder::default()
		.with_assets(vec![(Some(b"Test".to_vec()), 1 * UNIT, Some(2))])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let name = b"Test asset".to_vec();
			let metadata = Metadata {
				symbol: b"TKN".to_vec(),
				decimals: 12,
			};
			let xcm_rate_limit = 1_000;
			let existential_deposit = 10_000;

			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act & assert
			assert_noop!(
				Registry::register(
					RuntimeOrigin::root(),
					Some(name.clone()),
					AssetType::Token,
					existential_deposit,
					Some(asset_id),
					Some(metadata.clone()),
					Some(asset_location.clone()),
					Some(xcm_rate_limit)
				),
				Error::<Test>::AssetAlreadyRegistered
			);
		});
}

#[test]
fn register_should_fail_when_origin_is_not_allowed() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_noop!(Registry::register(
			RuntimeOrigin::signed(ALICE),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		), DispatchError::BadOrigin);
	});
}

#[test]
fn register_should_fail_when_origin_is_none() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_noop!(Registry::register(
			RuntimeOrigin::none(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		), DispatchError::BadOrigin);
	});
}

#[test]
fn register_should_fail_when_provided_asset_id_is_not_from_reserved_range() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = <Test as Config>::SequentialIdStartAt::get();
		let name = b"Test asset".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_noop!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		), Error::<Test>::NotInReservedRange);
    });
}

#[test]
fn register_should_fail_when_asset_name_is_too_long() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_id = 1;
		let name = b"Too long asset name".to_vec();
		let metadata = Metadata {
			symbol: b"TKN".to_vec(),
			decimals: 12,
		};
		let xcm_rate_limit = 1_000;
		let existential_deposit = 10_000;

		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_noop!(Registry::register(
			RuntimeOrigin::root(),
			Some(name.clone()),
			AssetType::Token,
			existential_deposit,
			Some(asset_id),
			Some(metadata.clone()),
			Some(asset_location.clone()),
			Some(xcm_rate_limit)
		), Error::<Test>::TooLong);
	});
}

#[test]
fn register_should_work_when_only_required_params_are_provided() {
	ExtBuilder::default().build().execute_with(|| {
		let existential_deposit = 10_000;
		let expected_asset_id = crate::NextAssetId::<Test>::get() + <Test as Config>::SequentialIdStartAt::get() + 1;

		//Act
		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			None,
			AssetType::Token,
			existential_deposit,
			None,
			None,
			None,
			None));

		//Assert
		assert_eq!(
			Registry::assets(expected_asset_id),
			Some(AssetDetails {
				name: None,
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: None,
				metadata: None,
			})
		);

		assert_eq!(Registry::locations(expected_asset_id), None);

		assert!(has_event(
			Event::<Test>::Registered {
				asset_id: expected_asset_id,
				asset_name: None,
				asset_type: AssetType::Token,
				existential_deposit,
				xcm_rate_limit: None
			}
			.into()
		));
	});
}
