#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};

use polkadot_xcm::{latest::prelude::*, v3::WeightLimit, VersionedMultiAssets, VersionedXcm};

use cumulus_primitives_core::ParaId;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::storage::with_transaction;
use frame_support::traits::OnInitialize;
use frame_support::weights::Weight;
use hex_literal::hex;
use hydradx_runtime::AssetRegistry;
use hydradx_traits::registry::Mutate;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::currency::MultiCurrency;
use pretty_assertions::assert_eq;
use primitives::AccountId;
use sp_core::Decode;
use sp_core::H256;
use sp_runtime::traits::ConstU32;
use sp_runtime::traits::{AccountIdConversion, BlakeTwo256, Hash};
use sp_runtime::DispatchResult;
use sp_runtime::FixedU128;
use sp_runtime::TransactionOutcome;
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
fn polkadot_should_receive_asset_when_sent_from_hydra() {
	//Arrange
	PolkadotRelay::execute_with(|| {
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

	Hydra::execute_with(|| {
		let fee = hydradx_runtime::Tokens::free_balance(ACA, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "Fees is not sent to treasury");
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
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
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
				.into()
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
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
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
				.into()
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
				.into()
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
				relaychain_block_number: 8,
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
			1,
			hydradx_runtime::AssetLocation(MultiLocation::new(1, X2(Parachain(ACALA_PARA_ID), GeneralIndex(0))))
		));
	});

	claim_asset(asset.clone(), BOB);

	Hydra::execute_with(|| {
		assert_eq!(
			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
			1_029_939_717_395_149 //1000 * UNITS + 30 * UNITS - fee
		);

		let origin = MultiLocation::new(1, X1(Parachain(ACALA_PARA_ID)));
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

			assert_ok!(hydradx_runtime::Tokens::deposit(
				FOREIGN_ASSET,
				&AccountId::from(ALICE),
				3000 * UNITS
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	AssetHub::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			assert_ok!(hydradx_runtime::Tokens::deposit(
				FOREIGN_ASSET,
				&AccountId::from(ALICE),
				3000 * UNITS
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		let foreign_asset: MultiAssets = MultiAsset::from((
			MultiLocation {
				parents: 2,
				interior: Junctions::X1(GlobalConsensus(NetworkId::BitcoinCash)),
			},
			100 * UNITS,
		))
		.into();

		let bob_beneficiary: MultiLocation = Junction::AccountId32 { id: BOB, network: None }.into();

		let xcm =
			xcm_for_transfer_and_deposit_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

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
		let fee = hydradx_runtime::Tokens::free_balance(FOREIGN_ASSET, &hydradx_runtime::Treasury::account_id());
		assert!(fee > 0, "treasury should have received fees");
		//Check if the foreign asset from Assethub has been deposited successfully
		assert_eq!(
			hydradx_runtime::Currencies::free_balance(FOREIGN_ASSET, &AccountId::from(BOB)),
			100 * UNITS - fee
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

			assert_ok!(hydradx_runtime::Tokens::deposit(
				FOREIGN_ASSET,
				&AccountId::from(ALICE),
				3000 * UNITS
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});

	Acala::execute_with(|| {
		let _ = with_transaction(|| {
			register_foreign_asset();
			assert_ok!(hydradx_runtime::Tokens::deposit(
				FOREIGN_ASSET,
				&AccountId::from(ALICE),
				3000 * UNITS
			));

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		let foreign_asset: MultiAssets = MultiAsset::from((
			MultiLocation {
				parents: 2,
				interior: Junctions::X1(GlobalConsensus(NetworkId::BitcoinCash)),
			},
			100 * UNITS,
		))
		.into();

		let bob_beneficiary: MultiLocation = Junction::AccountId32 { id: BOB, network: None }.into();

		let xcm =
			xcm_for_transfer_and_deposit_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

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
		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::Fail { .. }
			))
		));
	});
}

#[test]
fn transfer_dot_reserve_from_asset_hub_to_hydra_should_work() {
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
			hydradx_runtime::AssetLocation(MultiLocation::new(1, Here))
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

		let foreign_asset: MultiAssets = MultiAsset::from((
			MultiLocation {
				parents: 1,
				interior: Junctions::Here,
			},
			100 * UNITS,
		))
		.into();

		let bob_beneficiary: MultiLocation = Junction::AccountId32 { id: BOB, network: None }.into();

		let xcm =
			xcm_for_transfer_and_deposit_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

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
		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::Success { .. }
			))
		));

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
fn transfer_dot_reserve_from_non_asset_hub_chain_to_hydra_should_not_work() {
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
			hydradx_runtime::AssetLocation(MultiLocation::new(1, Here))
		));
	});

	Acala::execute_with(|| {
		let _ = with_transaction(|| {
			register_dot();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});

		assert_ok!(hydradx_runtime::Tokens::deposit(
			DOT,
			&AccountId::from(ALICE),
			3000 * UNITS
		));

		let foreign_asset: MultiAssets = MultiAsset::from((
			MultiLocation {
				parents: 1,
				interior: Junctions::Here,
			},
			100 * UNITS,
		))
		.into();

		let bob_beneficiary: MultiLocation = Junction::AccountId32 { id: BOB, network: None }.into();

		let xcm =
			xcm_for_transfer_and_deposit_asset_to_hydra::<hydradx_runtime::RuntimeCall>(foreign_asset, bob_beneficiary);

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
		assert!(matches!(
			last_hydra_events(2).first(),
			Some(hydradx_runtime::RuntimeEvent::XcmpQueue(
				cumulus_pallet_xcmp_queue::Event::Fail { .. }
			))
		));
	});
}

