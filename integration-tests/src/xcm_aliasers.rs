#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::traits::ContainsPair;
use hydradx_runtime::XcmConfig;
use polkadot_xcm::v5::{prelude::*, Junctions::*};
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
fn asset_hub_root_can_alias_whitelisted_locations() {
	Hydra::execute_with(|| {
		// Allows AH root to alias anything.
		let origin = Location::new(1, X1([Parachain(1000)].into()));

		let target = Location::new(1, X1([Parachain(1001)].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(2, X1([GlobalConsensus(Ethereum { chain_id: 1 })].into()));
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
		let target = Location::new(2, X1([GlobalConsensus(Kusama)].into()));
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

#[test]
fn asset_hub_root_cannot_alias_non_whitelisted_locations() {
	Hydra::execute_with(|| {
		// Asset Hub root origin (trusted)
		let origin = Location::new(1, X1([Parachain(1000)].into()));

		// Non-system parachain
		let target = Location::new(1, X1([Parachain(2000)].into()));
		assert!(
			!<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must NOT alias arbitrary parachain 2000"
		);

		// Polkadot relay chain
		let target = Location::new(1, X1([GlobalConsensus(NetworkId::Polkadot)].into()));
		assert!(
			!<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must NOT alias Polkadot relay"
		);

		// Random global consensus (e.g. Kusama)
		let target = Location::new(1, X1([GlobalConsensus(NetworkId::Kusama)].into()));
		assert!(
			!<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must NOT alias Kusama consensus"
		);
	});
}

#[test]
fn asset_hub_root_can_alias_kusama_asset_hub_accounts() {
	Hydra::execute_with(|| {
		// Polkadot Asset Hub root
		let origin = Location::new(1, X1([Parachain(1000)].into()));

		// A specific Kusama Asset Hub account
		let target = Location::new(
			2,
			X3([
				GlobalConsensus(Kusama),
				Parachain(1000),
				AccountId32 {
					network: None,
					id: [42u8; 32],
				},
			]
			.into()),
		);

		// Should be allowed to
		assert!(<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		));
	});
}

#[test]
fn asset_hub_root_cannot_alias_other_kusama_parachain_accounts() {
	Hydra::execute_with(|| {
		// Polkadot Asset Hub root
		let origin = Location::new(1, X1([Parachain(1000)].into()));

		// A specific Kusama Asset Hub account
		let target = Location::new(
			2,
			X3([
				GlobalConsensus(Kusama),
				Parachain(2000),
				AccountId32 {
					network: None,
					id: [42u8; 32],
				},
			]
			.into()),
		);
		assert!(!<XcmConfig as xcm_executor::Config>::Aliasers::contains(
			&origin, &target
		),);
	});
}

#[test]
fn asset_hub_root_can_alias_ethereum_accounts() {
	Hydra::execute_with(|| {
		let origin = Location::new(1, X1([Parachain(1000)].into()));
		let target = Location::new(2, X1([GlobalConsensus(Ethereum { chain_id: 1 })].into()));
		assert!(
			<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must be able to alias Ethereum consensus root"
		);

		// Ethereum account (AccountKey20)
		let target = Location::new(
			2,
			X2([
				GlobalConsensus(Ethereum { chain_id: 1 }),
				AccountKey20 {
					network: None,
					key: [0x42u8; 20],
				},
			]
			.into()),
		);
		assert!(
			<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must be able to alias Ethereum accounts"
		);

		// Different Ethereum chain
		let target = Location::new(
			2,
			X2([
				GlobalConsensus(Ethereum { chain_id: 137 }), // Polygon
				AccountKey20 {
					network: None,
					key: [0xAAu8; 20],
				},
			]
			.into()),
		);
		assert!(
			<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must be able to alias accounts on different Ethereum chains"
		);

		// Deeper Ethereum location (e.g., with GeneralIndex for token)
		let target = Location::new(
			2,
			X3([
				GlobalConsensus(Ethereum { chain_id: 1 }),
				AccountKey20 {
					network: None,
					key: [0x42u8; 20],
				},
				GeneralIndex(123),
			]
			.into()),
		);
		assert!(
			<XcmConfig as xcm_executor::Config>::Aliasers::contains(&origin, &target),
			"Asset Hub root must be able to alias deeper Ethereum locations"
		);
	});
}
