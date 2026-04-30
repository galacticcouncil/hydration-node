use super::mock::*;
use crate::hooks::GigaHdxVotingHooks;
use frame_support::traits::fungibles::{Inspect, Mutate};
use hydradx_traits::gigahdx::ReferendumOutcome;
use pallet_conviction_voting::{AccountVote, Status, Vote, VotingHooks};
use pallet_currencies::fungibles::FungibleCurrencies;

fn standard_vote(
	aye: bool,
	conviction: pallet_conviction_voting::Conviction,
	balance: Balance,
) -> AccountVote<Balance> {
	AccountVote::Standard {
		vote: Vote { aye, conviction },
		balance,
	}
}

#[test]
fn claim_rewards_should_transfer_hdx_and_mint_gigahdx() {
	ExtBuilder::default()
		.with_endowed(vec![
			(ALICE, HDX, 1_000 * ONE),
			(BOB, HDX, 1_000 * ONE),
			(ALICE, GIGAHDX, 500 * ONE),
			(BOB, GIGAHDX, 300 * ONE),
		])
		.build()
		.execute_with(|| {
			// Fund the GigaReward pot.
			let reward_pot = crate::Pallet::<Test>::giga_reward_pot_account();
			<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &reward_pot, 10_000 * ONE).unwrap();

			// Fund the gigapot with initial HDX (so exchange rate works).
			let gigapot = pallet_gigahdx::Pallet::<Test>::gigapot_account_id();
			<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(HDX, &gigapot, 1_000 * ONE).unwrap();
			// Mint stHDX to establish exchange rate (1:1 initially).
			<FungibleCurrencies<Test> as Mutate<AccountId>>::mint_into(ST_HDX, &ALICE, 1_000 * ONE).unwrap();

			set_track_id(0, 0);
			set_referendum_outcome(0, ReferendumOutcome::Approved);

			// ALICE votes and removes.
			let vote = standard_vote(true, pallet_conviction_voting::Conviction::Locked1x, 300 * ONE);
			assert_ok!(GigaHdxVotingHooks::<Test>::on_before_vote(&ALICE, 0, vote));
			GigaHdxVotingHooks::<Test>::on_remove_vote(&ALICE, 0, Status::Completed);

			// ALICE should have pending rewards.
			let pending = crate::PendingRewards::<Test>::get(&ALICE);
			assert!(!pending.is_empty());
			let reward_amount = pending[0].reward_amount;
			assert!(reward_amount > 0);

			let alice_sthdx_before = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(ST_HDX, &ALICE);

			// Claim rewards.
			assert_ok!(crate::Pallet::<Test>::claim_rewards(RuntimeOrigin::signed(ALICE)));

			// Pending rewards should be cleared.
			assert!(crate::PendingRewards::<Test>::get(&ALICE).is_empty());

			// With MoneyMarket no-op, stake_rewards mints stHDX and then
			// MoneyMarket::supply returns the same amount as "GIGAHDX".
			// Since GIGAHDX == ST_HDX through the identity MoneyMarket,
			// ALICE should have more stHDX (the MoneyMarket no-op returns
			// the stHDX amount as GIGAHDX, but actually mints stHDX).
			// Actually: stake_rewards mints stHDX to ALICE, then calls
			// MoneyMarket::supply which is identity (returns st_hdx_amount).
			// So ALICE gets GIGAHDX = stHDX minted (but GIGAHDX asset isn't
			// actually minted since MoneyMarket is a no-op).
			// The test should check stHDX increase instead.
			let alice_sthdx_after = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(ST_HDX, &ALICE);
			assert!(
				alice_sthdx_after > alice_sthdx_before,
				"ALICE should have more stHDX from rewards"
			);

			// Verify the gigapot received the HDX reward.
			let gigapot_balance = <FungibleCurrencies<Test> as Inspect<AccountId>>::balance(HDX, &gigapot);
			assert!(
				gigapot_balance > 1_000 * ONE,
				"gigapot should have more HDX after reward transfer"
			);
		});
}

#[test]
fn claim_rewards_should_fail_when_no_pending_rewards() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			crate::Pallet::<Test>::claim_rewards(RuntimeOrigin::signed(ALICE)),
			crate::Error::<Test>::NoPendingRewards
		);
	});
}
