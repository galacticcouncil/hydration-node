#![cfg(test)]
use crate::erc20::bind_erc20;
use crate::erc20::deploy_token_contract;
use crate::polkadot_test_net::Rococo;
use crate::polkadot_test_net::*;
use xcm_emulator::ConvertLocation;
use xcm_executor::traits::TransferType;

use frame_support::{assert_noop, assert_ok};

use polkadot_xcm::{v4::prelude::*, VersionedAssetId, VersionedAssets, VersionedXcm};

use cumulus_primitives_core::ParaId;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::storage::with_transaction;
use frame_support::traits::OnInitialize;
use frame_support::weights::Weight;
use hydradx_runtime::{AssetRegistry, LocationToAccountId};
use hydradx_traits::{registry::Mutate, AssetKind, Create};
use orml_traits::currency::MultiCurrency;
use polkadot_xcm::opaque::v3::{
	Junction,
	Junctions::{X1, X2},
	MultiLocation, NetworkId,
};
use pretty_assertions::assert_eq;
use primitives::AccountId;
use sp_core::{Decode, H256};
use sp_runtime::traits::{AccountIdConversion, BlakeTwo256, ConstU32, Hash};
use sp_runtime::{DispatchResult, FixedU128, TransactionOutcome};
use sp_std::sync::Arc;
use xcm_emulator::TestExt;

// Determine the hash for assets expected to be have been trapped.
pub fn determine_hash(origin: &MultiLocation, assets: Vec<Asset>) -> H256 {
	let versioned = VersionedAssets::from(Assets::from(assets));
	BlakeTwo256::hash_of(&(origin, &versioned))
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_rococo_relay_chain() {
	//Arrange
	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));
	});

	Rococo::execute_with(|| {
		//Act
		assert_ok!(rococo_runtime::XcmPallet::limited_reserve_transfer_assets(
			rococo_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(Parachain(HYDRA_PARA_ID).into_versioned()),
			Box::new(Junction::AccountId32 { id: BOB, network: None }.into_versioned()),
			Box::new((Here, 300 * UNITS).into()),
			0,
			WeightLimit::Unlimited,
		));

		//Assert
		assert_eq!(
			rococo_runtime::Balances::free_balance(AccountIdConversion::<AccountId>::into_account_truncating(
				&ParaId::from(HYDRA_PARA_ID)
			)),
			310 * UNITS
		);
	});

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "Fees is not sent to treasury");

		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS - fee
		);
	});
}

#[test]
fn rococo_should_receive_asset_when_sent_from_hydra() {
	//Arrange
	Rococo::execute_with(|| {
		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)), 0);
	});

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			1,
			hydradx_runtime::AssetLocation(MultiLocation::parent())
		));

		//Act
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			1,
			3 * UNITS,
			Box::new(MultiLocation::new(1, X1(Junction::AccountId32 { id: BOB, network: None })).into_versioned()),
			WeightLimit::Unlimited,
		));

		//Assert
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(ALICE)),
			200 * UNITS - 3 * UNITS
		);
	});

	Rococo::execute_with(|| {
		assert!(hydradx_runtime::Balances::free_balance(AccountId::from(BOB)) < 3 * UNITS);
	});
}

#[test]
fn xtoken_transfer_of_erc20_should_fail() {
	//Arrange
	Hydra::execute_with(|| {
		// Arrange
		let asset = bind_erc20(deploy_token_contract());

		//Act & Assert
		assert_noop!(
			hydradx_runtime::XTokens::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				asset,
				3 * UNITS,
				Box::new(MultiLocation::new(1, X1(Junction::AccountId32 { id: BOB, network: None })).into_versioned()),
				WeightLimit::Unlimited,
			),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::AssetHasNoReserve
		);
	});
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_acala() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
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
				.into_versioned()
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
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "Fee is not sent to treasury");
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(BOB)),
			30 * UNITS - fee
		);
	});
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_acala_to_eth_address() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
		));
	});

	let amount = 30 * UNITS;
	Acala::execute_with(|| {
		//We send to ethereum address with Account20
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountKey20 {
							network: None,
							key: evm_address().into(),
						}
					)
				)
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - amount
		);
	});

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "fee should be greater than 0");
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(evm_account())),
			amount - fee
		);
	});
}

