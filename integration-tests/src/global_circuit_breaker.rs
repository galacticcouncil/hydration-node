#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::ExistenceRequirement};
use hydradx_runtime::{
	circuit_breaker::WithdrawCircuitBreaker, CircuitBreaker, Currencies, NativeAssetId, RuntimeOrigin,
};
use orml_traits::MultiCurrency;
use pallet_circuit_breaker::GlobalAssetCategory;
use xcm_emulator::TestExt;

#[test]
fn withdraw_external_should_be_accounted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Set CORE_ASSET_ID (HDX) as External to avoid conversion issues
		assert_ok!(CircuitBreaker::set_asset_category(
			RuntimeOrigin::root(),
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
			ExistenceRequirement::AllowDeath
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
			ExistenceRequirement::AllowDeath
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
			RuntimeOrigin::root(),
			vec![sink.clone()]
		));

		let amount = 100 * UNITS;
		assert_ok!(Currencies::deposit(CORE_ASSET_ID, &ALICE.into(), amount * 2));

		// 1. External -> Accounted (Override CORE_ASSET_ID to External)
		assert_ok!(CircuitBreaker::set_asset_category(
			RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::External)
		));
		let initial_accumulator = CircuitBreaker::withdraw_limit_accumulator().0;
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
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
			RuntimeOrigin::root(),
			CORE_ASSET_ID,
			Some(GlobalAssetCategory::Local)
		));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
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
			RuntimeOrigin::root(),
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
			RuntimeOrigin::root(),
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
