#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::traits::ContainsPair;
use hydradx_runtime::XcmConfig;
use polkadot_xcm::v4::{prelude::*, Junctions::*};
use xcm_emulator::TestExt;

#[test]
fn aliasing_child_locations() {
	Hydra::execute_with(|| {
		// Allows aliasing descendant of origin.
		let origin = Location::new(1, X1([PalletInstance(8)].into()));
		let target = Location::new(1, X2([PalletInstance(8), GeneralIndex(9)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(8)].into()));
		let target = Location::new(
			1,
			X2([
				Parachain(8),
				AccountId32 {
					network: None,
					id: [1u8; 32],
				},
			]
			.into()),
		);
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(8)].into()));
		let target = Location::new(1, X3([Parachain(8), PalletInstance(8), GeneralIndex(9)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));

		// Does not allow if not descendant.
		let origin = Location::new(1, X1([PalletInstance(8)].into()));
		let target = Location::new(0, X2([PalletInstance(8), GeneralIndex(9)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(8)].into()));
		let target = Location::new(
			0,
			X2([
				Parachain(8),
				AccountId32 {
					network: None,
					id: [1u8; 32],
				},
			]
			.into()),
		);
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(8)].into()));
		let target = Location::new(
			0,
			X1([AccountId32 {
				network: None,
				id: [1u8; 32],
			}]
			.into()),
		);
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(
			1,
			X1([AccountId32 {
				network: None,
				id: [1u8; 32],
			}]
			.into()),
		);
		let target = Location::new(
			0,
			X1([AccountId32 {
				network: None,
				id: [1u8; 32],
			}]
			.into()),
		);
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
	});
}

#[test]
fn asset_hub_root_aliases_anything() {
	Hydra::execute_with(|| {
		// Allows AH root to alias anything.
		let origin = Location::new(1, X1([Parachain(1000)].into()));

		let target = Location::new(1, X1([Parachain(2000)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(
			1,
			X1([AccountId32 {
				network: None,
				id: [1u8; 32],
			}]
			.into()),
		);
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(
			1,
			X2([
				Parachain(8),
				AccountId32 {
					network: None,
					id: [1u8; 32],
				},
			]
			.into()),
		);
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(1, X3([Parachain(42), PalletInstance(8), GeneralIndex(9)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(2, X1([GlobalConsensus(Ethereum { chain_id: 1 })].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(2, X2([GlobalConsensus(Kusama), Parachain(1000)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(0, X2([PalletInstance(8), GeneralIndex(9)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));

		// Other AH locations cannot alias anything.
		let origin = Location::new(1, X2([Parachain(1000), GeneralIndex(9)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X2([Parachain(1000), PalletInstance(9)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(
			1,
			X2([
				Parachain(1000),
				AccountId32 {
					network: None,
					id: [1u8; 32],
				},
			]
			.into()),
		);
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));

		// Other root locations cannot alias anything.
		let origin = Location::new(1, Here);
		let target = Location::new(2, X1([GlobalConsensus(Ethereum { chain_id: 1 })].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(2, X2([GlobalConsensus(Kusama), Parachain(1000)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(0, X2([PalletInstance(8), GeneralIndex(9)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));

		let origin = Location::new(0, Here);
		let target = Location::new(1, X1([Parachain(2000)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(1001)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let origin = Location::new(1, X1([Parachain(1002)].into()));
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
	});
}
