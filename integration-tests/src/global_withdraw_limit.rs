#![cfg(test)]

use crate::evm::init_omnipool_with_oracle_for_block_10;
use crate::polkadot_test_net::*;
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::circuit_breaker::WithdrawCircuitBreaker;
use hydradx_runtime::{CircuitBreaker, NativeAssetId, RuntimeCall, DOT_ASSET_LOCATION};
use orml_traits::MultiCurrency;
use pallet_circuit_breaker::GlobalAssetCategory;
use polkadot_xcm::v5::prelude::*;
use polkadot_xcm::{VersionedAssetId, VersionedXcm};
use primitives::constants::time::MILLISECS_PER_BLOCK;
use sp_runtime::traits::Dispatchable;
use xcm_emulator::TestExt;
use xcm_executor::traits::TransferType;

#[test]
fn polkadot_xcm_execute_should_fail_when_lockdown_active_and_message_is_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = Xcm(vec![WithdrawAsset((Here, 1000).into()), ClearOrigin]);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert_eq!(
			res.map_err(|e| e.error),
			Err(pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncomplete.into())
		);

		// Assert invariants
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn polkadot_xcm_send_should_fail_when_lockdown_active_and_message_is_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			0, // Native asset
			1_000_000_000_000i128,
		));

		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = Xcm(vec![
			WithdrawAsset((Here, 1000).into()),
			BuyExecution {
				fees: (Here, 1000).into(),
				weight_limit: Unlimited,
			},
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
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert_ne!(
			res.map_err(|e| e.error),
			Err(pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncomplete.into())
		);

		// Assert invariants
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn polkadot_xcm_execute_should_succeed_when_lockdown_active_and_message_is_not_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = Xcm(vec![ClearOrigin]);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert_ne!(
			res,
			Err(pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncomplete.into())
		);

		// Assert invariants
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn root_origin_is_not_blocked_by_xcm_lockdown_gate() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = Xcm(vec![ClearOrigin]);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		// Root origin is not blocked by the same gate as users
		assert_ok!(call.dispatch(hydradx_runtime::RuntimeOrigin::root()));

		// Assert invariants
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn xcm_reserve_transfer_assets_blocked_during_lockdown() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle_for_block_10();
		hydradx_runtime::ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(ASSET_HUB_PARA_ID.into());

		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));

		assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

		let dot: Asset = Asset {
			id: cumulus_primitives_core::AssetId(DOT_ASSET_LOCATION.into()),
			fun: Fungible(1 * UNITS),
		};

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::limited_reserve_transfer_assets {
			dest: Box::new(Location::new(1, [Parachain(ASSET_HUB_PARA_ID)]).into()),
			beneficiary: Box::new(
				Location::new(
					0,
					[AccountId32 {
						network: None,
						id: ALICE,
					}],
				)
				.into(),
			),
			assets: Box::new(vec![dot].into()),
			fee_asset_item: 0,
			weight_limit: Unlimited,
		});

		// Act & Assert
		let res = call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()));
		assert!(res.is_err());
		assert_ne!(
			res,
			Err(frame_system::Error::<hydradx_runtime::Runtime>::CallFiltered.into())
		);

		// Assert invariants
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn xcm_fee_like_messages_do_not_change_global_withdraw_accumulator() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		// Set a limit but no lockdown yet
		assert_ok!(hydradx_runtime::CircuitBreaker::set_global_withdraw_limit(
			hydradx_runtime::RuntimeOrigin::root(),
			1_000_000_000_000
		));

		let message = Xcm(vec![ClearOrigin]);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act
		assert_ok!(call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));

		// Assert
		// Accumulator should still be 0
		assert_eq!(hydradx_runtime::CircuitBreaker::withdraw_limit_accumulator().0, 0);
		assert!(hydradx_runtime::CircuitBreaker::withdraw_lockdown_until().is_none());
	});
}