fn xcm_for_transfer_and_deposit_asset_to_hydra<RC: Decode + GetDispatchInfo>(
	assets: MultiAssets,
	beneficiary: MultiLocation,
) -> VersionedXcm<RC> {
	use polkadot_runtime::xcm_config::BaseXcmWeight;
	use xcm_builder::FixedWeightBounds;
	use xcm_executor::traits::WeightBounds;

	type Weigher<RC> = FixedWeightBounds<BaseXcmWeight, RC, ConstU32<100>>;

	let dest = MultiLocation::new(1, Parachain(HYDRA_PARA_ID));

	let max_assets = assets.len() as u32 + 1;
	let context = X2(GlobalConsensus(NetworkId::Polkadot), Parachain(ACALA_PARA_ID));
	let fees = assets
		.get(0)
		.expect("should have at least 1 asset")
		.clone()
		.reanchored(&dest, context)
		.expect("should reanchor");
	let weight_limit = {
		let fees = fees.clone();
		let mut remote_message = Xcm(vec![
			ReserveAssetDeposited::<RC>(assets.clone()),
			ClearOrigin,
			BuyExecution {
				fees,
				weight_limit: Limited(Weight::zero()),
			},
			DepositAsset {
				assets: Wild(AllCounted(max_assets)),
				beneficiary,
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
		TransferReserveAsset { assets, dest, xcm },
	]);
	VersionedXcm::V3(message)
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
			X1(GlobalConsensus(NetworkId::BitcoinCash))
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
		Some(hydradx_runtime::AssetLocation(MultiLocation::new(1, Here))),
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
				relaychain_block_number: 8,
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
fn polkadot_xcm_execute_extrinsic_should_be_allowed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let message = VersionedXcm::V3(Xcm(vec![
			WithdrawAsset((Here, 410000000000u128).into()),
			BuyExecution {
				fees: (Here, 400000000000u128).into(),
				weight_limit: Unlimited,
			},
		]));

		assert_ok!(hydradx_runtime::PolkadotXcm::execute(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			Box::new(message),
			Weight::from_parts(400_000_000_000, 0)
		),);
	});
}
