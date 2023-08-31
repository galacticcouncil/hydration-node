use super::*;

use crate::types::AssetType;
use mock::Registry;
use polkadot_xcm::v3::{
	Junction::{self, Parachain},
	Junctions::X2,
	MultiLocation,
};
use pretty_assertions::assert_eq;

#[test]
fn update_should_work_when_asset_exists() {
	let old_asset_name = b"Tkn2".to_vec();
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(old_asset_name.clone()), UNIT, None, None, true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let name = b"New Tkn 2".to_vec();
			let ed = 10_000 * UNIT;
			let xcm_rate_limit = 463;
			let symbol = b"nTkn2".to_vec();
			let decimals = 23;
			let is_sufficient = false;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location.clone()).unwrap();

			//Act
			assert_ok!(Registry::update(
				RuntimeOrigin::root(),
				asset_id,
				Some(name.clone()),
				Some(AssetType::External),
				Some(ed),
				Some(xcm_rate_limit),
				Some(is_sufficient),
				Some(symbol.clone()),
				Some(decimals),
			));

			//Assert
			let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
			let bounded_symbol = Pallet::<Test>::to_bounded_name(symbol).unwrap();
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: Some(bounded_name.clone()),
					asset_type: AssetType::External,
					existential_deposit: ed,
					xcm_rate_limit: Some(xcm_rate_limit),
					symbol: Some(bounded_symbol.clone()),
					decimals: Some(decimals),
					is_sufficient: false
				})
			);

			//NOTE: location should't change
			assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(asset_location));

			let old_bounded_name = Pallet::<Test>::to_bounded_name(old_asset_name).unwrap();
			assert_eq!(Registry::asset_ids(bounded_name.clone()).unwrap(), asset_id);
			assert!(Registry::asset_ids(old_bounded_name).is_none());

			assert_last_event!(Event::<Test>::Updated {
				asset_id,
				asset_name: Some(bounded_name),
				asset_type: AssetType::External,
				existential_deposit: ed,
				xcm_rate_limit: Some(xcm_rate_limit),
				decimals: Some(decimals),
				symbol: Some(bounded_symbol),
				is_sufficient,
			}
			.into());
		});
}

#[test]
fn update_should_update_provided_params_when_values_was_previously_set() {
	let old_asset_name = b"Tkn2".to_vec();
	ExtBuilder::default().with_assets(vec![]).build().execute_with(|| {
		//Arrange
		let asset_id = 1;
		let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
		let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		assert_ok!(Registry::register(
			RuntimeOrigin::root(),
			Some(asset_id),
			Some(b"Test asset".to_vec()),
			AssetType::Token,
			Some(10_000),
			Some(b"TKN".to_vec()),
			Some(12),
			Some(asset_location.clone()),
			Some(1_000),
			true
		));

		let name = b"New name".to_vec();
		let ed = 20_000 * UNIT;
		let xcm_rate_limit = 463;
		let symbol = b"nTkn".to_vec();
		let decimals = 23;
		let is_sufficient = false;

		//Act
		assert_ok!(Registry::update(
			RuntimeOrigin::root(),
			asset_id,
			Some(name.clone()),
			Some(AssetType::External),
			Some(ed),
			Some(xcm_rate_limit),
			Some(is_sufficient),
			Some(symbol.clone()),
			Some(decimals),
		));

		//Assert
		let bounded_name = Pallet::<Test>::to_bounded_name(name).unwrap();
		let bounded_symbol = Pallet::<Test>::to_bounded_name(symbol).unwrap();
		assert_eq!(
			Registry::assets(asset_id),
			Some(AssetDetails {
				name: Some(bounded_name.clone()),
				asset_type: AssetType::External,
				existential_deposit: ed,
				xcm_rate_limit: Some(xcm_rate_limit),
				symbol: Some(bounded_symbol.clone()),
				decimals: Some(decimals),
				is_sufficient: false
			})
		);

		//NOTE: location should't change
		assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
		assert_eq!(Registry::locations(asset_id), Some(asset_location));

		let old_bounded_name = Pallet::<Test>::to_bounded_name(old_asset_name).unwrap();
		assert_eq!(Registry::asset_ids(bounded_name.clone()).unwrap(), asset_id);
		assert!(Registry::asset_ids(old_bounded_name).is_none());

		assert_last_event!(Event::<Test>::Updated {
			asset_id,
			asset_name: Some(bounded_name),
			asset_type: AssetType::External,
			existential_deposit: ed,
			xcm_rate_limit: Some(xcm_rate_limit),
			decimals: Some(decimals),
			symbol: Some(bounded_symbol),
			is_sufficient,
		}
		.into());
	});
}

