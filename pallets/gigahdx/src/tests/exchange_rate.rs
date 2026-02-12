use super::mock::*;
use frame_support::{
	assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::{traits::One, FixedPointNumber, FixedU128};

#[test]
fn initial_exchange_rate_is_one() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_unchanged_after_stake() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Rate remains 1:1 — HDX and stHDX increase proportionally
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 200 * ONE));
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_unchanged_after_unstake() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 100 * ONE));

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 50 * ONE));

		// Rate still 1:1 — HDX and stHDX decrease proportionally
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_increases_with_fee_accrual() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Simulate fee accrual: add 50 HDX to gigapot
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 50 * ONE));

		// Rate is now 150/100 = 1.5
		assert_eq!(
			GigaHdx::exchange_rate(),
			FixedU128::checked_from_rational(150u128, 100u128).unwrap()
		);
	});
}

#[test]
fn new_staker_gets_fewer_st_hdx_after_rate_increase() {
	ExtBuilder::default().build().execute_with(|| {
		// Alice stakes 100 HDX at 1:1 → gets 100 stHDX
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &ALICE), 100 * ONE);

		// Fee accrual: gigapot now has 200 HDX total (100 + 100 fees)
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// Rate is now 2.0
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(2));

		// Bob stakes 100 HDX at 2.0 → gets 50 stHDX
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 100 * ONE));
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &BOB), 50 * ONE);

		// Totals: gigapot=300 HDX, stHDX=150
		// Rate: 300/150 = 2.0 (unchanged by proportional stake)
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::from(2));
	});
}

#[test]
fn stake_rewards_at_current_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// Alice stakes 100 HDX
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Fee accrual to get 2:1 rate
		let gigapot = GigaHdx::gigapot_account_id();
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 100 * ONE));

		// stake_rewards for Bob: 50 HDX → should get 25 stHDX at rate 2.0
		// First, put 50 HDX into gigapot (simulates voting pallet transferring reward)
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 50 * ONE));

		let gigahdx = crate::Pallet::<Test>::stake_rewards(&BOB, 50 * ONE).unwrap();
		assert_eq!(gigahdx, 25 * ONE);

		// Bob now has 25 stHDX
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &BOB), 25 * ONE);
	});
}
