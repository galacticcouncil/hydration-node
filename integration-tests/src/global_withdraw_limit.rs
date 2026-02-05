#![cfg(test)]

use crate::evm::init_omnipool_with_oracle_for_block_10;
use crate::polkadot_test_net::*;
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_noop, assert_ok};
use hydradx_runtime::circuit_breaker::WithdrawCircuitBreaker;
use hydradx_runtime::{CircuitBreaker, NativeAssetId, RuntimeCall, DOT_ASSET_LOCATION};
use orml_traits::MultiCurrency;
use pallet_circuit_breaker::GlobalAssetCategory;
use pallet_transaction_payment::OnChargeTransaction;
use polkadot_xcm::v5::prelude::*;
use polkadot_xcm::{VersionedAssetId, VersionedXcm};
use primitives::constants::time::MILLISECS_PER_BLOCK;
use sp_runtime::traits::Dispatchable;
use xcm_emulator::TestExt;
use xcm_executor::traits::{ConvertLocation, TransferType};

fn hdx_location() -> Location {
	Location::new(1, [Parachain(HYDRA_PARA_ID), GeneralIndex(0)])
}

fn xcm_message_withdraw_deposit(token_location: Location, amount: Balance) -> Xcm<hydradx_runtime::RuntimeCall> {
	let asset: Asset = Asset {
		id: AssetId(token_location),
		fun: Fungible(amount),
	};

	Xcm(vec![
		WithdrawAsset(asset.clone().into()),
		BuyExecution {
			fees: asset.into(),
			weight_limit: Unlimited,
		},
		DepositReserveAsset {
			assets: All.into(),
			dest: Location::parent(),
			xcm: Xcm(vec![]),
		},
	])
}

fn set_dot_external_and_get_transfer_call() -> hydradx_runtime::RuntimeCall {
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

	let bob_beneficiary = Location::new(
		0,
		[cumulus_primitives_core::Junction::AccountId32 { id: BOB, network: None }],
	);

	let deposit_xcm = Xcm(vec![DepositAsset {
		assets: Wild(WildAsset::AllCounted(1)),
		beneficiary: bob_beneficiary.clone(),
	}]);

	RuntimeCall::PolkadotXcm(pallet_xcm::Call::transfer_assets_using_type_and_then {
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
	})
}

#[test]
fn polkadot_xcm_execute_should_fail_when_lockdown_active_and_asset_is_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle_for_block_10();
		let now = CircuitBreaker::timestamp_now();

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::Local)
		));

		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = xcm_message_withdraw_deposit(hdx_location(), 10 * UNITS);
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
fn polkadot_xcm_execute_should_succeed_when_lockdown_active_and_asset_is_not_egress() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let now = CircuitBreaker::timestamp_now();

		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let message = xcm_message_withdraw_deposit(hdx_location(), 10 * UNITS);

		let call = RuntimeCall::PolkadotXcm(pallet_xcm::Call::execute {
			message: Box::new(VersionedXcm::from(message)),
			max_weight: Weight::from_parts(1_000_000_000_000, 0),
		});

		// Act & Assert
		assert_ok!(call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		// Assert invariants
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn xtokens_transfer_should_fail_when_lockdown_active_and_asset_is_egress() {
	let bob_location = Location::new(1, Junction::AccountId32 { network: None, id: BOB });

	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle_for_block_10();
		let now = CircuitBreaker::timestamp_now();

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::Local)
		));

		let dest_account = xcm_builder::ParentIsPreset::convert_location(&bob_location.chain_location()).unwrap();
		assert_ok!(CircuitBreaker::add_egress_accounts(
			hydradx_runtime::RuntimeOrigin::root(),
			vec![dest_account]
		));

		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let call = RuntimeCall::XTokens(orml_xtokens::Call::transfer {
			currency_id: HDX,
			amount: 1 * UNITS,
			dest: Box::new(bob_location.into_versioned()),
			dest_weight_limit: WeightLimit::Unlimited,
		});

		// Act & Assert
		assert_noop!(
			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			orml_xtokens::Error::<hydradx_runtime::Runtime>::XcmExecutionFailed
		);

		// Assert invariants
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn on_charge_transaction_skips_global_withdraw_accounting() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let alice: AccountId = ALICE.into();

		// Ensure HDX is a participating asset for the global-withdraw logic
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::External)
		));

		// Activate global lockdown
		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let initial_alice_balance = Currencies::free_balance(HDX, &alice);
		let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1, 2, 3] });

		// Act
		let fee_amount = 1 * UNITS;
		let _ = <hydradx_runtime::Runtime as pallet_transaction_payment::Config>::OnChargeTransaction::withdraw_fee(
			&alice,
			&call,
			&Default::default(),
			fee_amount,
			0,
		)
		.expect("Fee withdrawal should succeed even during lockdown");

		// Assert
		// Fee charge must work even during global lockdown
		let after_alice_balance = Currencies::free_balance(HDX, &alice);
		assert!(after_alice_balance < initial_alice_balance, "Fee should be charged");

		// Verify global-withdraw accounting was skipped for the fee withdraw
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);

		// Also assert lockdown is still active
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());

		// Negative control: normal (non-fee) operations with participating asset are blocked during lockdown
		assert_err!(
			Currencies::withdraw(
				HDX,
				&BOB.into(),
				1 * UNITS,
				frame_support::traits::ExistenceRequirement::AllowDeath
			),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::WithdrawLockdownActive
		);
	});
}

#[test]
fn xcm_transfer_assets_blocked_during_lockdown() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle_for_block_10();

		let now = CircuitBreaker::timestamp_now();
		let until = now + MILLISECS_PER_BLOCK * 11;

		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			until
		));

		let call = set_dot_external_and_get_transfer_call();

		// Act & Assert
		assert_noop!(
			call.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())),
			pallet_xcm::Error::<hydradx_runtime::Runtime>::LocalExecutionIncomplete
		);
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

		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			until
		));

		let call = set_dot_external_and_get_transfer_call();

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
