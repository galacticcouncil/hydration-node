#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::{FeeDistribution, ReferralCode};
use primitives::AccountId;
use sp_runtime::FixedU128;
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
		assert_eq!(pot_balance, 28_540_796_051_302_768);
	});
}
#[test]
fn trading_lrna_omnipool_should_not_transfer_portion_of_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_10();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			LRNA,
			1_000_000_000_000_000_000,
			u128::MAX,
		));
		let pot_balance = Currencies::free_balance(LRNA, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 0);
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
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 128_499_283);
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
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 256_998_567);
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
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id().into());
		assert_eq!(external_shares, 1_067_610_243_609);
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
		assert_eq!(total_shares, 1_067_995_741_459);
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
fn trading_hdx_in_omnipool_should_skip_referrals_program() {
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
		let referrer_shares = Referrals::referrer_shares::<AccountId>(BOB.into());
		assert_eq!(referrer_shares, 0);
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
		assert_eq!(pot_balance, 28_540_796_051_302_770);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
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
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 128_499_283);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 256_998_567);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1_067_610_243_609);
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
			FeeDistribution {
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
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 51_399_713);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 25_699_856);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1_066_967_747_190);
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
		let staking_shares = Referrals::trader_shares::<AccountId>(staking_acc.into());
		assert_eq!(staking_shares, 1_067_995_741_461);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, staking_shares);
	});
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	let native_position_id = hydradx_runtime::Omnipool::next_position_id();

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		AccountId::from(ALICE),
	));

	let stable_position_id = hydradx_runtime::Omnipool::next_position_id();

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));

	assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		native_position_id,
	));

	assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
		hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
		stable_position_id,
	));
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