#[test]
fn update_should_not_change_values_when_param_is_none() {
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(b"Tkn2".to_vec()), UNIT, None, None, true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location.clone()).unwrap();

			let details_0 = Registry::assets(asset_id).unwrap();

			//Act
			assert_ok!(Registry::update(
				RuntimeOrigin::root(),
				asset_id,
				None,
				None,
				None,
				None,
				None,
				None,
				None,
			));

			//Assert
			assert_eq!(Registry::assets(asset_id).unwrap(), details_0);

			let old_bounded_name = Pallet::<Test>::to_bounded_name(b"Tkn2".to_vec()).unwrap();
			assert_eq!(Registry::asset_ids(old_bounded_name).unwrap(), asset_id);

			//NOTE: location should't change
			assert_eq!(Registry::location_assets(asset_location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(asset_location));

			assert_last_event!(Event::<Test>::Updated {
				asset_id,
				asset_name: details_0.name,
				asset_type: details_0.asset_type,
				existential_deposit: details_0.existential_deposit,
				xcm_rate_limit: details_0.xcm_rate_limit,
				decimals: details_0.decimals,
				symbol: details_0.symbol,
				is_sufficient: details_0.is_sufficient,
			}
			.into());
		});
}

#[test]
fn update_origin_should_set_decimals_if_its_none() {
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(b"Tkn2".to_vec()), UNIT, None, None, true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let decimals = 52;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location).unwrap();

			let details_0 = Registry::assets(asset_id).unwrap();

			//NOTE: update origin is ste to ensure_signed
			//Act
			assert_ok!(Registry::update(
				RuntimeOrigin::signed(ALICE),
				asset_id,
				None,
				None,
				None,
				None,
				None,
				None,
				Some(decimals),
			));

			//Assert
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: details_0.name.clone(),
					asset_type: details_0.asset_type,
					existential_deposit: details_0.existential_deposit,
					xcm_rate_limit: details_0.xcm_rate_limit,
					symbol: details_0.symbol.clone(),
					decimals: Some(decimals),
					is_sufficient: details_0.is_sufficient
				})
			);

			assert_last_event!(Event::<Test>::Updated {
				asset_id,
				asset_name: details_0.name,
				asset_type: details_0.asset_type,
				existential_deposit: details_0.existential_deposit,
				xcm_rate_limit: details_0.xcm_rate_limit,
				decimals: Some(decimals),
				symbol: details_0.symbol,
				is_sufficient: details_0.is_sufficient,
			}
			.into());
		});
}

#[test]
fn update_origin_should_not_chane_decimals_if_its_some() {
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(b"Tkn2".to_vec()), UNIT, None, Some(3), true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let decimals = 52;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location).unwrap();

			//NOTE: update origin is ste to ensure_signed
			//Act & assert
			assert_noop!(
				Registry::update(
					RuntimeOrigin::signed(ALICE),
					asset_id,
					None,
					None,
					None,
					None,
					None,
					None,
					Some(decimals),
				),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn create_origin_should_always_set_decimals() {
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(b"Tkn2".to_vec()), UNIT, None, Some(3), true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let decimals = 52;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location).unwrap();

			let details_0 = Registry::assets(asset_id).unwrap();

			//NOTE: update origin is ste to ensure_signed
			//Act
			assert_ok!(Registry::update(
				RuntimeOrigin::root(),
				asset_id,
				None,
				None,
				None,
				None,
				None,
				None,
				Some(decimals),
			));

			assert_ok!(Registry::update(
				RuntimeOrigin::root(),
				asset_id,
				None,
				None,
				None,
				None,
				None,
				None,
				Some(u8::max_value()),
			));

			//Assert
			assert_eq!(
				Registry::assets(asset_id),
				Some(AssetDetails {
					name: details_0.name.clone(),
					asset_type: details_0.asset_type,
					existential_deposit: details_0.existential_deposit,
					xcm_rate_limit: details_0.xcm_rate_limit,
					symbol: details_0.symbol.clone(),
					decimals: Some(u8::max_value()),
					is_sufficient: details_0.is_sufficient
				})
			);

			assert_last_event!(Event::<Test>::Updated {
				asset_id,
				asset_name: details_0.name,
				asset_type: details_0.asset_type,
				existential_deposit: details_0.existential_deposit,
				xcm_rate_limit: details_0.xcm_rate_limit,
				decimals: Some(u8::max_value()),
				symbol: details_0.symbol,
				is_sufficient: details_0.is_sufficient,
			}
			.into());
		});
}

#[test]
fn update_should_fail_when_name_is_already_used() {
	let old_asset_name = b"Tkn2".to_vec();
	ExtBuilder::default()
		.with_assets(vec![
			(Some(1), Some(b"Tkn1".to_vec()), UNIT, None, None, true),
			(Some(2), Some(old_asset_name), UNIT, None, None, true),
			(Some(3), Some(b"Tkn3".to_vec()), UNIT, None, None, true),
		])
		.build()
		.execute_with(|| {
			let asset_id = 2;
			let name = b"Tkn3".to_vec();
			let ed = 10_000 * UNIT;
			let xcm_rate_limit = 463;
			let symbol = b"nTkn2".to_vec();
			let decimals = 23;
			let is_sufficient = false;

			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let asset_location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, asset_location).unwrap();

			//Act
			assert_noop!(
				Registry::update(
					RuntimeOrigin::root(),
					asset_id,
					Some(name),
					Some(AssetType::External),
					Some(ed),
					Some(xcm_rate_limit),
					Some(is_sufficient),
					Some(symbol),
					Some(decimals),
				),
				Error::<Test>::AssetAlreadyRegistered
			);
		});
}
