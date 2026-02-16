#![cfg(test)]

use crate::evm::init_omnipool_with_oracle_for_block_10;
use crate::polkadot_test_net::*;
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_noop, assert_ok};
use hydradx_runtime::{AssetRegistry, CircuitBreaker, RuntimeCall, DOT_ASSET_LOCATION};
use orml_traits::MultiCurrency;
use pallet_circuit_breaker::GlobalAssetCategory;
use pallet_transaction_payment::OnChargeTransaction;
use polkadot_xcm::v5::prelude::*;
use polkadot_xcm::{VersionedAssetId, VersionedXcm};
use primitives::constants::time::unix_time::DAY;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use sp_runtime::traits::Dispatchable;
use sp_runtime::FixedU128;
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
fn on_charge_transaction_skips_global_withdraw_accounting_for_native_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Arrange
		let alice: AccountId = ALICE.into();

		// Ensure HDX is a participating asset for the global-withdraw logic
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::Local)
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
fn on_charge_transaction_skips_global_withdraw_accounting_for_external_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		// Arrange
		let alice: AccountId = ALICE.into();

		// Ensure DOT is a participating asset for the global-withdraw logic
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));

		assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			FixedU128::from_rational(50, 100),
		));

		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(alice.clone()),
			DOT
		));

		// Activate global lockdown
		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1000
		));

		let initial_alice_balance = Currencies::free_balance(DOT, &alice);
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
		let after_alice_balance = Currencies::free_balance(DOT, &alice);
		assert!(after_alice_balance < initial_alice_balance, "Fee should be charged");

		// Verify global-withdraw accounting was skipped for the fee withdraw
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);

		// Also assert lockdown is still active
		assert!(CircuitBreaker::withdraw_lockdown_until().is_some());

		// Negative control: normal (non-fee) operations with participating asset are blocked during lockdown
		assert_err!(
			Currencies::withdraw(
				DOT,
				&ALICE.into(),
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
		// Set HDX as External to avoid conversion issues
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::External)
		));

		let amount = 100 * UNITS;
		assert_ok!(Currencies::deposit(HDX, &ALICE.into(), amount));

		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;

		assert_ok!(Currencies::withdraw(
			HDX,
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
		// HDX has type Token, so it should be None by default
		let amount = 100 * UNITS;

		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;

		assert_ok!(Currencies::withdraw(
			HDX,
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
		init_omnipool_with_oracle_for_block_10();

		let sink: AccountId = [99u8; 32].into();
		assert_ok!(CircuitBreaker::add_egress_accounts(
			hydradx_runtime::RuntimeOrigin::root(),
			vec![sink.clone()]
		));

		let amount = 100 * UNITS;

		// 1. External -> Accounted (Override DOT to External)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));
		assert_ok!(AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));
		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			sink.clone(),
			DOT,
			amount
		));

		let accumulator_after_ext = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			accumulator_after_ext > initial_accumulator,
			"Accumulator should increase for External transfer to sink"
		);

		// 2. Local -> Accounted (Override HDX to Local)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::Local)
		));
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			sink.clone(),
			HDX,
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
fn should_account_withdraw_operation_accounts_external_and_local() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		let who: AccountId = ALICE.into();
		let amount = 10 * UNITS;

		assert_ok!(CircuitBreaker::reset_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root()
		));

		// --- External -> accounted for withdraw ---
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));
		assert_ok!(AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));
		assert!(Currencies::free_balance(DOT, &who) >= amount);
		let acc0 = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::withdraw(
			DOT,
			&who,
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));
		assert!(
			CircuitBreaker::withdraw_limit_accumulator().0 > acc0,
			"External category must be accounted on withdraw"
		);

		// --- Local -> also accounted for withdraw (any Some category is accounted) ---
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			HDX,
			Some(GlobalAssetCategory::Local)
		));
		assert!(Currencies::free_balance(HDX, &who) >= amount);
		let acc1 = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::withdraw(
			HDX,
			&who,
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));
		assert!(
			CircuitBreaker::withdraw_limit_accumulator().0 > acc1,
			"Local category must also be accounted on withdraw"
		);
	});
}

#[test]
fn transfer_to_non_egress_succeeds_during_lockdown_and_does_not_change_accumulator() {
	// Lockdown must not block non-egress transfers and accounting must be skipped.
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into(); // not in egress list

		// on_init already set lockdown active and HDX as Local
		// Set to External to make the test more meaningful (External is accounted on egress)
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));
		assert_ok!(AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

		let acc_before = CircuitBreaker::withdraw_limit_accumulator().0;
		// Transfer to non-egress account should succeed even during lockdown
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice),
			bob,
			DOT,
			5 * UNITS
		));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, acc_before);
	});
}

