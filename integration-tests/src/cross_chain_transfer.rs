#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};

use polkadot_xcm::{latest::prelude::*, v3::WeightLimit, VersionedMultiAssets, VersionedXcm};

use cumulus_primitives_core::ParaId;
use frame_support::weights::Weight;
use hex_literal::hex;
use orml_traits::currency::MultiCurrency;
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, BlakeTwo256, Hash};
use xcm_emulator::TestExt;

// Determine the hash for assets expected to be have been trapped.
fn determine_hash<M>(origin: &MultiLocation, assets: M) -> H256
where
	M: Into<MultiAssets>,
{
	let versioned = VersionedMultiAssets::from(assets.into());
	BlakeTwo256::hash_of(&(origin, &versioned))
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_polkadot_relay_chain() {
	//Arrange
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));
	});

	PolkadotRelay::execute_with(|| {
		//Act
		assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
			polkadot_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(Parachain(HYDRA_PARA_ID).into_versioned()),
			Box::new(Junction::AccountId32 { id: BOB, network: None }.into()),
			Box::new((Here, 300 * UNITS).into()),
			0,
		));

		//Assert
		assert_eq!(
			polkadot_runtime::Balances::free_balance(AccountIdConversion::<AccountId>::into_account_truncating(
				&ParaId::from(HYDRA_PARA_ID)
			)),
			310 * UNITS
		);
	});

	let fees = 401884032343;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS - fees
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id()),
			fees
		);
	});
}

#[test]
fn polkadot_should_receive_asset_when_sent_from_hydra() {
	//Arrange
	PolkadotRelay::execute_with(|| {
		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)), 0);
	});

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));

		//Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			1,
			3 * UNITS,
			Box::new(MultiLocation::new(1, X1(Junction::AccountId32 { id: BOB, network: None })).into()),
			WeightLimit::Unlimited,
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(ALICE)),
			200 * UNITS - 3 * UNITS
		);
	});

	PolkadotRelay::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			2999978937205 // 3 * HDX - fee
		);
	});
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_acala() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));
	});

	Acala::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	let fee = 321507225875;
	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			30 * UNITS - fee
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id()),
			fee // fees should go to treasury
		);
	});
}

#[test]
fn transfer_from_acala_should_fail_when_transferring_insufficient_amount() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));
	});

	Acala::execute_with(|| {
		assert_noop!(
			hydradx_runtime::XTokens::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				0,
				1_000_000,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(HYDRA_PARA_ID),
							Junction::AccountId32 { id: BOB, network: None }
						)
					)
					.into()
				),
				WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
			),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::XcmExecutionFailed
		);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE
		);
	});

	Hydra::execute_with(|| {
		// Xcm should fail therefore nothing should be deposit into beneficiary account
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			1000 * UNITS
		);
	});
}

#[test]
fn hydra_treasury_should_receive_asset_when_transferred_to_protocol_account() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		// initialize the omnipool because we check whether assets are present there
		init_omnipool();

		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI, // we pretend that the incoming tokens are DAI
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));

		assert_eq!(
			hydradx_runtime::Tokens::free_balance(DAI, &hydradx_runtime::Omnipool::protocol_account()),
			50_000_000_000 * UNITS
		);
	});

	Acala::execute_with(|| {
		// Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 {
							id: hydradx_runtime::Omnipool::protocol_account().into(),
							network: None,
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(DAI, &hydradx_runtime::Omnipool::protocol_account()),
			50_000_000_000 * UNITS
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(DAI, &hydradx_runtime::Treasury::account_id()),
			30 * UNITS // fee and tokens should go to treasury
		);
	});
}

