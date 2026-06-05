use super::mock::*;
use crate::*;
use frame_support::assert_ok;
use frame_support::traits::fungibles::{Inspect, Mutate};
use pallet_currencies::fungibles::FungibleCurrencies;

fn balance(asset: AssetId, who: &AccountId) -> u128 {
	<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(asset, who)
}

#[test]
fn hdx_fee_distributes_to_pots_immediately() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 1000 * ONE;
		let staking_before = balance(HDX, &HDX_STAKING_POT);
		let referrals_before = balance(HDX, &HDX_REFERRALS_POT);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, amount);
		assert!(result.is_ok());
		let (taken, pot_account) = result.unwrap().unwrap();
		assert_eq!(taken, amount);
		assert_eq!(pot_account, FeeProcessor::pot_account_id());

		// HdxFeeReceivers: 50% to staking (HDX-target), 50% to referrals (raw — gets raw HDX).
		assert_eq!(balance(HDX, &HDX_STAKING_POT) - staking_before, 500 * ONE);
		assert_eq!(balance(HDX, &HDX_REFERRALS_POT) - referrals_before, 500 * ONE);
	});
}

#[test]
fn hdx_fee_fires_raw_callback_for_referrals_only() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 1000 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, amount);

		// HDX path uses HdxFeeReceivers; only the raw referrals receiver is notified, with its 50% slice.
		let raw = hdx_raw_fee_calls();
		assert_eq!(raw.len(), 1);
		assert_eq!(raw[0], (ALICE, HDX, 500 * ONE));

		// Non-HDX FeeReceivers raw callback should NOT have been called.
		assert!(raw_fee_calls().is_empty());
	});
}

#[test]
fn non_hdx_fee_routes_convert_slice_to_pot_and_raw_slice_to_referrals() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;
		let pot = FeeProcessor::pot_account_id();

		let pot_before = balance(DOT, &pot);
		let referrals_before = balance(DOT, &REFERRALS_POT);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);
		assert!(result.is_ok());
		let (taken, pot_account) = result.unwrap().unwrap();
		assert_eq!(taken, amount);
		assert_eq!(pot_account, pot);

		// Convert receiver (staking 70%) slice goes to the pot; raw receiver (referrals 30%) gets raw DOT.
		assert_eq!(balance(DOT, &pot) - pot_before, 350 * ONE);
		assert_eq!(balance(DOT, &REFERRALS_POT) - referrals_before, 150 * ONE);

		// Convert slice marked pending.
		assert!(PendingConversions::<Test>::contains_key(DOT));
	});
}

#[test]
fn raw_receiver_partial_take_leaves_remainder_with_source() {
	// When a raw receiver consumes less than its offered slice, only the used amount
	// is taken from `source`; the remainder is never moved.
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;
		let pot = FeeProcessor::pot_account_id();
		// Referrals is offered 30% (150) but only consumes 40.
		set_raw_fee_used(Some(40 * ONE));

		let source_before = balance(DOT, &FEE_SOURCE);
		let referrals_before = balance(DOT, &REFERRALS_POT);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);
		let (taken, _) = result.unwrap().unwrap();

		// Referrals received only the 40 it consumed (not the full 150 slice).
		assert_eq!(balance(DOT, &REFERRALS_POT) - referrals_before, 40 * ONE);
		// Staking convert slice (70% = 350) still goes to the pot.
		assert_eq!(balance(DOT, &pot), 350 * ONE);
		// total taken = 40 (raw used) + 350 (convert) = 390; the unused 110 stays with source.
		assert_eq!(taken, 390 * ONE);
		assert_eq!(source_before - balance(DOT, &FEE_SOURCE), 390 * ONE);
	});
}

#[test]
fn non_hdx_fee_fires_raw_callback_with_raw_slice() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);

		// Referrals (raw, 30%) is notified with its raw slice of the original asset.
		let raw = raw_fee_calls();
		assert_eq!(raw.len(), 1);
		assert_eq!(raw[0], (ALICE, DOT, 150 * ONE));
	});
}

#[test]
fn lrna_fee_is_skipped() {
	ExtBuilder::default().build().execute_with(|| {
		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, LRNA, 500 * ONE);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), None);

		assert!(raw_fee_calls().is_empty());
		assert!(!PendingConversions::<Test>::contains_key(LRNA));
	});
}

#[test]
fn event_emitted_for_hdx_fee() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 1000 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, amount);

		System::assert_has_event(
			Event::FeeReceived {
				asset: HDX,
				amount,
				trader: Some(ALICE),
			}
			.into(),
		);
	});
}

