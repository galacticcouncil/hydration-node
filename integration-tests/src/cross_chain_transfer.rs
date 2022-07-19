#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};

use polkadot_xcm::latest::prelude::*;

use cumulus_primitives_core::ParaId;
use orml_traits::currency::MultiCurrency;
use sp_runtime::traits::AccountIdConversion;
use xcm_emulator::TestExt;

#[test]
#[ignore]
//TODO: This test is ignored as cross chain reserve transfer disabled on polkadot v0.9.16 within pallet_xcm (via pallet config `XcmReserveTransferFilter`).
//From polkadot v0.9.19 this filter allows such transfer (https://github.com/paritytech/polkadot/blob/f00a2772497aadddf75b8b4b475843ea0d910c48/runtime/polkadot/src/xcm_config.rs#L185), so we should remove the #[ignore] tag once we upgrade to that version
fn hydra_should_receive_asset_when_transferred_from_polkadot_relay_chain() {
	//Arrange
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::Origin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));
	});
	PolkadotRelay::execute_with(|| {
		//Act
		assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
			polkadot_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(2000).into().into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((Here, 300 * UNITS).into()),
			0,
		));

		//Assert
		assert_eq!(
			polkadot_runtime::Balances::free_balance(&ParaId::from(2000i32).into_account()),
			310 * UNITS
		);
	});

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			12780 * UNITS / 10
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id()),
			22 * UNITS
		);
	});
}

#[test]
fn polkadot_should_receive_asset_when_sent_from_hydra() {
	//Arrange
	PolkadotRelay::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(BOB)),
			0
		);
	});

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::Origin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));

		//Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::Origin::signed(ALICE.into()),
			1,
			3 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X1(Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any,
					})
				)
				.into()
			),
			4_600_000_000
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(ALICE)),
			200 * UNITS - 3 * UNITS
		);
	});

	PolkadotRelay::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(BOB)),
			2999680000000 // 3 * HDX - fee
		);
	});
}

#[test]
#[ignore]
//TODO: it seems that the balance does not change on hydra. To be investigated
fn hydra_should_receive_asset_when_transferred_from_basilisk() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::Origin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(3000), GeneralIndex(0))))
		));
	});

	Basilisk::execute_with(|| {
		//Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::Origin::signed(ALICE.into()),
			0,
			30 * UNITS,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(2000),
						Junction::AccountId32 {
							id: BOB,
							network: NetworkId::Any,
						}
					)
				)
				.into()
			),
			399_600_000_000
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			200 * UNITS - 30 * UNITS
		);
	});

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			10080 * UNITS / 10
		);
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id()),
			22 * UNITS // fees should go to treasury
		);
	});
}

#[test]
fn transfer_from_basilisk_should_fail_when_transferring_insufficient_amount() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			hydradx_runtime::Origin::root(),
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(3000), GeneralIndex(0))))
		));
	});

	Basilisk::execute_with(|| {
		assert_noop!(
			hydradx_runtime::XTokens::transfer(
				hydradx_runtime::Origin::signed(ALICE.into()),
				0,
				1_000_000,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Junction::Parachain(2000),
							Junction::AccountId32 {
								id: BOB,
								network: NetworkId::Any,
							}
						)
					)
					.into()
				),
				399_600_000_000
			),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::XcmExecutionFailed
		);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			200000000000000
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
