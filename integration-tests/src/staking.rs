#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Bounded;
use frame_support::traits::OnInitialize;
use frame_support::traits::StorePreimage;
use frame_system::RawOrigin;
use hydradx_runtime::{Balances, Currencies, Democracy, Omnipool, Preimage, Scheduler, System, Tokens};
use orml_traits::currency::MultiCurrency;
use pallet_democracy::{AccountVote, Conviction, ReferendumIndex, Vote};
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
	assert_ok!(propose_set_balance(ALICE.into(), ALICE.into(), 2));
	fast_forward_to(2);
	0
}
fn fast_forward_to(n: u32) {
	while System::block_number() < n {
		dbg!(System::block_number());
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
	});
}
