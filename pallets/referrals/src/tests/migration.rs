use crate::tests::*;

use crate::migration::migrate_to_accumulator;
use sp_core::U256;

const PRECISION: u128 = crate::pallet::PRECISION;

// A legacy chain (pre-accumulator) holds `TotalShares > 0` and a funded pot, but the new
// `RewardPerShare` accumulator defaults to zero. Without the migration a claim computes
// `shares * 0 / PRECISION - 0 = 0` and pays nothing; the migration must seed the accumulator
// so a legacy holder claims exactly the old `shares * (pot - seed) / total_shares`.
#[test]
fn migrate_to_accumulator_should_preserve_legacy_claim_amount() {
	let seed = ONE;
	let alice_shares = 3_000_000_000u128;
	let rewards = 600 * ONE;
	let pot = Referrals::pot_account_id();

	// The builder mints `seed` into the pot itself, so only the claimable rewards are endowed here.
	ExtBuilder::default()
		.with_seed_amount(seed)
		.with_referrer_shares(vec![(ALICE, alice_shares)])
		.with_endowed_accounts(vec![(pot, HDX, rewards)])
		.build()
		.execute_with(|| {
			// Legacy state: accumulator at its default, no debt, shares + funded pot present.
			assert!(RewardPerShare::<Test>::get().is_zero());
			assert_eq!(TotalShares::<Test>::get(), alice_shares);

			migrate_to_accumulator::<Test>();

			let expected_rps = U256::from(rewards) * U256::from(PRECISION) / U256::from(alice_shares);
			assert_eq!(RewardPerShare::<Test>::get(), expected_rps);

			let alice_before = Tokens::free_balance(HDX, &ALICE);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let claimed = Tokens::free_balance(HDX, &ALICE) - alice_before;

			assert_eq!(
				claimed, rewards,
				"legacy holder must claim pot-minus-seed after the accumulator migration"
			);
		});
}

// The migration must be a no-op when there are no shares (fresh chain / pre-adoption):
// dividing the pot by `total_shares == 0` is undefined, so the accumulator stays at zero.
#[test]
fn migrate_to_accumulator_should_be_noop_when_no_shares() {
	let seed = ONE;
	let pot = Referrals::pot_account_id();

	ExtBuilder::default()
		.with_seed_amount(seed)
		.with_endowed_accounts(vec![(pot, HDX, 500 * ONE)])
		.build()
		.execute_with(|| {
			assert_eq!(TotalShares::<Test>::get(), 0);

			migrate_to_accumulator::<Test>();

			assert!(
				RewardPerShare::<Test>::get().is_zero(),
				"accumulator must stay zero when there are no shares"
			);
		});
}
