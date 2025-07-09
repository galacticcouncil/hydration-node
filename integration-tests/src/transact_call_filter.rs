#![cfg(test)]
use crate::polkadot_test_net::*;

use frame_support::{assert_ok, dispatch::GetDispatchInfo};
use sp_runtime::codec::Encode;

use polkadot_xcm::v4::prelude::*;
use sp_std::sync::Arc;
use xcm_emulator::TestExt;

#[test]
fn allowed_transact_call_should_pass_filter() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// allowed by SafeCallFilter and the runtime call filter
		let call = pallet_balances::Call::<hydradx_runtime::Runtime>::transfer_allow_death {
			dest: BOB.into(),
			value: UNITS,
		};

		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::SovereignAccount,
				call: hydradx_runtime::RuntimeCall::Balances(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: cumulus_primitives_core::Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID
				)])),
			),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert_xcm_message_processing_passed();

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
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// filtered by SafeCallFilter
		let call = pallet_treasury::Call::<hydradx_runtime::Runtime>::spend_local {
			amount: UNITS,
			beneficiary: ALICE.into(),
		};

		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::Native,
				call: hydradx_runtime::RuntimeCall::Treasury(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: cumulus_primitives_core::Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID
				)])),
			),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert_xcm_message_processing_failed();
	});
}

#[test]
fn safe_call_filter_should_respect_runtime_call_filter() {
	// Arrange
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			parachain_reserve_account(),
			1_000 * UNITS,
		));
	});

	Acala::execute_with(|| {
		// transfer to the Omnipool is filtered by the runtime call filter
		let call = pallet_balances::Call::<hydradx_runtime::Runtime>::transfer_allow_death {
			dest: hydradx_runtime::Omnipool::protocol_account(),
			value: UNITS,
		};

		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
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
				require_weight_at_most: call.get_dispatch_info().weight,
				origin_kind: OriginKind::Native,
				call: hydradx_runtime::RuntimeCall::Balances(call).encode().into(),
			},
			ExpectTransactStatus(MaybeErrorCode::Success),
			RefundSurplus,
			DepositAsset {
				assets: All.into(),
				beneficiary: cumulus_primitives_core::Junction::AccountId32 {
					id: parachain_reserve_account().into(),
					network: None,
				}
				.into(),
			},
		]);

		// Act
		assert_ok!(hydradx_runtime::PolkadotXcm::send_xcm(
			Here,
			Location::new(
				1,
				cumulus_primitives_core::Junctions::X1(Arc::new([cumulus_primitives_core::Junction::Parachain(
					HYDRA_PARA_ID
				)])),
			),
			message
		));
	});

	Hydra::execute_with(|| {
		// Assert
		assert_xcm_message_processing_failed();
	});
}
