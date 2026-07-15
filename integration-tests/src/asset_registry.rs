#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hex_literal::hex;
use hydradx_runtime::AssetRegistry as Registry;
use polkadot_xcm::v5::{
	Junction::{AccountKey20, GeneralIndex, PalletInstance, Parachain},
	Location,
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

		let loc_1 = hydradx_runtime::AssetLocation(Location {
			parents: 1,
			interior: [Parachain(MOONBEAM_PARA_ID), GeneralIndex(0)].into(),
		});

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
		let loc_2 = hydradx_runtime::AssetLocation(Location {
			parents: 1,
			interior: [Parachain(INTERLAY_PARA_ID), GeneralIndex(0)].into(),
		});

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

// Registering a Taifoon-native asset (xcFOON) on Hydration.
//
// Taifoon is a Moonbeam fork, so its native currency uses the Moonbeam-family multilocation shape:
// `{ parents: 1, interior: X2(Parachain(TAIFOON_PARA_ID), PalletInstance(10)) }` — PalletInstance 10
// is the Balances pallet on Moonbeam-family runtimes, i.e. the native token (18 decimals). This is
// the on-chain `assetRegistry.register` a GeneralAdmin referendum would submit to accept xcFOON.
#[test]
fn root_should_register_xcfoon_from_taifoon() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let foon_location = hydradx_runtime::AssetLocation(Location {
			parents: 1,
			interior: [Parachain(TAIFOON_PARA_ID), PalletInstance(10)].into(),
		});

		// The location must not resolve before registration.
		assert!(Registry::location_assets(foon_location.clone()).is_none());

		assert_ok!(Registry::register(
			RawOrigin::Root.into(),
			None,                                    // asset_id: auto
			Some(b"Taifoon FOON".to_vec().try_into().unwrap()),
			pallet_asset_registry::AssetType::Token, // asset_type
			None,                                    // existential_deposit: default
			Some(b"xcFOON".to_vec().try_into().unwrap()),
			Some(18),                                // decimals (FOON = 18)
			Some(foon_location.clone()),             // location
			None,                                    // xcm_rate_limit
			false,                                   // is_sufficient
		));

		// The location now resolves to a local asset id, and back.
		let foon_id = Registry::location_assets(foon_location.clone()).expect("xcFOON registered");
		assert_eq!(Registry::locations(foon_id).unwrap(), foon_location);
	});
}

// Pointing Hydration into MRL (Moonbeam Routed Liquidity) via Taifoon.
//
// Hydration already reaches Ethereum-ecosystem liquidity through Moonbeam: `weth_asset_location()`
// in `runtime/hydradx/src/evm/mod.rs` registers WETH as an ERC-20 living behind Moonbeam's
// ERC20-XCM bridge pallet, with the shape
//   `{ parents: 1, X3(Parachain(MOONBEAM_PARA_ID), PalletInstance(110), AccountKey20(erc20)) }`
// — PalletInstance(110) is the `erc20xcm_bridge`/xTokens pallet on Moonbeam-family runtimes, and
// AccountKey20 is the wrapped ERC-20's H160 address. That triple IS the MRL routing key.
//
// Taifoon is a Moonbeam fork, so it exposes the SAME pallet at the SAME instance (110). To route
// the same MRL asset through Taifoon instead, we copy Moonbeam's location verbatim and only swap the
// Parachain id — exactly the "re-wire, don't re-discover" inheritance. This test registers an
// MRL-routed ERC-20 (the canonical WETH H160) behind Taifoon's ERC20-XCM bridge and asserts the
// location↔asset round-trip, mirroring how the runtime pins Moonbeam's WETH.
#[test]
fn root_should_register_mrl_erc20_routed_via_taifoon() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Same ERC-20 (WETH H160) + same MRL bridge pallet (110) as Moonbeam — only the parachain differs.
		let weth_via_taifoon = hydradx_runtime::AssetLocation(Location {
			parents: 1,
			interior: [
				Parachain(TAIFOON_PARA_ID),
				PalletInstance(110),
				AccountKey20 {
					network: None,
					key: hex!["ab3f0245b83feb11d15aaffefd7ad465a59817ed"],
				},
			]
			.into(),
		});

		assert!(Registry::location_assets(weth_via_taifoon.clone()).is_none());

		assert_ok!(Registry::register(
			RawOrigin::Root.into(),
			None,
			Some(b"WETH (via Taifoon MRL)".to_vec().try_into().unwrap()),
			pallet_asset_registry::AssetType::Token,
			None,
			Some(b"WETH".to_vec().try_into().unwrap()),
			Some(18), // WETH = 18 decimals
			Some(weth_via_taifoon.clone()),
			None,
			false,
		));

		// The MRL location resolves to a local asset id, and back — Taifoon is now a live MRL router.
		let weth_id = Registry::location_assets(weth_via_taifoon.clone()).expect("MRL WETH registered");
		assert_eq!(Registry::locations(weth_id).unwrap(), weth_via_taifoon);
	});
}
