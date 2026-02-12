use super::mock::*;
use crate::*;
use frame_support::pallet_prelude::Zero;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};
use pallet_currencies::fungibles::FungibleCurrencies;

#[test]
fn convert_extrinsic_works() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Fund pot with DOT above minimum
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Mock convert returns 1000 * ONE of HDX
		set_convert_result(Some(1000 * ONE));

		// Fund pot with HDX for distribution (convert result goes here)
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		// Should have called convert
		let calls = convert_calls();
		assert_eq!(calls.len(), 1);
		assert_eq!(calls[0].1, DOT); // asset_from
		assert_eq!(calls[0].2, HDX); // asset_to

		// Pending should be removed
		assert!(!PendingConversions::<Test>::contains_key(DOT));

		// Event emitted
		System::assert_has_event(
			Event::Converted {
				asset_id: DOT,
				amount_in: 500 * ONE,
				hdx_out: 1000 * ONE,
			}
			.into(),
		);
	});
}

#[test]
fn convert_fails_for_hdx() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			FeeProcessor::convert(RuntimeOrigin::signed(ALICE), HDX),
			Error::<Test>::AlreadyHdx
		);
	});
}

#[test]
fn convert_fails_below_minimum() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();
		// Fund pot with DOT below minimum (100)
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 50).unwrap();

		assert_noop!(
			FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT),
			Error::<Test>::AmountTooLow
		);
	});
}

#[test]
fn on_idle_converts_within_weight_budget() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Set up DOT pending conversion
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Set up DAI pending conversion
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DAI, &pot, 300 * ONE).unwrap();
		PendingConversions::<Test>::insert(DAI, ());

		// Mock: convert returns 1000 HDX and ensure pot has enough
		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 2000 * ONE).unwrap();

		// Provide enough weight for 2 conversions
		let weight = frame_support::weights::Weight::from_parts(200_000_000, 0);
		let used = FeeProcessor::on_idle(1u64, weight);

		assert!(!used.is_zero());

		// Both should be converted
		assert!(!PendingConversions::<Test>::contains_key(DOT));
		assert!(!PendingConversions::<Test>::contains_key(DAI));
	});
}

#[test]
fn on_idle_handles_conversion_failure_gracefully() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Set up DOT pending conversion with amount above minimum
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Mock convert to fail
		set_convert_result(None);

		let weight = frame_support::weights::Weight::from_parts(200_000_000, 0);
		let _used = FeeProcessor::on_idle(1u64, weight);

		// Pending should be removed even on failure
		assert!(!PendingConversions::<Test>::contains_key(DOT));

		// ConversionFailed event emitted (DispatchError::Other loses string through encoding)
		System::assert_has_event(
			Event::ConversionFailed {
				asset_id: DOT,
				reason: DispatchError::Other(""),
			}
			.into(),
		);
	});
}

#[test]
fn on_idle_respects_max_conversions_per_block() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Set up 10 pending conversions (but max is 5)
		for asset_id in 10..20u32 {
			<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(asset_id, &pot, 500 * ONE).unwrap();
			PendingConversions::<Test>::insert(asset_id, ());
		}

		set_convert_result(Some(100 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 10_000 * ONE).unwrap();

		// Provide a LOT of weight
		let weight = frame_support::weights::Weight::from_parts(u64::MAX, u64::MAX);
		let _used = FeeProcessor::on_idle(1u64, weight);

		// Should have processed at most MaxConversionsPerBlock (5)
		let remaining = PendingConversions::<Test>::count();
		assert!(
			remaining >= 5,
			"Should have at most 5 processed, remaining = {remaining}"
		);
	});
}
