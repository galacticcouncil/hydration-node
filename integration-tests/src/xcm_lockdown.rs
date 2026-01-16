#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::{CircuitBreaker, RuntimeCall, TokenGateway};
use polkadot_xcm::v4::prelude::*;
use polkadot_xcm::VersionedXcm;
use xcm_emulator::TestExt;
use frame_support::weights::Weight;
use sp_runtime::traits::Dispatchable;
use ismp::host::StateMachine;

#[test]
fn polkadot_xcm_execute_should_fail_when_lockdown_active_and_message_is_egress() {
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		let message = Xcm(vec![
			WithdrawAsset((Here, 1000).into()),
			DepositReserveAsset {
				assets: All.into(),
				dest: Location::parent(),
				xcm: Xcm(vec![]),
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		assert_noop!(
			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered
		);
	});
}

#[test]
fn polkadot_xcm_send_should_fail_when_lockdown_active_and_message_is_egress() {
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		let message = Xcm(vec![
			DepositReserveAsset {
				assets: All.into(),
				dest: Location::parent(),
				xcm: Xcm(vec![]),
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::send {
			dest: Box::new(Location::parent().into()),
			message: Box::new(VersionedXcm::from(message)),
		});

		// Act & Assert
		// CallFilter should block it
		assert_noop!(
			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered
		);
	});
}

#[test]
fn polkadot_xcm_execute_should_succeed_when_lockdown_active_and_message_is_not_egress() {
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			0, // Native asset
			1_000_000_000_000i128,
		));

		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		// WithdrawAsset + BuyExecution is not egress
		let message = Xcm(vec![
			WithdrawAsset((Here, 1000).into()),
			BuyExecution {
				fees: (Here, 1000).into(),
				weight_limit: Unlimited,
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		// We expect successful dispatch (it might still fail inside the executor if ALICE has no balance, but not because of CallFilter)
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert_ne!(res, Err(frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered.into()));
	});
}

#[test]
fn root_origin_should_bypass_call_filter_lockdown() {
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		let message = Xcm(vec![
			DepositReserveAsset {
				assets: All.into(),
				dest: Location::parent(),
				xcm: Xcm(vec![]),
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::send {
			dest: Box::new(Location::parent().into()),
			message: Box::new(VersionedXcm::from(message)),
		});

		// Act & Assert
		// Root origin bypasses CallFilter. 
		// It might fail in dispatch because of missing hrmp channel or whatever, but not CallFiltered.
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::root());
		assert_ne!(res, Err(frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered.into()));
	});
}

#[test]
fn xcm_reserve_transfer_assets_blocked_during_lockdown() {
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(sp_std::sync::Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
		);
		let asset: Asset = Asset {
			id: cumulus_primitives_core::AssetId(hdx_loc),
			fun: Fungible(1000),
		};

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::limited_reserve_transfer_assets {
			dest: Box::new(Location::new(1, [Parachain(ASSET_HUB_PARA_ID)]).into()),
			beneficiary: Box::new(Location::new(0, [AccountId32 { network: None, id: ALICE }]).into()),
			assets: Box::new(vec![asset].into()),
			fee_asset_item: 0,
			weight_limit: Unlimited,
		});

		// Act & Assert
		// XcmReserveTransferFilter should block it
		assert_noop!(
			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::Filtered
		);
	});
}

#[test]
fn xcm_fees_do_not_trigger_lockdown() {
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			0, // Native asset
			1_000_000_000_000i128,
		));
		
		// Set a limit but no lockdown yet
		assert_ok!(hydradx_runtime::CircuitBreaker::set_global_withdraw_limit(hydradx_runtime::RuntimeOrigin::root(), 1_000_000_000_000));

		let hdx_loc = Location::new(
			1,
			cumulus_primitives_core::Junctions::X2(sp_std::sync::Arc::new([
				cumulus_primitives_core::Junction::Parachain(HYDRA_PARA_ID),
				cumulus_primitives_core::Junction::GeneralIndex(0),
			])),
		);
		let asset_to_withdraw: Asset = Asset {
			id: cumulus_primitives_core::AssetId(hdx_loc.clone()),
			fun: Fungible(900_000_000_000u128),
		};

		// message that only pays fees
		let message = Xcm(vec![
			WithdrawAsset(asset_to_withdraw.into()),
			BuyExecution {
				fees: (hdx_loc, 800_000_000_000u128).into(),
				weight_limit: Unlimited,
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act
		assert_ok!(call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));

		// Assert
		// Accumulator should still be 0 because BuyExecution fees stay on-chain
		assert_eq!(hydradx_runtime::CircuitBreaker::withdraw_limit_accumulator().0, 0);
		assert!(hydradx_runtime::CircuitBreaker::withdraw_lockdown_until().is_none());
	});
}

#[test]
fn lockdown_expiry_allows_egress() {
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
		
		let message = Xcm(vec![
			WithdrawAsset((Here, 1000).into()),
			DepositReserveAsset {
				assets: All.into(),
				dest: Location::parent(),
				xcm: Xcm(vec![]),
			},
		]);
		
		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		// Blocked initially
		assert_noop!(
			call.clone().dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered
		);

		// Advance time past lockdown
		pallet_timestamp::Now::<hydradx_runtime::Runtime>::put(now + 1001);

		// Now it should pass CallFilter (dispatch will still fail due to other reasons but not CallFiltered)
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert_ne!(res, Err(frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered.into()));
	});
}

// todo:
// #[test]
// fn token_gateway_teleport_blocked_during_lockdown() {
// 	Hydra::execute_with(|| {
// 		// Arrange
// 		let now = CircuitBreaker::timestamp_now();
// 		pallet_circuit_breaker::WithdrawLockdownUntil::<hydradx_runtime::Runtime>::put(now + 1000);
//
// 		let params = hydradx_runtime::pallet_token_gateway::types::TeleportParams {
// 			asset_id: 0, // Native asset
// 			destination: StateMachine::Polkadot(1000),
// 			recepient: [1u8; 32].into(),
// 			amount: 1000,
// 			call_data: None,
// 			redeem: false,
// 		};
//
// 		let call = RuntimeCall::TokenGateway(hydradx_runtime::pallet_token_gateway::Call::teleport {
// 			params,
// 		});
//
// 		// Act & Assert
// 		// It should fail with GlobalLockdownActive because it goes through pallet_currencies
// 		assert_noop!(
// 			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
// 			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::GlobalLockdownActive
// 		);
// 	});
// }
