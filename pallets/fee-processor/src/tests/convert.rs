use super::mock::*;
use crate::*;
use frame_support::pallet_prelude::Zero;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::fungibles::{Inspect, Mutate};
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};
use pallet_currencies::fungibles::FungibleCurrencies;
use sp_runtime::Permill;

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

		// Pending entry is dropped on failure — a subsequent fee will re-insert it.
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
fn convert_extrinsic_for_asset_not_in_pending_still_executes() {
	// `convert` is permissionless and not gated on PendingConversions membership.
	// As long as the asset is convertible (non-HDX, has balance) the swap proceeds.
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Fund pot with DOT but DO NOT mark it pending.
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		assert!(!PendingConversions::<Test>::contains_key(DOT));

		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		assert_eq!(convert_calls().len(), 1);
		assert!(!PendingConversions::<Test>::contains_key(DOT));
	});
}

#[test]
fn distribute_proportionally_uses_total_param_not_actual_pot_balance() {
	// Regression: `do_convert` passes `hdx_received` (the swap output) to
	// `distribute_proportionally`, NOT the pot's full HDX balance. Any pre-existing
	// HDX in the pot (e.g. ED seeding, stray dust) must remain untouched.
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Add extra HDX on top of the pot's seeded ED — must NOT be redistributed.
		let extra_preexisting_hdx = 5_000 * ONE;
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, extra_preexisting_hdx).unwrap();
		let pot_hdx_before_swap_topup = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &pot);

		// Convert produces only 1000 HDX — the receivers should split this, not the pot total.
		let hdx_from_swap = 1_000 * ONE;
		set_convert_result(Some(hdx_from_swap));
		// Top up the pot so the swap output is physically available for distribution
		// (MockConvert only signals success; the HDX must already be in the pot to be transferred).
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, hdx_from_swap).unwrap();

		let staking_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT);
		let referrals_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &REFERRALS_POT);

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		// Only the convert receiver (staking) shares the swap output; the raw referrals
		// receiver is paid in the original asset, not from the converted HDX.
		let staking_received =
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT) - staking_before;
		let referrals_received =
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &REFERRALS_POT) - referrals_before;
		assert_eq!(
			staking_received,
			1_000 * ONE,
			"staking (only convert receiver) gets the full swap output"
		);
		assert_eq!(referrals_received, 0, "raw referrals receiver gets no converted HDX");

		// Pre-existing HDX in pot is untouched (pot balance equals what was there before
		// the swap topup, since the swap output flows out to receivers in full).
		let pot_hdx_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &pot);
		assert_eq!(
			pot_hdx_after, pot_hdx_before_swap_topup,
			"pre-existing HDX must remain in the pot — distribute uses the `total` param, not pot balance"
		);
	});
}

#[test]
fn on_idle_conversion_is_atomic_when_a_later_receiver_transfer_fails() {
	// Regression: `on_idle` establishes no storage layer, so `do_convert` must be
	// `#[transactional]`. With two convert receivers, fund the pot enough for the
	// first payout but not the second; the second transfer fails after the first
	// succeeded. Everything must roll back — the first receiver keeps nothing.
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// Two non-raw convert receivers: staking 70%, second 30%.
		set_second_convert_pct(Permill::from_percent(30));

		// Pending DOT to convert.
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(DOT, &pot, 500 * ONE).unwrap();
		PendingConversions::<Test>::insert(DOT, ());

		// Swap "produces" 1000 HDX, but physically fund the pot with only the staking
		// slice (700) — the second receiver's 300 transfer will fail on insufficient pot.
		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 700 * ONE).unwrap();

		let staking_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT);
		let second_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &SECOND_POT);
		let pot_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &pot);

		let weight = frame_support::weights::Weight::from_parts(200_000_000, 0);
		let _used = FeeProcessor::on_idle(1u64, weight);

		// The whole conversion rolled back: neither receiver was paid, pot HDX intact.
		assert_eq!(
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT),
			staking_before,
			"first receiver payout must roll back when a later transfer fails"
		);
		assert_eq!(
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &SECOND_POT),
			second_before,
		);
		assert_eq!(
			<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &pot),
			pot_before,
			"pot HDX untouched — swap output not distributed"
		);

		// Pending entry dropped by on_idle's failure arm; failure surfaced as an event.
		assert!(!PendingConversions::<Test>::contains_key(DOT));
		assert!(
			System::events().iter().any(|r| matches!(
				r.event,
				RuntimeEvent::FeeProcessor(Event::ConversionFailed { asset_id, .. }) if asset_id == DOT
			)),
			"a ConversionFailed event for DOT must be emitted"
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

		// Should have processed exactly MaxConversionsPerBlock (5) of the 10 pending.
		assert_eq!(
			PendingConversions::<Test>::count(),
			5,
			"Exactly MaxConversionsPerBlock (5) of 10 must be processed"
		);
	});
}