#[test]
fn hydra_should_receive_asset_when_transferred_from_acala_to_same_address_represented_as_both_account32_and_20() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			ACA,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
		));
	});

	let amount = 30 * UNITS;
	Acala::execute_with(|| {
		//We send to ethereum address with Account20
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountKey20 {
							network: None,
							key: evm_address().into(),
						}
					)
				)
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		//We send it again to the same address, but to normal Account32
		assert_ok!(hydradx_runtime::XTokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			0,
			amount,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Junction::Parachain(HYDRA_PARA_ID),
						Junction::AccountId32 {
							id: evm_account().into(),
							network: None
						}
					)
				)
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));

		// Assert
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 2 * amount
		);
	});

	Hydra::execute_with(|| {
		let fee_2x = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fee_2x > 0, "fee should be greater than 0");
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(ACA, &AccountId::from(evm_account())),
			2 * amount - fee_2x
		);
	});
}

#[test]
fn transfer_from_acala_should_fail_when_transferring_insufficient_amount() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
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
					.into_versioned()
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
			DAI, // we pretend that the incoming tokens are DAI
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
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
				.into_versioned()
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
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	Hydra::execute_with(|| {
		assert_xcm_message_processing_failed();
		let origin = MultiLocation::new(1, X1(Junction::Parachain(ACALA_PARA_ID)));
		let asset: Asset = Asset {
			id: cumulus_primitives_core::AssetId(Location::new(
				1,
				cumulus_primitives_core::Junctions::X2(Arc::new([
					cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				])),
			)),
			fun: Fungible(30 * UNITS),
		};
		let hash = determine_hash(&origin, vec![asset.clone()]);

		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 1);

		expect_hydra_events(vec![hydradx_runtime::RuntimeEvent::PolkadotXcm(
			pallet_xcm::Event::AssetsTrapped {
				hash,
				origin: origin.try_into().unwrap(),
				assets: vec![asset].into(),
			},
		)]);
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
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(
				1,
				X2(Junction::Parachain(ACALA_PARA_ID), Junction::GeneralIndex(0))
			))
		));
	});

	let bob_loc = Location::new(
		0,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
			id: BOB,
			network: None,
		}])),
	);

	claim_asset(asset.clone(), bob_loc);

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(LRNA, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "treasury should have received fees");

		let bob_new_lrna_balance = hydradx_runtime::Tokens::free_balance(LRNA, &AccountId::from(BOB));
		assert!(
			bob_new_lrna_balance > BOB_INITIAL_LRNA_BALANCE,
			"Bob should have received the claimed trapped asset"
		);

		let origin = MultiLocation::new(1, X1(Junction::Parachain(ACALA_PARA_ID)));
		let hash = determine_hash(&origin, vec![asset]);
		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 0);
	});
}

#[test]
fn transfer_foreign_asset_from_asset_hub_to_hydra_should_work() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();

			add_currency_price(FOREIGN_ASSET, FixedU128::from(1));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	AssetHub::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			FOREIGN_ASSET,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		let foreign_asset: Asset = Asset {
			id: cumulus_primitives_core::AssetId(Location::new(
				2,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GlobalConsensus(
					cumulus_primitives_core::NetworkId::BitcoinCash,
				)])),
			)),
			fun: Fungible(100 * UNITS),
		};

		let bob_beneficiary = Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
				id: BOB,
				network: None,
			}])),
		);

		let xcm =
			xcm_for_deposit_reserve_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	//Assert
	Hydra::execute_with(|| {
		assert_xcm_message_processing_passed();

		let fee = hydradx_runtime::Tokens::free_balance(FOREIGN_ASSET, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "treasury should have received fees");

		//Check if the foreign asset from Assethub has been deposited successfully
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(FOREIGN_ASSET, &AccountId::from(BOB)),
			100 * UNITS
		);
	});
}

