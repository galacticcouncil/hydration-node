use super::*;

use hydradx_traits::registry::Create;
use mock::Registry;

use frame_support::storage::with_transaction;
use polkadot_xcm::v3::{
	Junction::{self, Parachain},
	Junctions::X2,
	MultiLocation,
};
use sp_runtime::{DispatchResult, TransactionOutcome};

#[test]
fn register_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let asset_id = 1;
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;
			let is_sufficient = true;

			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(<Registry as Create<Balance>>::register_asset(
				Some(asset_id),
				Some(name.clone()),
				AssetKind::XYK,
				Some(ed),
				Some(symbol.clone()),
				Some(decimals),
				Some(asset_location.clone()),
				Some(xcm_rate_limit),
				is_sufficient
			));

			//Assert
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient
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

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn register_insufficient_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let asset_id = 1;
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;

			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(<Registry as Create<Balance>>::register_insufficient_asset(
				Some(asset_id),
				Some(name.clone()),
				AssetKind::XYK,
				Some(ed),
				Some(symbol.clone()),
				Some(decimals),
				Some(asset_location.clone()),
				Some(xcm_rate_limit),
			));

			//Assert
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient: false
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient: false
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

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn register_sufficient_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let asset_id = 1;
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;

			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(<Registry as Create<Balance>>::register_sufficient_asset(
				Some(asset_id),
				Some(name.clone()),
				AssetKind::XYK,
				ed,
				Some(symbol.clone()),
				Some(decimals),
				Some(asset_location.clone()),
				Some(xcm_rate_limit),
			));

			//Assert
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient: true
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient: true
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

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn get_or_register_asset_should_register_asset_when_does_not_exists() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let new_asset_id = Registry::next_asset_id().unwrap();
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;
			let is_sufficient = true;

			let key = Junction::from(BoundedVec::try_from(new_asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(
				<Registry as Create<Balance>>::get_or_register_asset(
					name.clone(),
					AssetKind::XYK,
					Some(ed),
					Some(symbol.clone()),
					Some(decimals),
					Some(asset_location.clone()),
					Some(xcm_rate_limit),
					is_sufficient
				),
				new_asset_id
			);

			//Assert
			assert_eq!(
				Registry::assets(new_asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(new_asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(new_asset_id));
			assert_eq!(Registry::locations(new_asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id: new_asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient
				}
				.into()
			));

			assert!(has_event(
				Event::<Test>::LocationSet {
					asset_id: new_asset_id,
					location: asset_location
				}
				.into()
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn get_or_register_asset_should_return_asset_id_when_asset_exists() {
	let existing_asset_id = 1_u32;
	ExtBuilder::default()
		.with_assets(vec![(
			Some(existing_asset_id),
			Some(b"Asset".to_vec().try_into().unwrap()),
			UNIT,
			None,
			None,
			None,
			false,
		)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Asset".to_vec().try_into().unwrap();
				let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
				let decimals = 12;
				let xcm_rate_limit = 1_000;
				let ed = 10_000;
				let is_sufficient = true;

				let key = Junction::from(BoundedVec::try_from(1_000.encode()).unwrap());
				let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

				//Act
				assert_ok!(
					<Registry as Create<Balance>>::get_or_register_asset(
						name.clone(),
						AssetKind::XYK,
						Some(ed),
						Some(symbol),
						Some(decimals),
						Some(asset_location),
						Some(xcm_rate_limit),
						is_sufficient
					),
					existing_asset_id
				);

				//Assert
				assert_eq!(
					Registry::assets(existing_asset_id),
					Some(AssetDetails {
						name: Some(name),
						asset_type: AssetType::Token,
						existential_deposit: UNIT,
						xcm_rate_limit: None,
						symbol: None,
						decimals: None,
						is_sufficient: false
					})
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn get_or_register_sufficient_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let new_asset_id = Registry::next_asset_id().unwrap();
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;

			let key = Junction::from(BoundedVec::try_from(new_asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(<Registry as Create<Balance>>::get_or_register_sufficient_asset(
				name.clone(),
				AssetKind::XYK,
				ed,
				Some(symbol.clone()),
				Some(decimals),
				Some(asset_location.clone()),
				Some(xcm_rate_limit),
			),);

			//Assert
			assert_eq!(
				Registry::assets(new_asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient: true
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(new_asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(new_asset_id));
			assert_eq!(Registry::locations(new_asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id: new_asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient: true
				}
				.into()
			));

			assert!(has_event(
				Event::<Test>::LocationSet {
					asset_id: new_asset_id,
					location: asset_location
				}
				.into()
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn get_or_register_insufficient_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let _ = with_transaction(|| {
			let new_asset_id = Registry::next_asset_id().unwrap();
			let name: BoundedVec<u8, mock::RegistryStringLimit> = b"Test asset".to_vec().try_into().unwrap();
			let symbol: BoundedVec<u8, mock::RegistryStringLimit> = b"TKN".to_vec().try_into().unwrap();
			let decimals = 12;
			let xcm_rate_limit = 1_000;
			let ed = 10_000;

			let key = Junction::from(BoundedVec::try_from(new_asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

			//Act
			assert_ok!(<Registry as Create<Balance>>::get_or_register_insufficient_asset(
				name.clone(),
				AssetKind::XYK,
				Some(ed),
				Some(symbol.clone()),
				Some(decimals),
				Some(asset_location.clone()),
				Some(xcm_rate_limit),
			),);

			//Assert
			assert_eq!(
				Registry::assets(new_asset_id),
				Some(AssetDetails {
					name: Some(name.clone()),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol.clone()),
					decimals: Some(decimals),
					is_sufficient: false
				})
			);

			assert_eq!(Registry::asset_ids(name.clone()), Some(new_asset_id));

			assert_eq!(Registry::location_assets(asset_location.clone()), Some(new_asset_id));
			assert_eq!(Registry::locations(new_asset_id), Some(asset_location.clone()));

			assert!(has_event(
				Event::<Test>::Registered {
					asset_id: new_asset_id,
					asset_name: Some(name),
					asset_type: AssetType::XYK,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(symbol),
					decimals: Some(decimals),
					is_sufficient: false
				}
				.into()
			));

			assert!(has_event(
				Event::<Test>::LocationSet {
					asset_id: new_asset_id,
					location: asset_location
				}
				.into()
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
