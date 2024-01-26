use super::*;

use hydradx_traits::registry::Mutate;
use mock::Registry;

use polkadot_xcm::v3::{
	Junction::{self, Parachain},
	Junctions::X2,
	MultiLocation,
};

#[test]
fn set_location_should_work_when_location_was_not_set_yet() {
	let asset_id = 1_u32;
	ExtBuilder::default()
		.with_assets(vec![(
			Some(asset_id),
			Some(b"Suff".to_vec().try_into().unwrap()),
			UNIT,
			None,
			None,
			None,
			true,
		)])
		.build()
		.execute_with(|| {
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			assert_eq!(Registry::locations(asset_id), None);

			//Act
			assert_ok!(<Registry as Mutate>::set_location(asset_id, location.clone()));

			//Assert
			assert_eq!(Registry::location_assets(location.clone()), Some(asset_id));
			assert_eq!(Registry::locations(asset_id), Some(location));
		});
}

#[test]
fn set_location_should_not_work_when_location_was_not() {
	let asset_id = 1_u32;
	ExtBuilder::default()
		.with_assets(vec![(
			Some(asset_id),
			Some(b"Suff".to_vec().try_into().unwrap()),
			UNIT,
			None,
			None,
			None,
			true,
		)])
		.build()
		.execute_with(|| {
			//Arrange
			let key = Junction::from(BoundedVec::try_from(asset_id.encode()).unwrap());
			let location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));
			Pallet::<Test>::set_location(asset_id, location.clone()).unwrap();

			//Act
			assert_noop!(
				<Registry as Mutate>::set_location(asset_id, location),
				Error::<Test>::LocationAlreadyRegistered
			);
		});
}

#[test]
fn set_location_should_not_work_when_asset_does_not_exists() {
	let non_existing_id = 190_u32;
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let key = Junction::from(BoundedVec::try_from(non_existing_id.encode()).unwrap());
		let location = AssetLocation(MultiLocation::new(0, X2(Parachain(200), key)));

		//Act
		assert_noop!(
			<Registry as Mutate>::set_location(non_existing_id, location),
			Error::<Test>::AssetNotFound
		);
	});
}