#[test]
fn transfer_foreign_asset_from_acala_to_hydra_should_not_work() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();

			add_currency_price(FOREIGN_ASSET, FixedU128::from(1));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	Acala::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			FOREIGN_ASSET,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		let foreign_asset: Asset = Asset {
			id: cumulus_primitives_core::AssetId(Location::new(
				2,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::GlobalConsensus(
					cumulus_primitives_core::NetworkId::BitcoinCash,
				)])),
			)),
			fun: Fungible(100 * UNITS),
		};

		let bob_beneficiary = Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
				id: BOB,
				network: None,
			}])),
		);

		let xcm =
			xcm_for_deposit_reserve_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	//Assert
	Hydra::execute_with(|| {
		assert_xcm_message_processing_failed();
	});
}

#[test]
fn transfer_dot_from_asset_hub_to_hydra_should_work() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		add_currency_price(DOT, FixedU128::from(1));

		assert_ok!(hydradx_runtime::Tokens::deposit(
			DOT,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			DOT,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, polkadot_xcm::opaque::v3::Junctions::Here))
		));
	});

	AssetHub::execute_with(|| {
		let _ = with_transaction(|| {
			register_dot();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			DOT,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		let dot: Asset = Asset {
			id: cumulus_primitives_core::AssetId(Location::new(1, cumulus_primitives_core::Junctions::Here)),
			fun: Fungible(100 * UNITS),
		};

		let bob_beneficiary = Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
				id: BOB,
				network: None,
			}])),
		);

		let xcm =
			xcm_transfer_reserve_asset_and_deposit_asset_to_hydra::<hydradx_runtime::RuntimeCall>(dot, bob_beneficiary);

		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	//Assert
	Hydra::execute_with(|| {
		assert_xcm_message_processing_passed();

		let fee = hydradx_runtime::Tokens::free_balance(DOT, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "treasury should have received fees");
		//Check if the foreign asset from Assethub has been deposited successfully
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(DOT, &AccountId::from(BOB)),
			100 * UNITS - fee
		);
	});
}

#[test]
fn transfer_dot_from_hydra_to_asset_hub_should_work() {
	let init_hydra_para_dot_balance_on_ah = 4000 * UNITS;
	let hydra_at_ah = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)])),
	);

	let hydra_parachain_account_at_ah = LocationToAccountId::convert_location(&hydra_at_ah).unwrap();

	let transfer_amount = 3 * UNITS;

	AssetHub::execute_with(|| {
		let _ = with_transaction(|| {
			register_dot();
			add_currency_price(DOT, FixedU128::from(1));
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			DOT,
			&hydra_parachain_account_at_ah,
			init_hydra_para_dot_balance_on_ah
		));
	});

	//Arrange
	Hydra::execute_with(|| {
		let dot_multiloc = MultiLocation::new(1, polkadot_xcm::opaque::v3::Junctions::Here);

		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
			DOT,
			hydradx_runtime::AssetLocation(dot_multiloc)
		));

		let dot_loc = Location::new(1, cumulus_primitives_core::Junctions::Here);

		let dot: Asset = Asset {
			id: cumulus_primitives_core::AssetId(dot_loc.clone()),
			fun: Fungible(transfer_amount),
		};

		let bob_beneficiary = Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
				id: BOB,
				network: None,
			}])),
		);

		let deposit_xcm = Xcm(vec![DepositAsset {
			assets: Wild(WildAsset::AllCounted(1)),
			beneficiary: bob_beneficiary.clone(),
		}]);

		//Act
		assert_ok!(hydradx_runtime::PolkadotXcm::transfer_assets_using_type_and_then(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(MultiLocation::new(1, X1(Junction::Parachain(ASSET_HUB_PARA_ID),)).into_versioned()),
			Box::new(dot.into()),
			Box::new(TransferType::DestinationReserve),
			Box::new(VersionedAssetId::V4(cumulus_primitives_core::AssetId(dot_loc))),
			Box::new(TransferType::DestinationReserve),
			Box::new(VersionedXcm::from(deposit_xcm)),
			WeightLimit::Unlimited,
		));

		assert_eq!(
			hydradx_runtime::Tokens::free_balance(DOT, &AccountId::from(ALICE)),
			ALICE_INITIAL_DOT_BALANCE - transfer_amount
		);
	});

	//Needed to process horizontal xcm messages
	Rococo::execute_with(|| {});

	AssetHub::execute_with(|| {
		assert_xcm_message_processing_passed();

		//We check if the hydra parachain account balance is reduced on AH, meaning AH is responsible for reserve tracking
		let hydra_sovereign_account_dot_balance =
			hydradx_runtime::Currencies::free_balance(DOT, &hydra_parachain_account_at_ah);
		assert_eq!(
			hydra_sovereign_account_dot_balance,
			init_hydra_para_dot_balance_on_ah - transfer_amount
		);

		let fee = hydradx_runtime::Currencies::free_balance(DOT, &hydradx_runtime::Treasury::account_id());

		assert_eq!(
			hydradx_runtime::Currencies::free_balance(DOT, &AccountId::from(BOB)),
			transfer_amount - fee
		);
	});
}

