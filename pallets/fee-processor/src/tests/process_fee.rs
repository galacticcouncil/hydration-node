use super::mock::*;
use crate::*;
use frame_support::assert_ok;
use frame_support::traits::fungibles::{Inspect, Mutate};
use pallet_currencies::fungibles::FungibleCurrencies;

#[test]
fn hdx_fee_distributes_to_pots_immediately() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 1000 * ONE;
		let staking_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &HDX_STAKING_POT);
		let referrals_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &HDX_REFERRALS_POT);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, amount);
		assert!(result.is_ok());
		// HDX path returns Some with pot account (amount routed through fee-processor pot)
		let (taken, pot_account) = result.unwrap().unwrap();
		assert_eq!(taken, amount);
		assert_eq!(pot_account, FeeProcessor::pot_account_id());

		let staking_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &HDX_STAKING_POT);
		let referrals_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &HDX_REFERRALS_POT);

		// HdxFeeReceivers: 50% to staking, 50% to referrals
		assert_eq!(staking_after - staking_before, 500 * ONE);
		assert_eq!(referrals_after - referrals_before, 500 * ONE);
	});
}

#[test]
fn hdx_fee_fires_callbacks_with_correct_amounts() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 1000 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, amount);

		// HDX path uses HdxFeeReceivers (50/50 split)
		let pre = hdx_pre_deposit_calls();
		assert_eq!(pre.len(), 2);
		assert_eq!(pre[0], (ALICE, 500 * ONE));
		assert_eq!(pre[1], (ALICE, 500 * ONE));

		let post = hdx_deposit_calls();
		assert_eq!(post.len(), 2);
		assert_eq!(post[0], 500 * ONE);
		assert_eq!(post[1], 500 * ONE);

		// Non-HDX FeeReceivers should NOT have been called
		assert!(pre_deposit_calls().is_empty());
		assert!(deposit_calls().is_empty());
	});
}

#[test]
fn non_hdx_fee_goes_to_pot_and_marks_pending() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;
		let pot = FeeProcessor::pot_account_id();

		let dot_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(DOT, &pot);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);
		assert!(result.is_ok());
		let (taken, pot_account) = result.unwrap().unwrap();
		assert_eq!(taken, amount);
		assert_eq!(pot_account, pot);

		let dot_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(DOT, &pot);
		assert_eq!(dot_after - dot_before, amount);

		// Should be marked as pending
		assert!(PendingConversions::<Test>::contains_key(DOT));
	});
}

#[test]
fn non_hdx_fee_fires_pre_deposit_callbacks_with_spot_price_equivalent() {
	ExtBuilder::default().build().execute_with(|| {
		// Price: DOT->HDX = 2/1, so 500 DOT = 1000 HDX equivalent
		set_mock_price(Some(hydra_dx_math::ema::EmaPrice::new(2, 1)));
		let amount = 500 * ONE;

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);

		// Pre-deposit callbacks with trader context (optimistic, before conversion)
		let pre = pre_deposit_calls();
		// HDX equivalent = 500 * 2/1 = 1000
		// Staking: 70% of 1000 = 700
		// Referrals: 30% of 1000 = 300
		assert_eq!(pre.len(), 2);
		assert_eq!(pre[0], (ALICE, 700 * ONE));
		assert_eq!(pre[1], (ALICE, 300 * ONE));

		// No post-deposit callbacks yet (conversion hasn't happened)
		assert!(deposit_calls().is_empty());
	});
}

#[test]
fn lrna_fee_is_skipped() {
	ExtBuilder::default().build().execute_with(|| {
		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, LRNA, 500 * ONE);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), None);

		// No callbacks
		assert!(pre_deposit_calls().is_empty());
		assert!(deposit_calls().is_empty());
		// No pending
		assert!(!PendingConversions::<Test>::contains_key(LRNA));
	});
}

#[test]
fn process_trade_fee_succeeds_with_zero_hdx_equivalent_when_price_not_available() {
	ExtBuilder::default().build().execute_with(|| {
		set_mock_price(None);

		let result = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, 500 * ONE);
		// Should succeed — fee is accumulated, just no optimistic callback
		assert!(result.is_ok());

		// Should still be pending for conversion
		assert!(PendingConversions::<Test>::contains_key(DOT));

		// No callbacks fired (hdx_equivalent was 0)
		assert!(pre_deposit_calls().is_empty());
		assert!(deposit_calls().is_empty());

		// Event emitted with zero hdx_equivalent
		System::assert_has_event(
			Event::FeeReceived {
				asset: DOT,
				amount: 500 * ONE,
				hdx_equivalent: 0,
				trader: Some(ALICE),
			}
			.into(),
		);
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
				hdx_equivalent: amount,
				trader: Some(ALICE),
			}
			.into(),
		);
	});
}

#[test]
fn deposit_callbacks_fire_after_conversion() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 500 * ONE;
		let pot = FeeProcessor::pot_account_id();

		// Process non-HDX fee — only pre-deposit fires
		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, DOT, amount);
		assert_eq!(pre_deposit_calls().len(), 2);
		assert!(deposit_calls().is_empty());

		// Mock: convert returns 1000 HDX
		set_convert_result(Some(1000 * ONE));
		// Fund pot with HDX for distribution
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		// Trigger conversion
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));

		// Now deposit callbacks should have fired
		let post = deposit_calls();
		assert_eq!(post.len(), 2);
		assert_eq!(post[0], 700 * ONE); // 70% of 1000
		assert_eq!(post[1], 300 * ONE); // 30% of 1000
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
				hdx_equivalent: 1000 * ONE,
				trader: Some(ALICE),
			}
			.into(),
		);
	});
}

#[test]
fn hdx_and_non_hdx_use_different_receivers() {
	ExtBuilder::default().build().execute_with(|| {
		let pot = FeeProcessor::pot_account_id();

		// --- HDX fee: uses HdxFeeReceivers (50/50) ---
		let hdx_amount = 1000 * ONE;
		let staking_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT);
		let referrals_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &REFERRALS_POT);

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, hdx_amount);

		let staking_after_hdx = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &STAKING_POT);
		let referrals_after_hdx = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &REFERRALS_POT);

		// HDX path: 50/50 via HdxFeeReceivers
		assert_eq!(staking_after_hdx - staking_before, 500 * ONE);
		assert_eq!(referrals_after_hdx - referrals_before, 500 * ONE);

		// HDX callbacks fired, non-HDX callbacks did NOT fire
		assert_eq!(hdx_pre_deposit_calls().len(), 2);
		assert!(pre_deposit_calls().is_empty());

		// --- Non-HDX fee (conversion path): uses FeeReceivers (70/30) ---
		set_convert_result(Some(1000 * ONE));
		// Fund pot with HDX for distribution after conversion
		<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &pot, 1000 * ONE).unwrap();

		let _ = Pallet::<Test>::process_trade_fee(FEE_SOURCE, BOB, DOT, 500 * ONE);

		// Non-HDX pre-deposit callbacks use FeeReceivers (70/30)
		let pre = pre_deposit_calls();
		assert_eq!(pre.len(), 2);
		assert_eq!(pre[0], (BOB, 700 * ONE)); // 70% of 1000 HDX equivalent
		assert_eq!(pre[1], (BOB, 300 * ONE)); // 30% of 1000 HDX equivalent

		// Trigger conversion — distribution uses FeeReceivers (70/30)
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE), DOT));
		let post = deposit_calls();
		assert_eq!(post.len(), 2);
		assert_eq!(post[0], 700 * ONE);
		assert_eq!(post[1], 300 * ONE);
	});
}