#[test]
fn lockdown_expiry_allows_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();
		let until = now + MILLISECS_PER_BLOCK * 11;
		init_omnipool_with_oracle_for_block_10();

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));

		assert_ok!(CircuitBreaker::set_global_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			until
		));

		assert_ok!(hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

		let dot: Asset = Asset {
			id: cumulus_primitives_core::AssetId(DOT_ASSET_LOCATION.into()),
			fun: Fungible(1 * UNITS),
		};

		let bob_beneficiary = Location::new(
			0,
			[cumulus_primitives_core::Junction::AccountId32 { id: BOB, network: None }],
		);

		let deposit_xcm = Xcm(vec![DepositAsset {
			assets: Wild(WildAsset::AllCounted(1)),
			beneficiary: bob_beneficiary.clone(),
		}]);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::transfer_assets_using_type_and_then {
			dest: Box::new(
				Location {
					parents: 1,
					interior: [Junction::Parachain(ASSET_HUB_PARA_ID)].into(),
				}
				.into_versioned(),
			),
			assets: Box::new(dot.into()),
			assets_transfer_type: Box::new(TransferType::DestinationReserve),
			remote_fees_id: Box::new(VersionedAssetId::V5(AssetId(DOT_ASSET_LOCATION.into()))),
			fees_transfer_type: Box::new(TransferType::DestinationReserve),
			custom_xcm_on_dest: Box::new(VersionedXcm::from(deposit_xcm)),
			weight_limit: WeightLimit::Unlimited,
		});

		// Act & Assert
		// Blocked initially
		assert_noop!(
			call.clone()
				.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncomplete
		);

		// Advance time past lockdown
		pallet_timestamp::Pallet::<hydradx_runtime::Runtime>::set_timestamp(until);
		hydradx_run_to_next_block();
		hydradx_runtime::ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(ASSET_HUB_PARA_ID.into());

		// Now it should pass a lockdown check
		assert_ok!(call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
	});
}

#[test]
fn withdraw_external_should_be_accounted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Set CORE_ASSET_ID (HDX) as External to avoid conversion issues
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::External)
		));

		let amount = 100 * UNITS;
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &ALICE.into(), amount));

		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;

		assert_ok!(Currencies::withdraw(
			CORE_ASSET_ID,
			&ALICE.into(),
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));

		let final_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			final_accumulator > initial_accumulator,
			"Accumulator should increase for external withdraw"
		);
	});
}

#[test]
fn withdraw_token_without_override_should_not_be_accounted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// CORE_ASSET_ID is Token, so it should be None by default
		let amount = 100 * UNITS;
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &ALICE.into(), amount));

		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;

		assert_ok!(Currencies::withdraw(
			CORE_ASSET_ID,
			&ALICE.into(),
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));

		let final_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_eq!(
			final_accumulator, initial_accumulator,
			"Accumulator should NOT increase for token withdraw without override"
		);
	});
}

#[test]
fn transfer_to_sink_should_be_accounted_for_participating_assets() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sink: AccountId = [99u8; 32].into();
		assert_ok!(CircuitBreaker::add_egress_accounts(
			hydradx_runtime::RuntimeOrigin::root(),
			vec![sink.clone()]
		));

		let amount = 100 * UNITS;
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &ALICE.into(), amount * 2));

		// 1. External -> Accounted (Override CORE_ASSET_ID to External)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::External)
		));
		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			sink.clone(),
			CORE_ASSET_ID,
			amount
		));
		let accumulator_after_ext = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			accumulator_after_ext > initial_accumulator,
			"Accumulator should increase for External transfer to sink"
		);

		// 2. Local -> Accounted (Override CORE_ASSET_ID to Local)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::Local)
		));
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			sink.clone(),
			CORE_ASSET_ID,
			amount
		));
		let accumulator_after_local = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			accumulator_after_local > accumulator_after_ext,
			"Accumulator should increase for Local transfer to sink"
		);
	});
}

#[test]
fn note_local_egress_should_work_only_for_local_assets() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let amount = 100 * UNITS;

		// Set CORE_ASSET_ID as Local
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::Local)
		));

		// 1. Local asset -> Accounted
		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(WithdrawCircuitBreaker::<NativeAssetId>::note_local_egress(
			CORE_ASSET_ID,
			amount
		));
		let accumulator_after_local = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			accumulator_after_local > initial_accumulator,
			"note_local_egress should increase accumulator for Local asset"
		);

		// 2. Non-Local asset -> NOT accounted (set to None)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			None
		));
		assert_ok!(WithdrawCircuitBreaker::<NativeAssetId>::note_local_egress(
			CORE_ASSET_ID,
			amount
		));
		let accumulator_after_none = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_eq!(
			accumulator_after_none, accumulator_after_local,
			"note_local_egress should NOT increase accumulator for non-Local asset"
		);
	});
}
