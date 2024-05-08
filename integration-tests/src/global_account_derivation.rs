#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::assert_ok;
use sp_runtime::codec::Encode;

use frame_support::dispatch::GetDispatchInfo;
use orml_traits::MultiCurrency;
use polkadot_xcm::v4::prelude::*;
use sp_std::sync::Arc;
use xcm_builder::DescribeAllTerminal;
use xcm_builder::DescribeFamily;
use xcm_builder::HashedDescription;
use xcm_emulator::ConvertLocation;
use xcm_emulator::TestExt;

#[test]
fn other_chain_remote_account_should_work_on_hydra() {
	// Arrange
	TestNet::reset();

	let xcm_interior_at_acala = cumulus_primitives_core::Junctions::X1(Arc::new(
		vec![cumulus_primitives_core::Junction::AccountId32 {
			network: None,
			id: evm_account().into(),
		}]
		.try_into()
		.unwrap(),
	));

	let xcm_origin_at_hydra = Location {
		parents: 1,
		interior: cumulus_primitives_core::Junctions::X2(Arc::new(
			vec![
				cumulus_primitives_core::Junction::Parachain(ACALA_PARA_ID),
				cumulus_primitives_core::Junction::AccountId32 {
					network: None,
					id: evm_account().into(),
				},
			]
			.try_into()
			.unwrap(),
		)),
	};

	let acala_account_id_at_hydra: AccountId =
		HashedDescription::<AccountId, DescribeFamily<DescribeAllTerminal>>::convert_location(&xcm_origin_at_hydra)
			.unwrap();

	Hydra::execute_with(|| {
		init_omnipool();

		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			acala_account_id_at_hydra.clone(),
			1_000 * UNITS,
		));

		assert_eq!(
			hydradx_runtime::Currencies::free_balance(DAI, &AccountId::from(acala_account_id_at_hydra.clone())),
			0
		);
	});

	// Act
	Acala::execute_with(|| {
		let omni_sell =
			hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
				asset_in: HDX,
				asset_out: DAI,
				amount: UNITS,
				min_buy_amount: 0,
			});

		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new(
				vec![
					cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
					cumulus_primitives_core::Junction::GeneralIndex(0),
				]
				.try_into()
				.unwrap(),
			)),
		);
		let asset_to_withdraw: Asset = Asset {
			id: cumulus_primitives_core::AssetId(hdx_loc.clone()),
			fun: Fungible(900 * UNITS),
		};
		let asset_for_buy_execution: Asset = Asset {
			id: cumulus_primitives_core::AssetId(hdx_loc),
			fun: Fungible(800 * UNITS),
		};

		let message = Xcm(vec![
			WithdrawAsset(asset_to_withdraw.into()),
			BuyExecution {
				fees: asset_for_buy_execution,
				weight_limit: Unlimited,
			},
			Transact {
				require_weight_at_most: omni_sell.get_dispatch_info().weight,
				origin_kind: OriginKind::SovereignAccount,
				call: omni_sell.encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: cumulus_primitives_core::Junction::AccountId32 {
					id: acala_account_id_at_hydra.clone().into(),
					network: None,
				}
				.into(),
			},
		]);

		let dest_hydradx = Location::new(
			1,
			cumulus_primitives_core::Junctions::X1(Arc::new(
				vec![cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID)]
					.try_into()
					.unwrap(),
			)),
		);

		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			xcm_interior_at_acala,
			dest_hydradx,
			message
		));
	});

	// Assert
	Hydra::execute_with(|| {
		assert_xcm_message_processing_passed();

		let dai_balance = hydradx_runtime::Currencies::free_balance(DAI, &AccountId::from(acala_account_id_at_hydra));
		assert!(
			dai_balance > 0,
			"Omnipool sell swap failed as the user did not receive any DAI"
		);
	});
}
