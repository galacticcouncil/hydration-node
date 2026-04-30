use super::mock::*;
use frame_support::{
	assert_ok,
	traits::fungibles::{Inspect, Mutate as FungiblesMutate},
};
use sp_runtime::{traits::One, FixedPointNumber, FixedU128};

#[test]
fn exchange_rate_should_be_one_when_pool_empty() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_should_remain_constant_when_user_stakes() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		// Rate remains 1:1 — HDX and stHDX increase proportionally
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());

		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 200 * ONE));
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_should_remain_constant_when_user_unstakes() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(BOB), 100 * ONE));

		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 50 * ONE));

		// Rate still 1:1 — HDX and stHDX decrease proportionally
		assert_eq!(GigaHdx::exchange_rate(), FixedU128::one());
	});
}

#[test]
fn exchange_rate_should_increase_when_fees_accrue() {
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
fn staker_should_receive_fewer_st_hdx_when_rate_increased() {
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
fn stake_rewards_should_use_current_exchange_rate() {
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

// `stake_rewards` divides by `pre_reward_hdx = total_hdx - hdx_amount`.
// When stHDX is outstanding but the pre-reward gigapot is empty,
// `pre_reward_hdx == 0` and the divisor would be zero. The fix routes that
// case through a 1:1 bootstrap mint instead of erroring with `Arithmetic`.
#[test]
fn stake_rewards_should_succeed_when_pre_reward_hdx_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// Drain Alice so we can build the degenerate state precisely.
		assert_ok!(GigaHdx::giga_stake(RuntimeOrigin::signed(ALICE), 100 * ONE));
		assert_ok!(GigaHdx::giga_unstake(RuntimeOrigin::signed(ALICE), 100 * ONE));

		let gigapot = GigaHdx::gigapot_account_id();

		// stHDX outstanding (CHARLIE) with empty backing, then a reward equal
		// to whatever the gigapot now holds — i.e. pre_reward_hdx will be 0.
		assert_ok!(<Test as crate::Config>::Currency::mint_into(ST_HDX, &CHARLIE, 50 * ONE));
		assert_ok!(<Test as crate::Config>::Currency::mint_into(HDX, &gigapot, 10 * ONE));

		// total_st_hdx == 50 * ONE, total_hdx == 10 * ONE.
		// pre_reward_hdx = 10 - 10 = 0 → bootstrap branch → mint 1:1.
		let received = crate::Pallet::<Test>::stake_rewards(&BOB, 10 * ONE).unwrap();
		assert_eq!(received, 10 * ONE);
		assert_eq!(<Test as crate::Config>::Currency::balance(ST_HDX, &BOB), 10 * ONE);
	});
}
