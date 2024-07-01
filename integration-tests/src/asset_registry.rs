#![cfg(test)]

use crate::asset_registry::Junction::GeneralIndex;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::{AssetRegistry as Registry, TechnicalCollective};
use polkadot_xcm::v3::{
	Junction::{self, Parachain},
	Junctions::X2,
	MultiLocation,
};
use pretty_assertions::{assert_eq, assert_ne};
use xcm_emulator::TestExt;

#[test]
fn root_should_update_decimals_when_it_was_already_set() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let new_decimals = 53_u8;

		assert_ne!(Registry::assets(HDX).unwrap().decimals.unwrap(), new_decimals);

		assert_ok!(Registry::update(
			RawOrigin::Root.into(),
			HDX,
			None,
			None,
			None,
			None,
			None,
			None,
			Some(new_decimals),
			None
		));

		assert_eq!(Registry::assets(HDX).unwrap().decimals.unwrap(), new_decimals);
	});
}

#[test]
fn root_should_update_location_when_asset_exists() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert!(Registry::locations(LRNA).is_none());

		let loc_1 =
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(0))));

		//Set location 1-th time.
		assert_ok!(Registry::update(
			RawOrigin::Root.into(),
			LRNA,
			None,
			None,
			None,
			None,
			None,
			None,
			None,
			Some(loc_1.clone())
		),);
		assert_eq!(Registry::locations(LRNA).unwrap(), loc_1);
		assert_eq!(Registry::location_assets(loc_1.clone()).unwrap(), LRNA);

		// Update location if it was previously set.
		let loc_2 =
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(INTERLAY_PARA_ID), GeneralIndex(0))));

		assert_ok!(Registry::update(
			RawOrigin::Root.into(),
			LRNA,
			None,
			None,
			None,
			None,
			None,
			None,
			None,
			Some(loc_2.clone())
		),);
		assert_eq!(Registry::locations(LRNA).unwrap(), loc_2);
		assert_eq!(Registry::location_assets(loc_2).unwrap(), LRNA);

		assert!(Registry::location_assets(loc_1).is_none());
	});
}