#[test]
fn event_emitted_for_non_hdx_fee() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);

		System::assert_has_event(
			Event::FeeReceived {
				asset: DOT,
				amount,
				trader: Some(ALICE),
			}
			.into(),
		);
	});
}

#[test]
fn converted_hdx_is_distributed_to_convert_receivers_only() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;
		let pot = FeeProcessor::pot_account_id();

		// Process non-HDX fee — convert slice (350 DOT) sits in the pot, pending.
		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);
		assert!(PendingConversions::<Test>::contains_key(DOT));

		// Mock: convert returns 1000 HDX; fund pot with the swap output for distribution.
		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		let staking_before = balance(HDX, &STAKING_POT);

		// Trigger conversion. Only the convert receivers (staking, 70%) share the HDX;
		// the raw referrals receiver was already paid in DOT.
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		assert_eq!(balance(HDX, &STAKING_POT) - staking_before, 1000 * ONE);
	});
}

#[test]
fn process_trade_fee_same_asset_twice_does_not_duplicate_pending_count() {
	// PendingConversions is a `CountedStorageMap`; inserting the same key twice
	// must not bump the count beyond 1.
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, 100 * ONE));
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, 200 * ONE));

		assert!(PendingConversions::<Test>::contains_key(DOT));
		assert_eq!(
			PendingConversions::<Test>::count(),
			1,
			"Same asset processed twice must yield count == 1"
		);

		// Pot accumulates only the convert (staking 70%) slices: 70 + 140 = 210.
		let pot = FeeProcessor::pot_account_id();
		assert_eq!(balance(DOT, &pot), 210 * ONE);
	});
}

#[test]
fn process_trade_fee_nothing_changes_when_raw_callback_fails() {
	// The pallet's hook is called from a dispatch context (transactional).
	// We emulate that with `with_transaction` + Rollback-on-Err to confirm that
	// when the raw-receiver callback errors, the raw transfer to the referrals pot
	// is rolled back too — no funds get stuck.
	use frame_support::storage::with_transaction;
	use sp_runtime::TransactionOutcome;

	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();
		let source_dot_before = balance(DOT, &FEE_SOURCE);
		let pot_dot_before = balance(DOT, &pot);
		let referrals_dot_before = balance(DOT, &REFERRALS_POT);

		set_raw_fee_should_fail(true);

		let result = with_transaction::<(), sp_runtime::DispatchError, _>(|| {
			match Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, 500 * ONE) {
				Ok(_) => TransactionOutcome::Commit(Ok(())),
				Err(e) => TransactionOutcome::Rollback(Err(e)),
			}
		});
		assert!(result.is_err(), "process_trade_fee must propagate the callback error");

		// Balances unchanged.
		assert_eq!(
			balance(DOT, &FEE_SOURCE),
			source_dot_before,
			"source DOT must not have moved"
		);
		assert_eq!(balance(DOT, &pot), pot_dot_before, "pot must not have received DOT");
		assert_eq!(
			balance(DOT, &REFERRALS_POT),
			referrals_dot_before,
			"referrals pot must not have received DOT"
		);

		// PendingConversions not inserted.
		assert!(!PendingConversions::<Test>::contains_key(DOT));
	});
}

#[test]
fn hdx_and_non_hdx_use_different_receivers() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// --- HDX fee: uses HdxFeeReceivers (50/50) ---
		let hdx_amount = 1000 * ONE;
		let staking_before = balance(HDX, &STAKING_POT);
		let referrals_before = balance(HDX, &REFERRALS_POT);

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, hdx_amount);

		assert_eq!(balance(HDX, &STAKING_POT) - staking_before, 500 * ONE);
		assert_eq!(balance(HDX, &REFERRALS_POT) - referrals_before, 500 * ONE);

		// HDX raw callback fired, non-HDX raw callback did NOT fire.
		assert_eq!(hdx_raw_fee_calls().len(), 1);
		assert!(raw_fee_calls().is_empty());

		// --- Non-HDX fee (conversion path): uses FeeReceivers (70 convert / 30 raw) ---
		set_convert_result(Some(1000 * ONE));
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		let staking_hdx_before = balance(HDX, &STAKING_POT);

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, BOB, DOT, 500 * ONE);

		// Non-HDX raw callback uses FeeReceivers' referrals (30%).
		let raw = raw_fee_calls();
		assert_eq!(raw.len(), 1);
		assert_eq!(raw[0], (BOB, DOT, 150 * ONE));

		// Trigger conversion — distribution goes to the convert receiver (staking 70% → all of swap output).
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));
		assert_eq!(balance(HDX, &STAKING_POT) - staking_hdx_before, 1000 * ONE);
	});
}