#[test]
fn transfer_dot_reserve_from_non_asset_hub_chain_to_hydra_should_not_work() {
	//Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			assert_ok!(hydradx_runtime::AssetRegistry::set_location(
				DOT,
				hydradx_runtime::AssetLocation(MultiLocation::new(1, polkadot_xcm::opaque::v3::Junctions::Here))
			));

			add_currency_price(FOREIGN_ASSET, FixedU128::from(1));
			add_currency_price(DOT, FixedU128::from(1));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	Acala::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			register_dot();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			FOREIGN_ASSET,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		assert_ok!(hydradx_runtime::Tokens::deposit(
			DOT,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		let dot: Asset = Asset {
			id: cumulus_primitives_core::AssetId(Location::new(1, cumulus_primitives_core::Junctions::Here)),
			fun: Fungible(100 * UNITS),
		};

		let bob_beneficiary = Location::new(
			0,
			cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::AccountId32 {
				id: BOB,
				network: None,
			}])),
		);

		let xcm = xcm_for_deposit_reserve_asset_to_hydra::<hydradx_runtime::RuntimeCall>(dot, bob_beneficiary);

		//Act
		let res = hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(xcm),
			Weight::from_parts(399_600_000_000, 0),
		);
		assert_ok!(res);

		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::XcmpMessageSent { .. }
			))
		));
	});

	//Assert
	Hydra::execute_with(|| {
		assert_xcm_message_processing_failed();
	});
}

fn xcm_for_deposit_reserve_asset_to_hydra<RC: Decode + GetDispatchInfo>(
	assets: Asset,
	beneficiary: Location,
) -> VersionedXcm<RC> {
	use rococo_runtime::xcm_config::BaseXcmWeight;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let dest = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)])),
	);

	let context = cumulus_primitives_core::Junctions::X2(Arc::new([
		cumulus_primitives_core::Junction::GlobalConsensus(cumulus_primitives_core::NetworkId::Polkadot),
		cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
	]));

	let fee_asset = assets.clone().reanchored(&dest, &context).expect("should reanchor");
	let weight_limit = {
		let fees = fee_asset.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(vec![assets.clone()].into()),
			ClearOrigin,
			BuyExecution {
				fees,
				weight_limit: Limited(Weight::zero()),
			},
			DepositAsset {
				assets: Definite(assets.clone().into()),
				beneficiary: beneficiary.clone(),
			},
		]);
		// use local weight for remote message and hope for the best.
		let remote_weight = Weigher::weight(&mut remote_message).expect("weighing should not fail");
		Limited(remote_weight)
	};

	// executed on local (AssetHub)
	let message = Xcm(vec![
		WithdrawAsset(vec![fee_asset.clone(), assets.clone()].into()),
		DepositReserveAsset {
			assets: Definite(vec![fee_asset.clone(), assets.clone()].into()),
			dest,
			xcm: Xcm(vec![
				// executed on remote (on hydra)
				BuyExecution {
					fees: fee_asset,
					weight_limit,
				},
				DepositAsset {
					assets: Definite(assets.into()),
					beneficiary,
				},
			]),
		},
	]);

	VersionedXcm::from(message)
}

fn register_foreign_asset() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(FOREIGN_ASSET),
		Some(b"FORA".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			2,
			X1(Junction::GlobalConsensus(NetworkId::BitcoinCash))
		))),
		None,
	));
}

