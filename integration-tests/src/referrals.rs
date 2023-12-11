#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Referrals, Runtime, RuntimeOrigin, Tokens};
use orml_traits::MultiCurrency;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

#[test]
fn registering_a_code_should_charge_registration_fee() {
	Hydra::execute_with(|| {
		let code = b"BALLS69".to_vec();
		let (reg_asset, reg_fee, reg_account) = <Runtime as pallet_referrals::Config>::RegistrationFee::get();
		let balance = Currencies::free_balance(reg_asset, &reg_account);
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE.into()), code));
		let balance_after = Currencies::free_balance(reg_asset, &reg_account);
		let diff = balance_after - balance;
		assert_eq!(diff, reg_fee);
	});
}

#[test]
fn trading_in_omnipool_should_transfer_portion_of_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		init_referrals_program();
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 2_060_386_836_081);
	});
}

#[test]
fn trading_in_omnipool_should_increase_referrer_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		init_referrals_program();
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let referrer_shares = Referrals::account_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 51_399_742);
	});
}
#[test]
fn trading_in_omnipool_should_increase_trader_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		init_referrals_program();
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let referrer_shares = Referrals::account_shares::<AccountId>(BOB.into());
		assert_eq!(referrer_shares, 25_699_871);
	});
}

#[test]
fn trading_in_omnipool_should_increase_total_shares_correctly() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		init_referrals_program();
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let referrer_shares = Referrals::total_shares();
		assert_eq!(referrer_shares, 25_699_871 + 51_399_742);
	});
}

#[test]
fn trading_hdx_in_omnipool_should_work_when_fee_is_below_existential_deposit() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		init_referrals_program();
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10_000_000_000_000_000_000,
			0
		));
		let referrer_shares = Referrals::account_shares::<AccountId>(BOB.into());
		assert_eq!(referrer_shares, 25_677_196);
	});
}

fn init_omnipool_with_oracle_for_block_10() {
	init_omnipool();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	set_relaychain_block_number(10);
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}

fn init_referrals_program() {
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Novice,
		Permill::from_percent(2),
		Permill::from_percent(1),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Advanced,
		Permill::from_percent(5),
		Permill::from_percent(2),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Expert,
		Permill::from_percent(10),
		Permill::from_percent(5),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Novice,
		Permill::from_percent(2),
		Permill::from_percent(1),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Advanced,
		Permill::from_percent(5),
		Permill::from_percent(2),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Expert,
		Permill::from_percent(10),
		Permill::from_percent(5),
	));
}
