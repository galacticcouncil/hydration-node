#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_ok, dispatch::GetDispatchInfo};
use sp_runtime::codec::Encode;

use polkadot_xcm::latest::prelude::*;
use xcm_emulator::TestExt;

#[test]
fn allowed_transact_call_should_pass_filter() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// allowed by SafeCallFilter and the runtime call filter
		let call = pallet_balances::Call::<hydradx_runtime::Runtime>::transfer {
			dest: BOB.into(),
			value: UNITS,
		};
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::SovereignAccount,
				call: hydradx_runtime::RuntimeCall::Balances(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			MultiLocation::new(1, X1(Parachain(HYDRA_PARA_ID))),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert!(hydradx_runtime::System::events().iter().any(|r| matches!(
			r.event,
			hydradx_runtime::RuntimeEvent::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Success { .. })
		)));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(AccountId::from(BOB)),
			BOB_INITIAL_NATIVE_BALANCE + UNITS
		);
	});
}

#[test]
fn blocked_transact_calls_should_not_pass_filter() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// filtered by SafeCallFilter
		let call = pallet_tips::Call::<hydradx_runtime::Runtime>::report_awesome {
			reason: vec![0, 10],
			who: BOB.into(),
		};
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::Native,
				call: hydradx_runtime::RuntimeCall::Tips(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			MultiLocation::new(1, X1(Parachain(HYDRA_PARA_ID))),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert!(hydradx_runtime::System::events().iter().any(|r| matches!(
			r.event,
			hydradx_runtime::RuntimeEvent::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Fail {
				error: cumulus_primitives_core::XcmError::NoPermission,
				..
			})
		)));
	});
}

#[test]
fn safe_call_filter_should_respect_runtime_call_filter() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// transfer to the Omnipool is filtered by the runtime call filter
		let call = pallet_balances::Call::<hydradx_runtime::Runtime>::transfer {
			dest: hydradx_runtime::Omnipool::protocol_account(),
			value: UNITS,
		};
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::Native,
				call: hydradx_runtime::RuntimeCall::Balances(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			MultiLocation::new(1, X1(Parachain(HYDRA_PARA_ID))),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert!(hydradx_runtime::System::events().iter().any(|r| matches!(
			r.event,
			hydradx_runtime::RuntimeEvent::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Fail {
				error: cumulus_primitives_core::XcmError::NoPermission,
				..
			})
		)));
	});
}
