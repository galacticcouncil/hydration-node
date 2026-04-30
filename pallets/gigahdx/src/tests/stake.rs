use super::mock::*;
use crate::{Error, Event};
use frame_support::{
	assert_noop, assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::{traits::One, FixedU128};

#[test]
fn giga_stake_should_mint_gigahdx_when_amount_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = 100 * ONE;

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), amount));

		let gigapot = GigaHdx::gigapot_account_id();

		// HDX transferred to gigapot
		assert_eq!(<Test as crate::Config>::Currency::balance(HDX, &gigapot), amount);

		// stHDX minted to user (1:1 at initial rate).
		// With no-op MoneyMarket, supply() is identity — stHDX stays with user.
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), amount);

		// Exchange rate is still 1:1
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

		// Check event
		System::assert_last_event(
			Event::Staked {
				who: ALICE,
				hdx_amount: amount,
				st_hdx_minted: amount,
				gigahdx_received: amount,
				exchange_rate: FixedU128::one(),
			}
			.into(),
		);
	});
}

#[test]
fn giga_stake_should_fail_when_amount_below_min_stake() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = ONE / 2; // Below MinStake of 1 ONE

		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), amount),
			Error::<Test>::InsufficientStake
		);
	});
}

#[test]
fn giga_stake_should_fail_when_amount_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 0),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn giga_stake_should_fail_when_balance_is_insufficient() {
	ExtBuilder::default().build().execute_with(|| {
		// ALICE has 1000 ONE, try to stake 2000 ONE
		assert!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 2_000 * ONE).is_err());
	});
}

#[test]
fn giga_stake_should_mint_proportional_gigahdx_when_rate_increased() {
	ExtBuilder::default().build().execute_with(|| {
		// First stake: 100 HDX at 1:1
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Simulate fee accrual: add 100 HDX to gigapot directly
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// Exchange rate is now 200 HDX / 100 stHDX = 2.0
		let rate = GigaHdx::exchange_rate();
		assert_eq!(rate, FixedU128::from(2));

		// Second stake: 100 HDX should get 50 stHDX (100 * 100 / 200 = 50)
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 100 * ONE));

		System::assert_last_event(
			Event::Staked {
				who: BOB,
				hdx_amount: 100 * ONE,
				st_hdx_minted: 50 * ONE,
				gigahdx_received: 50 * ONE,
				exchange_rate: GigaHdx::exchange_rate(),
			}
			.into(),
		);
	});
}

#[test]
fn giga_stake_should_track_balances_independently_when_multiple_users_stake() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 200 * ONE));

		let gigapot = GigaHdx::gigapot_account_id();

		// Gigapot has 300 HDX
		assert_eq!(<Test as crate::Config>::Currency::balance(HDX, &gigapot), 300 * ONE);

		// Total stHDX supply is 300 ONE (1:1 rate)
		assert_eq!(GigaHdx::total_st_hdx_supply(), 300 * ONE);

		// Exchange rate unchanged
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

// direct HDX donation to the gigapot is treated as a fee accrual:
// exchange rate goes up, no panic, no integer wrap, existing stakers benefit.
#[test]
fn exchange_rate_should_inflate_safely_when_hdx_donated_to_gigapot() {
	ExtBuilder::default()
		.with_endowed(vec![
			(ALICE, HDX, 1_000 * ONE),
			(BOB, HDX, 1_000_000 * ONE), // wealthy attacker
		])
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
			assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

			// Attacker BOB sends HDX directly to gigapot via the standard
			// `Currency::transfer` — no special privilege needed.
			let gigapot = GigaHdx::gigapot_account_id();
			assert_ok!(<Test as crate::Config>::Currency::transfer(
				HDX,
				&BOB,
				&gigapot,
				900 * ONE,
				frame_support::traits::tokens::Preservation::Expendable,
			));

			// Rate should now reflect 1000 HDX / 100 stHDX = 10x.
			assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(10));

			// Existing staker can still unstake and gets the inflated payout.
			assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));
			let positions = GigaHdx::unstake_positions(&ALICE);
			assert_eq!(positions.len(), 1);
			assert_eq!(positions[0].amount, 1_000 * ONE, "unstaker receives all donated HDX");
		});
}

// donation BEFORE first stake: exchange_rate falls back to 1.0 because
// total_st_hdx is zero. The donor can never recover their HDX (it's a true
// burn into the treasury) and the first staker gets the donation as a bonus.
#[test]
fn donated_hdx_should_be_irrecoverable_when_donated_before_first_stake() {
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (BOB, HDX, 1_000 * ONE)])
		.build()
		.execute_with(|| {
			let gigapot = GigaHdx::gigapot_account_id();
			assert_ok!(<Test as crate::Config>::Currency::transfer(
				HDX,
				&BOB,
				&gigapot,
				500 * ONE,
				frame_support::traits::tokens::Preservation::Expendable,
			));

			// No stakers yet → rate is the safe fallback 1.0.
			assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

			// First staker is treated at the bootstrap branch (rate ignored when
			// total_st_hdx is zero). They get 1:1 stHDX for their HDX, donation
			// stays in the gigapot.
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
			assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), 100 * ONE);

			// Now exchange rate jumps because gigapot has the donation + stake
			// (600 HDX) but only 100 stHDX exists.
			assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(6));
		});
}

