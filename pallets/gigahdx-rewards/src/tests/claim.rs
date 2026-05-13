// SPDX-License-Identifier: Apache-2.0

use super::mock::*;
use crate::pallet::{Error, Event, PendingRewards};

use frame_support::{assert_err, assert_noop, assert_ok};
use frame_system::RawOrigin;

fn seed_pending_reward(who: AccountId, amount: u128) {
	// Mint HDX into the allocated pot and credit `PendingRewards[who]`
	// directly — this short-circuits the full vote → remove_vote flow.
	use frame_support::traits::Currency;
	let _ = <Balances as Currency<AccountId>>::deposit_creating(&allocated_pot(), amount);
	PendingRewards::<Test>::insert(who, amount);
}

#[test]
fn claim_rewards_should_compound_pending_into_gigahdx() {
	ExtBuilder::default().build().execute_with(|| {
		use frame_system::RawOrigin;
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));

		let stake_before = pallet_gigahdx::Stakes::<Test>::get(ALICE).unwrap();
		let alloc_before = account_balance(&allocated_pot());
		seed_pending_reward(ALICE, 10 * ONE);
		assert_eq!(account_balance(&allocated_pot()), alloc_before + 10 * ONE);

		assert_ok!(GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()));

		// PendingRewards drained.
		assert_eq!(PendingRewards::<Test>::get(ALICE), 0);
		// Stake compounded.
		let stake_after = pallet_gigahdx::Stakes::<Test>::get(ALICE).unwrap();
		assert_eq!(stake_after.hdx, stake_before.hdx + 10 * ONE);
		assert!(stake_after.gigahdx > stake_before.gigahdx);
	});
}

#[test]
fn claim_rewards_should_fail_when_no_pending() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()),
			Error::<Test>::NoPendingRewards
		);
	});
}

#[test]
fn claim_rewards_should_revert_and_preserve_pending_when_conversion_rounds_to_zero() {
	ExtBuilder::default()
		.with_pot_balance(1_000 * ONE) // heavy gigapot → rate >> 1
		.build()
		.execute_with(|| {
			// Build up an appreciated rate: tiny stake against an inflated gigapot.
			assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
			// Rate ≈ (100 + 1000) / 100 = 11 HDX per gigahdx.

			// Seed a `PendingRewards` of `10` (in raw units), below the rate floor.
			seed_pending_reward(ALICE, 10u128);

			assert_err!(
				GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()),
				pallet_gigahdx::Error::<Test>::ZeroAmount
			);

			// Transactional revert: pending intact for retry.
			assert_eq!(PendingRewards::<Test>::get(ALICE), 10u128);
		});
}

#[test]
fn claim_rewards_should_revert_and_preserve_pending_when_money_market_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		seed_pending_reward(ALICE, 10 * ONE);

		TestMoneyMarket::fail_supply();

		assert_err!(
			GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()),
			pallet_gigahdx::Error::<Test>::MoneyMarketSupplyFailed
		);

		// Pending preserved; allocated pot balance preserved by transactional revert.
		assert_eq!(PendingRewards::<Test>::get(ALICE), 10 * ONE);
	});
}

#[test]
fn claim_rewards_should_fail_when_caller_has_external_claim() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		seed_pending_reward(ALICE, 10 * ONE);

		// Simulate the caller holding HDX claimed by another pallet (legacy
		// staking lock, vesting, etc.). `claim_rewards` must refuse to compound
		// because FRAME's max-of-locks semantics would let the larger external
		// lock shadow the freshly-applied gigahdx lock.
		TestExternalClaims::set(50 * ONE);

		assert_noop!(
			GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()),
			pallet_gigahdx::Error::<Test>::BlockedByExternalLock
		);

		// `PendingRewards` untouched — guard runs before `take`, no rewards burned.
		assert_eq!(PendingRewards::<Test>::get(ALICE), 10 * ONE);

		// After the external claim clears, the same caller can compound.
		TestExternalClaims::reset();
		assert_ok!(GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()));
		assert_eq!(PendingRewards::<Test>::get(ALICE), 0);
	});
}

#[test]
fn claim_rewards_should_emit_event_with_gigahdx_received() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), 100 * ONE));
		seed_pending_reward(ALICE, 10 * ONE);

		assert_ok!(GigaHdxRewards::claim_rewards(RawOrigin::Signed(ALICE).into()));

		let recent = last_events(10);
		let found = recent.iter().any(|e| {
			matches!(
				e,
				RuntimeEvent::GigaHdxRewards(Event::RewardsClaimed { who, total_hdx, gigahdx_received })
					if *who == ALICE && *total_hdx == 10 * ONE && *gigahdx_received > 0
			)
		});
		assert!(found, "expected RewardsClaimed event; got {recent:?}");
	});
}