#[test]
fn ingress_deposit_decrements_accumulator_for_external() {
	// External deposits are always accounted as ingress and decrement the accumulator.
	TestNet::reset();
	Hydra::execute_with(|| {
		let who: AccountId = ALICE.into();
		let amount = 10 * UNITS;

		assert_ok!(CircuitBreaker::reset_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root()
		));
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::External)
		));

		// Prime accumulator via an egress withdraw.
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &who, amount * 2));
		assert_ok!(Currencies::withdraw(
			CORE_ASSET_ID,
			&who,
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));
		let acc_after_withdraw = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			acc_after_withdraw >= amount,
			"Accumulator must be >= deposit amount for exact decrement assertion"
		);

		// Ingress deposit (maybe_from = None) should decrement.
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &who, amount));
		assert_eq!(
			CircuitBreaker::withdraw_limit_accumulator().0,
			acc_after_withdraw - amount
		);
	});
}

#[test]
fn ingress_deposit_decrements_accumulator_for_local_only_when_source_is_egress() {
	// Local deposits are accounted only when `maybe_from` is an egress account.
	TestNet::reset();
	Hydra::execute_with(|| {
		let alice: AccountId = ALICE.into();
		let egress_src: AccountId = [9u8; 32].into();
		let non_egress_src: AccountId = [10u8; 32].into();
		let amount = 7 * UNITS;

		assert_ok!(CircuitBreaker::reset_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root()
		));
		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::Local)
		));

		// Prime accumulator so we can observe a decrement.
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &alice, amount * 2));
		assert_ok!(Currencies::withdraw(
			CORE_ASSET_ID,
			&alice,
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));
		let acc_primed = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			acc_primed >= amount,
			"Accumulator must be >= transfer amount for exact decrement assertion"
		);

		// Fund sources for transfer-based deposits.
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &egress_src, amount));
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &non_egress_src, amount));
		assert_ok!(CircuitBreaker::add_egress_accounts(
			hydradx_runtime::RuntimeOrigin::root(),
			vec![egress_src.clone()]
		));

		// Deposit via transfer from egress source: should decrement.
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(egress_src.clone()),
			alice.clone(),
			CORE_ASSET_ID,
			amount
		));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, acc_primed - amount);

		// Deposit via transfer from non-egress source: must NOT decrement.
		let before_non_egress = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(non_egress_src),
			alice,
			CORE_ASSET_ID,
			amount
		));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, before_non_egress);
	});
}

#[test]
fn dot_external_limit_trigger_fails_then_decays_to_zero() {
	// Once DOT is a participating asset (External), exceeding the configured global-withdraw limit
	// must fail; after the window passes, the accumulator must fully decay to 0.
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(CircuitBreaker::reset_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root()
		));

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));
		// Keep DOT location consistent with other DOT/XCM tests (not strictly required for this test).
		assert_ok!(AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

		let alice: AccountId = ALICE.into();
		let amount = 1 * UNITS;
		assert!(
			Currencies::free_balance(DOT, &alice) >= amount * 3,
			"Test requires Alice to have enough DOT"
		);
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);

		// Prime accumulator with one successful egress.
		assert_ok!(Currencies::withdraw(
			DOT,
			&alice,
			amount,
			frame_support::traits::ExistenceRequirement::AllowDeath
		));
		let acc_after_first = CircuitBreaker::withdraw_limit_accumulator().0;
		assert!(
			acc_after_first > 0,
			"DOT withdraw should be accounted and increase the accumulator"
		);

		// Set a limit that allows the first withdraw but blocks the next one.
		assert_ok!(CircuitBreaker::set_global_withdraw_limit(
			hydradx_runtime::RuntimeOrigin::root(),
			acc_after_first + 1
		));

		assert_err!(
			Currencies::withdraw(
				DOT,
				&alice,
				amount,
				frame_support::traits::ExistenceRequirement::AllowDeath
			),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::GlobalWithdrawLimitExceeded
		);
		assert_eq!(
			CircuitBreaker::withdraw_limit_accumulator().0,
			acc_after_first,
			"Accumulator must not be mutated on a failed increment"
		);

		// Advance time past the window; trigger decay via a no-op egress (amount 0) and assert full decay.
		let now = CircuitBreaker::timestamp_now();
		pallet_timestamp::Pallet::<hydradx_runtime::Runtime>::set_timestamp(now + DAY * 2);
		assert_ok!(CircuitBreaker::note_egress(0));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}

#[test]
fn dot_external_lockdown_blocks_withdraw_but_regular_dot_transfer_still_works() {
	// During lockdown, egress accounting must fail fast, but normal (non-egress-dest) DOT transfers
	// must still be allowed.
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(CircuitBreaker::reset_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root()
		));

		assert_ok!(CircuitBreaker::set_asset_category(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Some(GlobalAssetCategory::External)
		));
		assert_ok!(AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION));

		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let amount = 1 * UNITS;
		assert!(Currencies::free_balance(DOT, &alice) >= amount * 2);
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);

		let now = CircuitBreaker::timestamp_now();
		assert_ok!(CircuitBreaker::set_global_withdraw_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			now + 1_000
		));

		// Egress is blocked during lockdown.
		assert_err!(
			Currencies::withdraw(
				DOT,
				&alice,
				amount,
				frame_support::traits::ExistenceRequirement::AllowDeath
			),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::WithdrawLockdownActive
		);

		// But a regular transfer to a non-egress destination must still work.
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(alice),
			bob,
			DOT,
			amount
		));
		assert_eq!(CircuitBreaker::withdraw_limit_accumulator().0, 0);
	});
}