#[test]
fn assets_should_be_trapped_when_assets_are_unknown() {
	TestNet::reset();

	Acala::execute_with(|| {
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	Hydra::execute_with(|| {
		expect_hydra_events(vec![
			cumulus_pallet_xcmp_queue::Event::Fail {
				message_hash: hex!["30291d1dfb68ae6f66d4c841facb78f44e7611ab2a25c84f4fb7347f448d2944"],
				message_id: hex!["30291d1dfb68ae6f66d4c841facb78f44e7611ab2a25c84f4fb7347f448d2944"],
				error: XcmError::AssetNotFound,
				weight: Weight::from_parts(300_000_000, 0),
			}
			.into(),
			pallet_relaychain_info::Event::CurrentBlockNumbers {
				parachain_block_number: 3,
				relaychain_block_number: 7,
			}
			.into(),
		]);
		let origin = MultiLocation::new(1, X1(Parachain(ACALA_PARA_ID)));
		let loc = MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0)));
		let asset: MultiAsset = (loc, 30 * UNITS).into();
		let hash = determine_hash(&origin, vec![asset]);
		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 1);
	});
}

#[test]
fn claim_trapped_asset_should_work() {
	TestNet::reset();

	// traps asset when asset is not registered yet
	let asset = trap_asset();

	// register the asset
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::RuntimeOrigin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));
	});

	claim_asset(asset.clone(), BOB);

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			1000 * UNITS + 29_758_869_580_594
		);

		let origin = MultiLocation::new(1, X1(Parachain(ACALA_PARA_ID)));
		let hash = determine_hash(&origin, vec![asset]);
		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 0);
	});
}

fn trap_asset() -> MultiAsset {
	Acala::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE
		);
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 { id: BOB, network: None }
					)
				)
				.into()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	let loc = MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0)));
	let asset: MultiAsset = (loc, 30 * UNITS).into();

	Hydra::execute_with(|| {
		expect_hydra_events(vec![
			cumulus_pallet_xcmp_queue::Event::Fail {
				message_hash: hex!["30291d1dfb68ae6f66d4c841facb78f44e7611ab2a25c84f4fb7347f448d2944"],
				message_id: hex!["30291d1dfb68ae6f66d4c841facb78f44e7611ab2a25c84f4fb7347f448d2944"],
				error: XcmError::AssetNotFound,
				weight: Weight::from_parts(300_000_000, 0),
			}
			.into(),
			pallet_relaychain_info::Event::CurrentBlockNumbers {
				parachain_block_number: 3,
				relaychain_block_number: 7,
			}
			.into(),
		]);
		let origin = MultiLocation::new(1, X1(Parachain(ACALA_PARA_ID)));
		let loc = MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0)));
		let asset: MultiAsset = (loc, 30 * UNITS).into();
		let hash = determine_hash(&origin, vec![asset]);
		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 1);
	});

	asset
}

fn claim_asset(asset: MultiAsset, recipient: [u8; 32]) {
	Acala::execute_with(|| {
		let recipient = MultiLocation::new(
			0,
			X1(Junction::AccountId32 {
				network: None,
				id: recipient,
			}),
		);
		let xcm_msg = Xcm(vec![
			ClaimAsset {
				assets: vec![asset.clone()].into(),
				ticket: Here.into(),
			},
			BuyExecution {
				fees: asset,
				weight_limit: Unlimited,
			},
			DepositAsset {
				assets: All.into(),
				beneficiary: recipient,
			},
		]);
		assert_ok!(hydradx_runtime::PolkadotXcm::send(
			hydradx_runtime::RuntimeOrigin::root(),
			Box::new(MultiLocation::new(1, X1(Parachain(HYDRA_PARA_ID))).into()),
			Box::new(VersionedXcm::from(xcm_msg))
		));
	});
}

#[test]
fn polkadot_xcm_execute_extrinsic_should_not_be_allowed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let message = VersionedXcm::V3(Xcm(vec![
			WithdrawAsset((Here, 410000000000u128).into()),
			BuyExecution {
				fees: (Here, 400000000000u128).into(),
				weight_limit: Unlimited,
			},
		]));

		assert_noop!(
			hydradx_runtime::PolkadotXcm::execute(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				Box::new(message),
				Weight::from_parts(400_000_000_000, 0)
			),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::Filtered
		);
	});
}
