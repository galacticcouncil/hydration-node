#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_ok, weights::Weight};
use sp_runtime::codec::Encode;

use hydradx_adapters::xcm_account_derivation::HashedDescriptionDescribeFamilyAllTerminal;
use orml_traits::MultiCurrency;
use polkadot_xcm::latest::prelude::*;
use xcm_emulator::TestExt;
use xcm_emulator::ConvertLocation;
use frame_support::dispatch::GetDispatchInfo;

#[test]
fn other_chain_remote_account_should_work_on_hydra() {
	// Arrange
	TestNet::reset();

	let xcm_interior_at_acala = X1(Junction::AccountId32 {
		network: None,
		id: evm_account().into(),
	});

	let xcm_origin_at_hydra = MultiLocation {
		parents: 1,
		interior: X2(
			Junction::Parachain(ACALA_PARA_ID),
			Junction::AccountId32 {
				network: None,
				id: evm_account().into(),
			},
		),
	};

	let acala_account_id_at_hydra: AccountId =
		HashedDescriptionDescribeFamilyAllTerminal::convert_location(&xcm_origin_at_hydra).unwrap();

	Hydra::execute_with(|| {
		init_omnipool();

		assert_ok!(hydradx_runtime::Balances::transfer(
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

		let message = Xcm(vec![
			WithdrawAsset(
				(
					MultiLocation {
						parents: 1,
						interior: X2(Parachain(HYDRA_PARA_ID), GeneralIndex(0)),
					},
					900 * UNITS,
				)
					.into(),
			),
			BuyExecution {
				fees: (
					MultiLocation {
						parents: 1,
						interior: X2(Parachain(HYDRA_PARA_ID), GeneralIndex(0)),
					},
					800 * UNITS,
				)
					.into(),
				weight_limit: Unlimited,
			},
			Transact {
				require_weight_at_most:  omni_sell.get_dispatch_info().weight,
				origin_kind: OriginKind::SovereignAccount,
				call: omni_sell.encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: Junction::AccountId32 {
					id: acala_account_id_at_hydra.clone().into(),
					network: None,
				}
				.into(),
			},
		]);

		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			xcm_interior_at_acala,
			MultiLocation::new(1, X1(Parachain(HYDRA_PARA_ID))),
			message
		));
	});

	// Assert
	Hydra::execute_with(|| {
		assert!(hydradx_runtime::System::events().iter().any(|r| matches!(
			r.event,
			hydradx_runtime::RuntimeEvent::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Success { .. })
		)));

		let dai_balance = hydradx_runtime::Currencies::free_balance(DAI, &AccountId::from(acala_account_id_at_hydra));
		assert!(
			dai_balance > 0,
			"Omnipool sell swap failed as the user did not receive any DAI"
		);
	});
}
