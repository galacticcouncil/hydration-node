use super::mock::*;
use crate::*;
use frame_support::assert_ok;
use frame_support::traits::fungibles::{Inspect, Mutate};
use frame_support::traits::tokens::Preservation;
use pallet_currencies::fungibles::FungibleCurrencies;

fn balance(asset: AssetId, who: &AccountId) -> u128 {
	<FungibleCurrencies<Test> as Inspect<AccountId>>::balance(asset, who)
}

// Empty an account (reaped) so it starts below ED.
fn drain(asset: AssetId, who: &AccountId) {
	let bal = balance(asset, who);
	if bal > 0 {
		assert_ok!(<FungibleCurrencies<Test> as Mutate<AccountId>>::transfer(
			asset,
			who,
			&BOB,
			bal,
			Preservation::Expendable
		));
	}
}

// HDX path: HdxStakingFeeReceiver (50%, HDX-target, hold_until_ed) -> HDX_STAKING_POT,
// HdxReferralsFeeReceiver (50%, raw) -> HDX_REFERRALS_POT. With one non-raw receiver
// the pot `take` equals that receiver's slice, so the numbers stay clean. Genesis
// funds HDX_STAKING_POT / HDX_REFERRALS_POT / pot with ONE each.

#[test]
fn hdx_fee_should_hold_staking_slice_when_pot_below_ed() {
	ExtBuilder::default().build().execute_with(|| {
		set_hdx_existential_deposit(ONE);
		drain(HDX, &HDX_STAKING_POT); // receiver below ED

		let pot = FeeProcessor::pot_account_id();
		let pot_before = balance(HDX, &pot);

		// Staking slice = ONE/2 < ED -> held, not delivered.
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, ONE));

		assert_eq!(balance(HDX, &HDX_STAKING_POT), 0, "nothing delivered to the receiver");
		assert_eq!(HeldFees::<Test>::get(HDX_STAKING_POT), ONE / 2, "slice earmarked");
		// The held HDX physically sits in the pot.
		assert_eq!(balance(HDX, &pot), pot_before + ONE / 2);
	});
}

#[test]
fn hdx_fee_should_flush_staking_buffer_when_accumulation_reaches_ed() {
	ExtBuilder::default().build().execute_with(|| {
		set_hdx_existential_deposit(ONE);
		drain(HDX, &HDX_STAKING_POT);

		// First trade: ONE/2 held.
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, ONE));
		assert_eq!(balance(HDX, &HDX_STAKING_POT), 0);
		assert_eq!(HeldFees::<Test>::get(HDX_STAKING_POT), ONE / 2);

		// Second trade: held(ONE/2) + slice(ONE/2) = ONE >= ED -> flush.
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, ONE));
		assert_eq!(balance(HDX, &HDX_STAKING_POT), ONE, "buffer flushed to the receiver");
		assert_eq!(HeldFees::<Test>::get(HDX_STAKING_POT), 0, "buffer cleared");
	});
}

#[test]
fn hdx_fee_should_deliver_immediately_when_pot_already_above_ed() {
	ExtBuilder::default().build().execute_with(|| {
		set_hdx_existential_deposit(ONE);
		// Receiver already at/above ED (genesis ONE) -> even a sub-ED slice flows straight through.
		let before = balance(HDX, &HDX_STAKING_POT);

		// amount = 2 -> staking slice = 1 (< ED).
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, 2));

		assert_eq!(balance(HDX, &HDX_STAKING_POT), before + 1);
		assert_eq!(HeldFees::<Test>::get(HDX_STAKING_POT), 0);
	});
}

#[test]
fn hdx_fee_should_revert_below_ed_when_hold_until_ed_disabled() {
	ExtBuilder::default().build().execute_with(|| {
		set_hdx_existential_deposit(ONE);
		set_hdx_staking_hold(false);
		drain(HDX, &HDX_STAKING_POT);

		// Without buffering, the sub-ED slice is transferred straight into the empty
		// receiver and reverts — the original failure mode this feature removes.
		let err = Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, ONE).unwrap_err();
		assert_eq!(err, sp_runtime::TokenError::BelowMinimum.into());
	});
}

#[test]
fn hdx_fee_should_not_revert_when_pot_account_starts_empty() {
	ExtBuilder::default().build().execute_with(|| {
		set_hdx_existential_deposit(ONE);
		let pot = FeeProcessor::pot_account_id();
		drain(HDX, &pot); // reaped pot mimics a fresh chain
		assert_eq!(balance(HDX, &pot), 0);

		// `ensure_pot_exists` provider-backs the reaped pot so distributing the full
		// `take` through it does not revert with FundsUnavailable. amount = 2*ONE so
		// take = ONE (>= ED) is deposited and the slice is delivered to the receiver.
		let before = balance(HDX, &HDX_STAKING_POT);
		assert_ok!(Pallet::<Test>::process_trade_fee(FEE_SOURCE, ALICE, HDX, 2 * ONE));
		assert_eq!(balance(HDX, &HDX_STAKING_POT), before + ONE, "slice delivered");
		assert!(
			frame_system::Pallet::<Test>::providers(&pot) >= 1,
			"pot kept alive by ensure_pot_exists",
		);
	});
}
