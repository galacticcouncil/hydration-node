#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::{ReferralCode, Tier};
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

#[test]
fn registering_a_code_should_charge_registration_fee() {
	Hydra::execute_with(|| {
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert_eq!(pot_balance, 28_540_796_091_592_978);
	});
}

#[test]
fn trading_in_omnipool_should_increase_referrer_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert_eq!(referrer_shares, 128_499_434);
	});
}
#[test]
fn trading_in_omnipool_should_increase_trader_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		let trader_shares = Referrals::account_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 256_998_869);
	});
}
#[test]
fn trading_in_omnipool_should_increase_external_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		let external_shares = Referrals::account_shares::<AccountId>(Staking::pot_account_id().into());
		assert_eq!(external_shares, 2_164_560_909_660);
	});
}

#[test]
fn trading_in_omnipool_should_increase_total_shares_correctly() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, 256_998_869 + 128_499_434 + 2_164_560_909_660);
	});
}

#[test]
fn claiming_rewards_should_convert_all_assets_to_reward_asset() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert!(pot_balance > 0);

		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 0);
	});
}

#[test]
fn trading_hdx_in_omnipool_should_work_when_fee_is_below_existential_deposit() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert_eq!(referrer_shares, 98_704_716_390);
	});
}

#[test]
fn trading_in_omnipool_should_transfer_some_portion_of_fee_when_no_code_linked() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 28_540_796_091_592_980);
		let external_shares = Referrals::account_shares::<AccountId>(Staking::pot_account_id());
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_global_rewards_when_not_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert_eq!(referrer_shares, 128_499_434);
		let trader_shares = Referrals::account_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 256_998_869);
		let external_shares = Referrals::account_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 2_164_560_909_660);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_asset_rewards_when_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(Referrals::set_reward_percentage(
			RuntimeOrigin::root(),
			DAI,
			pallet_referrals::Level::Tier0,
			Tier {
				referrer: Permill::from_percent(2),
				trader: Permill::from_percent(1),
				external: Permill::from_percent(10),
			}
		));
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
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
		assert_eq!(referrer_shares, 51_399_773);
		let trader_shares = Referrals::account_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 25_699_886);
		let external_shares = Referrals::account_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 2_163_918_412_488);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_increase_staking_shares_when_no_code_linked() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let staking_acc = Staking::pot_account_id();
		let staking_shares = Referrals::account_shares::<AccountId>(staking_acc.into());
		assert_eq!(staking_shares, 2_164_946_407_964);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, staking_shares);
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
	seed_pot_account();
}

fn seed_pot_account() {
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		Referrals::pot_account_id(),
		HDX,
		(10 * UNITS) as i128,
	));
}

/*
fn init_referrals_program() {
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Tier0,
		Permill::from_percent(2),
		Permill::from_percent(1),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Tier1,
		Permill::from_percent(5),
		Permill::from_percent(2),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		HDX,
		pallet_referrals::Level::Tier2,
		Permill::from_percent(10),
		Permill::from_percent(5),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Tier0,
		Permill::from_percent(2),
		Permill::from_percent(1),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Tier1,
		Permill::from_percent(5),
		Permill::from_percent(2),
	));
	assert_ok!(Referrals::set_reward_percentage(
		RuntimeOrigin::root(),
		DAI,
		pallet_referrals::Level::Tier2,
		Permill::from_percent(10),
		Permill::from_percent(5),
	));

	seed_pot_account();
}
 */