// vault inflation attack: attacker stakes the minimum, then donates to
// inflate the rate so a victim's stake rounds down. We verify two halves of
// the bound:
//   (a) For the attack to silently zero out a victim's mint, the donation
//       must satisfy: victim_stake * total_st_hdx < total_hdx, i.e. the
//       attacker needs roughly `victim_stake * MinStake` base units of HDX to
//       grief a victim of size `victim_stake`. With MinStake = ONE = 10^12,
//       griefing a victim staking ONE requires donating > ONE * ONE = 10^24
//       base units, i.e. 10^12 ONE. Attacker simply cannot afford this within
//       any realistic balance, so the attack is economically infeasible.
//   (b) Even if the attacker could afford it, the pallet's
//       `ensure!(!st_hdx_amount.is_zero())` returns `ZeroAmount` and the
//       victim's HDX is never silently transferred away.
#[test]
fn donation_should_not_steal_victim_funds_when_vault_inflated() {
	// Half (a): realistic attacker cannot afford the donation.
	ExtBuilder::default()
		.with_endowed(vec![
			(ALICE, HDX, 1_000_000 * ONE), // wealthy attacker (1 million HDX)
			(BOB, HDX, 1_000 * ONE),       // victim
		])
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), ONE));

			let gigapot = GigaHdx::gigapot_account_id();
			// Attacker donates the entire wallet. Even with 10^6 ONE = 10^18 base
			// units in the gigapot, victim staking ONE gets:
			//   ONE * ONE / 10^18 = 10^6 base units = nonzero.
			assert_ok!(<Test as crate::Config>::Currency::transfer(
				HDX,
				&ALICE,
				&gigapot,
				1_000_000 * ONE - ONE,
				frame_support::traits::tokens::Preservation::Expendable,
			));

			// Victim CAN still stake — they get a tiny but nonzero amount of stHDX.
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), ONE));
			let bob_st = <Test as crate::Config>::Currency::balance(ST_HDX, &BOB);
			assert!(
				bob_st > 0,
				"victim mint must be nonzero — donation too small to round to 0"
			);
			// Concretely: ONE * ONE / (~10^6 ONE) ≈ 10^6 base units (exact value
			// depends on rounding direction; we only care that it's nonzero and
			// well under MinStake).
			assert!(bob_st < ONE, "rate is heavily inflated, victim mint must be tiny");
		});

	// Half (b): synthesize the silent-mint scenario by directly minting the
	// attacker's hypothetical donation, and verify the pallet still refuses
	// to mint zero stHDX rather than swallowing the victim's HDX.
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 1_000 * ONE), (BOB, HDX, 1_000 * ONE)])
		.build()
		.execute_with(|| {
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), ONE));

			// Synthesize the worst-case donation by minting raw HDX into the
			// gigapot — this stands in for "attacker has unbounded budget".
			let gigapot = GigaHdx::gigapot_account_id();
			assert_ok!(<Test as crate::Config>::Currency::mint_into(
				HDX,
				&gigapot,
				ONE * ONE, // 10^24 base units — the bound.
			));

			// Victim staking ONE: ONE * ONE / (ONE*ONE + ONE) rounds DOWN to zero.
			let bob_hdx_before = <Test as crate::Config>::Currency::balance(HDX, &BOB);
			assert_noop!(
				GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), ONE),
				Error::<Test>::ZeroAmount
			);
			assert_eq!(
				<Test as crate::Config>::Currency::balance(HDX, &BOB),
				bob_hdx_before,
				"victim's HDX must be untouched on a refused mint"
			);

			// Victim CAN still stake successfully if they scale up past the
			// rounding threshold — the protocol degrades to "expensive to enter",
			// not "broken".
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 1_000 * ONE));
			assert!(<Test as crate::Config>::Currency::balance(ST_HDX, &BOB) > 0);
		});
}

// donation does not break `stake_rewards`: the per-claim `pre_reward_hdx`
// accounts for the donation correctly, claimers just get fewer stHDX per HDX
// (which is the documented design).
#[test]
fn stake_rewards_should_succeed_when_direct_donation_inflated_rate() {
	ExtBuilder::default()
		.with_endowed(vec![(ALICE, HDX, 10_000 * ONE), (BOB, HDX, 10_000 * ONE)])
		.build()
		.execute_with(|| {
			// Bootstrap with a stake.
			assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

			// Attacker donates to inflate the rate.
			let gigapot = GigaHdx::gigapot_account_id();
			assert_ok!(<Test as crate::Config>::Currency::transfer(
				HDX,
				&BOB,
				&gigapot,
				200 * ONE,
				frame_support::traits::tokens::Preservation::Expendable,
			));

			// Simulate the voting-pallet flow: 30 HDX of reward arrives in the gigapot.
			assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 30 * ONE));

			// Claim should succeed and produce a non-zero stHDX share even though
			// the donation diluted the per-HDX value.
			let received = crate::Pallet::<Test>::stake_rewards(&BOB, 30 * ONE).unwrap();
			assert!(received > 0, "claim must produce non-zero stHDX even after donation");

			// And the math has not blown up: BOB's stHDX equals what was returned.
			assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &BOB), received);
		});
}
