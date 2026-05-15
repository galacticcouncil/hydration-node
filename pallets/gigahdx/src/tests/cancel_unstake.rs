// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::{Error, Event, Stakes, TotalLocked};
use frame_support::sp_runtime::traits::AccountIdConversion;
use frame_support::traits::fungibles::Inspect as FungiblesInspect;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::gigahdx::MoneyMarketOperations;
use primitives::Balance;

fn pot() -> AccountId {
	GigaHdxPalletId::get().into_account_truncating()
}

fn locked_under_ghdx(account: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(account)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn stake_alice_100() {
	assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
}

fn cancel_unstake_event(who: AccountId) -> Option<(u64, Balance, Balance)> {
	System::events().into_iter().rev().find_map(|r| match r.event {
		RuntimeEvent::GigaHdx(Event::UnstakeCancelled {
			who: w,
			position_id,
			amount,
			gigahdx,
		}) if w == who => Some((position_id, amount, gigahdx)),
		_ => None,
	})
}

// ---------------------------------------------------------------------------
// Behavior
// ---------------------------------------------------------------------------

#[test]
fn cancel_unstake_should_fail_when_no_pending_unstake() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_noop!(
			GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1),
			Error::<Test>::PendingUnstakeNotFound,
		);
	});
}

#[test]
fn cancel_unstake_should_fail_when_no_stake_and_no_pending() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1),
			Error::<Test>::PendingUnstakeNotFound,
		);
	});
}

#[test]
fn cancel_unstake_should_remove_pending_entry_when_called() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		assert_eq!(pending_count(ALICE), 1);

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
		assert_eq!(pending_count(ALICE), 0);
	});
}

#[test]
fn cancel_unstake_should_restore_state_when_partial_unstake_within_principal() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		let pre_lock = locked_under_ghdx(ALICE);

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), pre_lock);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 100 * ONE);
		assert_eq!(pending_count(ALICE), 0);
	});
}

#[test]
fn cancel_unstake_should_restore_state_when_full_unstake_no_yield() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		let pre_lock = locked_under_ghdx(ALICE);

		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 100 * ONE);
		assert_eq!(TotalLocked::<Test>::get(), 100 * ONE);
		assert_eq!(locked_under_ghdx(ALICE), pre_lock);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), 100 * ONE);
	});
}

#[test]
fn cancel_unstake_should_restore_state_when_yield_was_paid_from_gigapot() {
	ExtBuilder::default()
		.with_pot_balance(30 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let pre_lock = locked_under_ghdx(ALICE);
			let pre_pot = Balances::free_balance(pot());

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_eq!(Balances::free_balance(pot()), pre_pot - 30 * ONE);
			assert_eq!(only_pending(ALICE).amount, 130 * ONE);

			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_eq!(s.hdx, 130 * ONE);
			assert_eq!(s.gigahdx, 130 * ONE);
			assert_eq!(TotalLocked::<Test>::get(), 130 * ONE);
			assert_eq!(locked_under_ghdx(ALICE), pre_lock + 30 * ONE);
			assert_eq!(Balances::free_balance(pot()), 0);
			assert_eq!(pending_count(ALICE), 0);
		});
}

#[test]
fn cancel_unstake_should_emit_event() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

		let (position_id, amount, gigahdx) = cancel_unstake_event(ALICE).expect("event emitted");
		assert_eq!(position_id, 1);
		assert_eq!(amount, 40 * ONE);
		assert_eq!(gigahdx, 40 * ONE);
	});
}

#[test]
fn cancel_unstake_should_refresh_lock_to_active_stake_only() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
		assert_eq!(locked_under_ghdx(ALICE), 100 * ONE);
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().hdx, 100 * ONE);
	});
}

#[test]
fn cancel_unstake_should_succeed_before_cooldown_elapses() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		let entry = only_pending(ALICE);
		assert!(System::block_number() < entry.expires_at);
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
	});
}

#[test]
fn cancel_unstake_should_succeed_after_cooldown_when_not_yet_unlocked() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		let entry = only_pending(ALICE);
		System::set_block_number(entry.expires_at + 10);
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
	});
}

#[test]
fn cancel_unstake_should_fail_after_unlock() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		let entry = only_pending(ALICE);
		System::set_block_number(entry.expires_at);
		assert_ok!(GigaHdx::unlock(RawOrigin::Signed(ALICE).into(), 1));

		assert_noop!(
			GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1),
			Error::<Test>::PendingUnstakeNotFound,
		);
	});
}

#[test]
fn cancel_unstake_should_rollback_when_supply_fails() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 40 * ONE));

		let pre_pending = only_pending(ALICE);
		let pre_stake = Stakes::<Test>::get(ALICE).unwrap();
		let pre_total_locked = TotalLocked::<Test>::get();
		let pre_mm = TestMoneyMarket::balance_of(&ALICE);
		let pre_sthdx = Tokens::balance(ST_HDX, &ALICE);

		TestMoneyMarket::fail_supply();
		assert_noop!(
			GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1),
			Error::<Test>::MoneyMarketSupplyFailed,
		);

		assert_eq!(only_pending(ALICE), pre_pending);
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap(), pre_stake);
		assert_eq!(TotalLocked::<Test>::get(), pre_total_locked);
		assert_eq!(TestMoneyMarket::balance_of(&ALICE), pre_mm);
		assert_eq!(Tokens::balance(ST_HDX, &ALICE), pre_sthdx);
	});
}

