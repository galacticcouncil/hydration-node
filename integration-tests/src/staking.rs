#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Bounded;
use frame_support::traits::OnInitialize;
use frame_support::traits::StorePreimage;
use frame_system::RawOrigin;
use hydradx_runtime::{Balances, Currencies, Democracy, Omnipool, Preimage, Scheduler, Staking, System, Tokens};
use orml_traits::currency::MultiCurrency;
use pallet_democracy::{AccountVote, Conviction, ReferendumIndex, Vote};
use primitives::constants::time::DAYS;
use sp_runtime::AccountId32;
use xcm_emulator::TestExt;

type CallOf<T> = <T as frame_system::Config>::RuntimeCall;
type BoundedCallOf<T> = Bounded<CallOf<T>>;

fn set_balance_proposal(who: AccountId, value: u128) -> BoundedCallOf<hydradx_runtime::Runtime> {
	let inner = pallet_balances::Call::set_balance {
		who,
		new_free: value,
		new_reserved: 0,
	};
	let outer = hydradx_runtime::RuntimeCall::Balances(inner);
	Preimage::bound(outer).unwrap()
}

fn propose_set_balance(who: AccountId, dest: AccountId, value: u128) -> DispatchResult {
	Democracy::propose(
		hydradx_runtime::RuntimeOrigin::signed(who),
		set_balance_proposal(dest, value),
		100_000 * UNITS,
	)
}

fn begin_referendum() -> ReferendumIndex {
	assert_ok!(propose_set_balance(ALICE.into(), CHARLIE.into(), 2));
	fast_forward_to(3 * DAYS);
	0
}
fn end_referendum() {
	fast_forward_to(7 * DAYS);
}

fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		next_block();
	}
}
fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
	Democracy::on_initialize(System::block_number());
}
const AYE: Vote = Vote {
	aye: true,
	conviction: Conviction::None,
};

fn aye(amount: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: AYE,
		balance: amount,
	}
}

#[test]
fn staking_should_transfer_hdx_fees_to_pot_account_when_omnipool_trade_is_executed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool();

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			CHARLIE.into(),
			DAI,
			20_000_000 * UNITS,
			0,
		));

		assert_ok!(Omnipool::sell(
			hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			0u128,
		));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();

		assert_eq!(Currencies::free_balance(HDX, &staking_account), 1093580529360);
	});
}

#[test]
fn democracy_vote_should_record_stake_vote() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		let r = begin_referendum();
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);

		assert!(!stake_voting.votes.is_empty());
		let (ref_vote_idx, vote) = stake_voting.votes[0];
		assert_eq!(ref_vote_idx, 0);
		assert_eq!(
			vote,
			pallet_staking::types::Vote::new(2 * UNITS, pallet_staking::types::Conviction::None)
		);
		end_referendum();
	});
}

#[test]
fn staking_action_should_claim_points_for_finished_referendums_when_voted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let r = begin_referendum();

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();

		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		println!(
			"{:?}",
			pallet_staking::Pallet::<hydradx_runtime::Runtime>::positions(stake_position_id)
		);
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);
		let stake_position =
			pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position(stake_position_id).unwrap();

		assert_eq!(stake_position.get_action_points(), 2);
		assert!(stake_voting.votes.is_empty());
	});
}

#[test]
fn staking_should_transfer_rewards_when_claimed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let r = begin_referendum();

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();

		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));

		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));

		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));

		assert!(alice_balance_after_claim > alice_balance);

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);
		let stake_position =
			pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position(stake_position_id).unwrap();

		assert_eq!(stake_position.get_action_points(), 2);
		assert!(stake_voting.votes.is_empty());
	});
}

#[test]
fn staking_should_not_reward_when_double_claimed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let r = begin_referendum();

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();

		// first claim
		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert!(alice_balance_after_claim > alice_balance);

		// second claim
		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_eq!(alice_balance, alice_balance_after_claim);
	});
}

#[test]
fn staking_should_not_reward_when_stake_again_and_no_vote_activity() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let r = begin_referendum();

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();

		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		// first claim
		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert!(alice_balance_after_claim > alice_balance);

		// second claim
		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_eq!(alice_balance, alice_balance_after_claim);
	});
}

#[test]
fn staking_should_claim_and_unreserve_rewards_when_unstaked() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let r = begin_referendum();

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();

		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::unstake(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert!(alice_balance_after_claim > alice_balance);

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap();

		assert!(stake_position_id.is_none());
	});
}

#[test]
fn staking_should_remove_vote_when_democracy_removes_vote() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		let r = begin_referendum();
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);
		assert!(!stake_voting.votes.is_empty());
		let (ref_vote_idx, vote) = stake_voting.votes[0];
		assert_eq!(ref_vote_idx, 0);
		assert_eq!(
			vote,
			pallet_staking::types::Vote::new(2 * UNITS, pallet_staking::types::Conviction::None)
		);

		assert_ok!(Democracy::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r
		));
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);
		assert!(stake_voting.votes.is_empty());
		end_referendum();
	});
}

#[test]
fn staking_should_not_reward_when_refenrendum_is_ongoing() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		let r = begin_referendum();
		assert_ok!(Staking::stake(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			10 * UNITS
		));

		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));

		let stake_position_id = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_user_position_id(
			&sp_runtime::AccountId32::from(ALICE),
		)
		.unwrap()
		.unwrap();
		let stake_voting = pallet_staking::Pallet::<hydradx_runtime::Runtime>::get_position_votes(stake_position_id);
		assert!(!stake_voting.votes.is_empty());
		let alice_balance = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_ok!(Staking::claim(hydradx_runtime::RuntimeOrigin::signed(ALICE.into())));
		let alice_balance_after_claim = Currencies::free_balance(HDX, &AccountId32::from(ALICE));
		assert_eq!(alice_balance, alice_balance_after_claim);
		end_referendum();
	});
}

#[test]
fn democracy_vote_should_work_correctly_when_account_has_no_stake() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		let r = begin_referendum();
		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		end_referendum();
	});
}
#[test]
fn democracy_remote_vote_should_work_correctly_when_account_has_no_stake() {
	TestNet::reset();
	Hydra::execute_with(|| {
		System::set_block_number(0);
		init_omnipool();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into(), 0_u128));

		let staking_account = pallet_staking::Pallet::<hydradx_runtime::Runtime>::pot_account_id();
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			staking_account,
			HDX,
			10_000 * UNITS,
			0,
		));
		assert_ok!(Balances::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			1_000_000 * UNITS,
			0,
		));
		let r = begin_referendum();
		assert_ok!(Democracy::vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r,
			aye(2 * UNITS)
		));
		assert_ok!(Democracy::remove_vote(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			r
		));
		end_referendum();
	});
}