fn register_dot() {
	assert_ok!(AssetRegistry::register_sufficient_asset(
		Some(DOT),
		Some(b"DOT".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1_000_000,
		None,
		None,
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(
			1,
			polkadot_xcm::opaque::v3::Junctions::Here
		))),
		None,
	));
}

fn add_currency_price(asset_id: u32, price: FixedU128) {
	assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
		hydradx_runtime::RuntimeOrigin::root(),
		asset_id,
		price,
	));

	// make sure the price is propagated
	hydradx_runtime::MultiTransactionPayment::on_initialize(hydradx_runtime::System::block_number());
}

fn trap_asset() -> Asset {
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
				.into_versioned()
			),
			WeightLimit::Limited(Weight::from_parts(399_600_000_000, 0))
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
			ALICE_INITIAL_NATIVE_BALANCE - 30 * UNITS
		);
	});

	let asset: Asset = Asset {
		id: cumulus_primitives_core::AssetId(Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
		)),
		fun: Fungible(30 * UNITS),
	};

	Hydra::execute_with(|| {
		assert_xcm_message_processing_failed();
		let origin = MultiLocation::new(1, X1(Junction::Parachain(ACALA_PARA_ID)));
		let hash = determine_hash(&origin, vec![asset.clone()]);

		assert_eq!(hydradx_runtime::PolkadotXcm::asset_trap(hash), 1);
	});

	asset
}

fn claim_asset(asset: Asset, recipient: Location) {
	Acala::execute_with(|| {
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
			Box::new(MultiLocation::new(1, X1(Junction::Parachain(HYDRA_PARA_ID))).into_versioned()),
			Box::new(VersionedXcm::from(xcm_msg))
		));
	});
}

#[test]
fn rococo_xcm_execute_extrinsic_should_be_allowed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
		);
		let asset_to_withdraw: Asset = Asset {
			id: cumulus_primitives_core::AssetId(hdx_loc.clone()),
			fun: Fungible(410000000000u128),
		};

		let message = Xcm(vec![
			WithdrawAsset(asset_to_withdraw.into()),
			BuyExecution {
				fees: (Here, 400000000000u128).into(),
				weight_limit: Unlimited,
			},
		]);

		assert_ok!(hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(VersionedXcm::from(message)),
			Weight::from_parts(400_000_000_000, 0)
		),);
	});
}

fn xcm_transfer_reserve_asset_and_deposit_asset_to_hydra<RC: Decode + GetDispatchInfo>(
	assets: Asset,
	beneficiary: Location,
) -> VersionedXcm<RC> {
	use rococo_runtime::xcm_config::BaseXcmWeight;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let dest = Location::new(
		1,
		cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)])),
	);

	let max_assets = 1 + 1;

	let context = cumulus_primitives_core::Junctions::X2(Arc::new([
		cumulus_primitives_core::Junction::GlobalConsensus(cumulus_primitives_core::NetworkId::Polkadot),
		cumulus_primitives_core::Junction::Parachain(ASSET_HUB_PARA_ID),
	]));

	let fee_asset = assets.clone().reanchored(&dest, &context).expect("should reanchor");
	let fees = fee_asset.clone();

	let weight_limit = {
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets.clone().into()),
			ClearOrigin,
			BuyExecution {
				fees: fees.clone(),
				weight_limit: Limited(Weight::zero()),
			},
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary: beneficiary.clone(),
			},
		]);
		// use local weight for remote message and hope for the best.
		let remote_weight = Weigher::weight(&mut remote_message).expect("weighing should not fail");
		Limited(remote_weight)
	};
	// executed on remote (on hydra)
	let xcm = Xcm(vec![
		//ReserveAssetDeposited(assets.clone()),
		BuyExecution { fees, weight_limit },
		DepositAsset {
			assets: Wild(AllCounted(max_assets)),
			beneficiary,
		},
	]);
	// executed on local (AssetHub)
	let message = Xcm(vec![
		SetFeesMode { jit_withdraw: true },
		TransferReserveAsset {
			assets: assets.into(),
			dest,
			xcm,
		},
	]);
	VersionedXcm::from(message)
}
