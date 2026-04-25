use super::mock::*;
use crate::*;
use frame_support::pallet_prelude::Zero;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};
use pallet_currencies::fungibles::FungibleCurrencies;
use sp_runtime::Permill;

#[test]
fn on_idle_retries_failed_conversion_on_next_block() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Conversion fails - asset stays pending
		set_convert_result(None);
		let weight = frame_support::weights::Weight::from_parts(200_000_000, 0);
		FeeProcessor::on_idle(1u64, weight);

		assert!(PendingConversions::<Test>::contains_key(DOT));

		// Conversion succeeds - asset removed from pending
		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();
		System::set_block_number(2);
		FeeProcessor::on_idle(2u64, weight);

		assert!(!PendingConversions::<Test>::contains_key(DOT));
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
fn convert_extrinsic_for_asset_not_in_pending_still_executes() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Fund pot with DOT but do NOT insert into PendingConversions
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		assert!(!PendingConversions::<Test>::contains_key(DOT));

		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		// do_convert has no guard - succeeds regardless of PendingConversions membership
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

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

		// Set up DOT pending conversion
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Mock convert to fail
		set_convert_result(None);

		let weight = frame_support::weights::Weight::from_parts(200_000_000, 0);
		let _used = FeeProcessor::on_idle(1u64, weight);

		// Pending should be kept on failure so on_idle retries next block
		assert!(PendingConversions::<Test>::contains_key(DOT));

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

#[test]
fn distribute_to_pots_uses_total_param_not_actual_pot_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Pot already has extra HDX beyond ED (simulates leftover from previous rounds)
		let pre_existing_hdx = 500 * ONE;
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, pre_existing_hdx).unwrap();

		// Set up DOT for conversion
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 200 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// MockConvert returns 1000 ONE of HDX
		let hdx_received = 1000 * ONE;
		set_convert_result(Some(hdx_received));
		// Fund pot with the HDX that "convert" will produce
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, hdx_received).unwrap();

		let staking_before =
			<FungibleCurrencies<Test> as frame_support::traits::fungibles::Inspect<AccountId>>::balance(
				HDX,
				&STAKING_POT,
			);
		let referrals_before =
			<FungibleCurrencies<Test> as frame_support::traits::fungibles::Inspect<AccountId>>::balance(
				HDX,
				&REFERRALS_POT,
			);

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		let staking_after = <FungibleCurrencies<Test> as frame_support::traits::fungibles::Inspect<AccountId>>::balance(
			HDX,
			&STAKING_POT,
		);
		let referrals_after =
			<FungibleCurrencies<Test> as frame_support::traits::fungibles::Inspect<AccountId>>::balance(
				HDX,
				&REFERRALS_POT,
			);

		let staking_received = staking_after - staking_before;
		let referrals_received = referrals_after - referrals_before;

		// FeeReceivers: StakingFeeReceiver=70%, ReferralsFeeReceiver=30%
		// distribute_to_pots uses `hdx_received` (1000 ONE), NOT actual pot balance (1500+ ONE)
		assert_eq!(staking_received, Permill::from_percent(70).mul_floor(hdx_received));
		assert_eq!(referrals_received, Permill::from_percent(30).mul_floor(hdx_received));

		// Pre-existing HDX (500 ONE) + ED (1 ONE) stays on pot - not distributed
		let pot_balance_after =
			<FungibleCurrencies<Test> as frame_support::traits::fungibles::Inspect<AccountId>>::balance(HDX, &pot);
		assert_eq!(pot_balance_after, ONE + pre_existing_hdx); // only ED + leftover remains
	});
}

#[test]
fn on_idle_returns_zero_when_weight_below_single_conversion() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		set_convert_result(Some(1000 * ONE));

		// One conversion costs Weight::from_parts(100_000_000, 0); pass one unit less
		let below_threshold = frame_support::weights::Weight::from_parts(99_999_999, 0);
		let used = FeeProcessor::on_idle(1u64, below_threshold);

		assert!(used.is_zero());
		assert!(PendingConversions::<Test>::contains_key(DOT));
	});
}

#[test]
fn on_idle_processes_only_one_when_weight_fits_exactly_one() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DAI, &pot, 300 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());
		PendingConversions::<Test>::insert(DAI, ());

		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 2000 * ONE).unwrap();

		// Weight for exactly one conversion
		let one_conversion = frame_support::weights::Weight::from_parts(100_000_000, 0);
		let used = FeeProcessor::on_idle(1u64, one_conversion);

		assert_eq!(used, one_conversion);
		// Exactly one asset removed, one still pending
		assert_eq!(PendingConversions::<Test>::count(), 1);
	});
}