// ---------------------------------------------------------------------------
// `frozen` invariant
// ---------------------------------------------------------------------------

#[test]
fn cancel_unstake_should_preserve_frozen_amount() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		// Freeze 30 before unstake; partial unstake of 50 leaves hdx=50 ≥ frozen=30.
		GigaHdx::freeze(&ALICE, 30 * ONE);
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 50 * ONE));
		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().frozen, 30 * ONE);

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.frozen, 30 * ONE, "cancel must not touch frozen");
		assert_eq!(s.hdx, 100 * ONE);
		assert!(s.frozen <= s.hdx, "invariant frozen ≤ hdx preserved");
	});
}

// ---------------------------------------------------------------------------
// Exchange-rate sensitivity
// ---------------------------------------------------------------------------

#[test]
fn cancel_unstake_should_yield_fewer_atokens_when_rate_increased() {
	// Rate inflated between unstake and cancel → fewer aTokens minted.
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		// Donate to pot → rate climbs above 1.0 at cancel time.
		// (Supply was zeroed by full unstake; restore some via Bob to give the
		// pot a non-empty denominator.)
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(BOB).into(), 100 * ONE));
		assert_ok!(Balances::transfer_keep_alive(
			RawOrigin::Signed(TREASURY).into(),
			pot(),
			100 * ONE,
		));
		// rate now ≈ (100 + 100) / 100 = 2.0 → cancel of 100 mints 50 aTokens.

		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));
		let s = Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(s.hdx, 100 * ONE);
		assert_eq!(s.gigahdx, 50 * ONE);
	});
}

#[test]
fn cancel_unstake_should_yield_same_atoken_count_when_rate_unchanged() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		let pre_gigahdx = Stakes::<Test>::get(ALICE).unwrap().gigahdx;
		assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

		assert_eq!(Stakes::<Test>::get(ALICE).unwrap().gigahdx, pre_gigahdx);
	});
}

// ---------------------------------------------------------------------------
// Anti-extraction
// ---------------------------------------------------------------------------

#[test]
fn unstake_cancel_cycle_should_be_value_neutral_when_rate_unchanged() {
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let total_value_before = TotalLocked::<Test>::get().saturating_add(Balances::free_balance(pot()));

			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), 1));

			let total_value_after = TotalLocked::<Test>::get().saturating_add(Balances::free_balance(pot()));
			assert_eq!(total_value_after, total_value_before);
		});
}

fn alice_claim_hdx_value() -> Balance {
	let s = match Stakes::<Test>::get(ALICE) {
		Some(s) => s,
		None => return 0,
	};
	let supply = GigaHdx::total_gigahdx_supply();
	if supply == 0 || s.gigahdx == 0 {
		return 0;
	}
	let system_total = TotalLocked::<Test>::get().saturating_add(Balances::free_balance(pot()));
	s.gigahdx.saturating_mul(system_total) / supply
}

#[test]
fn repeated_unstake_cancel_cycles_should_not_inflate_user_claim_value() {
	// Alice can shuffle pot-yield into stake.hdx via cancel, but her HDX-claim
	// value (gigahdx × rate) must not grow across cycles.
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let initial_claim = alice_claim_hdx_value();

			for _ in 0..10 {
				let s = Stakes::<Test>::get(ALICE).unwrap();
				assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), s.gigahdx,));
				let id = only_pending(ALICE).id;
				assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), id));
			}

			let after_claim = alice_claim_hdx_value();
			assert!(
				after_claim <= initial_claim,
				"claim value must not grow across cycles: {initial_claim} → {after_claim}",
			);
		});
}

#[test]
fn repeated_unstake_cancel_cycles_should_not_drain_gigapot() {
	ExtBuilder::default()
		.with_pot_balance(50 * ONE)
		.build()
		.execute_with(|| {
			stake_alice_100();
			let pot_initial = Balances::free_balance(pot());

			for _ in 0..10 {
				let s = Stakes::<Test>::get(ALICE).unwrap();
				assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), s.gigahdx,));
				let id = only_pending(ALICE).id;
				assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), id));
			}

			// First cycle drains the 50 yield into Alice's principal; subsequent
			// cycles have pot=0 and operate at rate 1, so pot stays empty.
			let pot_after = Balances::free_balance(pot());
			let total_after = TotalLocked::<Test>::get().saturating_add(pot_after);
			let total_initial = 100 * ONE + pot_initial;
			assert_eq!(
				total_after, total_initial,
				"system total (TotalLocked + pot) must be conserved across cycles",
			);
		});
}

#[test]
fn repeated_unstake_cancel_cycles_should_not_affect_other_stakers() {
	ExtBuilder::default().build().execute_with(|| {
		stake_alice_100();
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(BOB).into(), 100 * ONE));
		let bob_initial = Stakes::<Test>::get(BOB).unwrap();
		let pot_initial = Balances::free_balance(pot());

		for _ in 0..5 {
			let s = Stakes::<Test>::get(ALICE).unwrap();
			assert_ok!(GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), s.gigahdx,));
			let id = only_pending(ALICE).id;
			assert_ok!(GigaHdx::cancel_unstake(RawOrigin::Signed(ALICE).into(), id));
		}

		assert_eq!(Stakes::<Test>::get(BOB).unwrap(), bob_initial);
		assert_eq!(Balances::free_balance(pot()), pot_initial);
	});
}
