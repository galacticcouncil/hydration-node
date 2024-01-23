#![cfg(test)]
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::{FeeDistribution, ReferralCode};
use primitives::AccountId;
use sp_core::crypto::Ss58AddressFormat;
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
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(pot_balance, 29_307_364_722_907_532);
	});
}

#[test]
fn buying_in_omnipool_should_transfer_portion_of_asset_out_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
			HDX,
			1_000_000_000_000_000_000,
			u128::MAX,
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 30_594_591_369_789_397);
	});
}

#[test]
fn trading_lrna_omnipool_should_not_transfer_portion_of_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(referrer_shares, 131_950_592);
	});
}
#[test]
fn trading_in_omnipool_should_increase_trader_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(trader_shares, 263_901_185);
	});
}
#[test]
fn trading_in_omnipool_should_increase_external_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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

		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1_096_284_866_630);
	});
}

#[test]
fn trading_in_omnipool_should_increase_total_shares_correctly() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(total_shares, 1_096_680_718_407);
	});
}

#[test]
fn claiming_rewards_should_convert_all_assets_to_reward_asset() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		init_omnipool_with_oracle_for_block_12();
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
		init_omnipool_with_oracle_for_block_12();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 29_307_364_722_907_532);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_global_rewards_when_not_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(referrer_shares, 131_950_592);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 263_901_185);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1_096_284_866_630);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_asset_rewards_when_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
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
		assert_eq!(referrer_shares, 52_780_237);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 26_390_118);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1_095_625_113_667);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn buying_hdx_in_omnipool_should_transfer_correct_fee() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));
		let staking_acc = Staking::pot_account_id();
		let ref_account = Referrals::pot_account_id();
		let orig_balance = Currencies::free_balance(DAI, &ref_account);
		let stak_orig_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			u128::MAX,
		));

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: BOB.into(),
			asset_in: DAI,
			asset_out: HDX,
			amount_in: 26_835_579_541_620_354,
			amount_out: 1_000_000_000_000,
			hub_amount_in: 1_209_746_177,
			hub_amount_out: 1_209_141_304,
			asset_fee_amount: 2_794_789_078,
			protocol_fee_amount: 604_873,
		}
		.into()]);

		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), 0);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 2_794_789_077);
	});
}

#[test]
fn buying_with_hdx_in_omnipool_should_transfer_correct_fee() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));
		let staking_acc = Staking::pot_account_id();
		let ref_account = Referrals::pot_account_id();
		let orig_balance = Currencies::free_balance(DAI, &ref_account);
		let stak_orig_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
			who: BOB.into(),
			asset_in: HDX,
			asset_out: DAI,
			amount_in: 37_506_757_329_085,
			amount_out: 1_000_000_000_000_000_000,
			hub_amount_in: 45_222_713_080,
			hub_amount_out: 45_200_101_724,
			asset_fee_amount: 2_644_977_450_514_458,
			protocol_fee_amount: 22_611_356,
		}
		.into()]);

		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), 2_644_977_450_514_458 / 2 - 1);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 0);
	});
}

#[test]
fn trading_in_omnipool_should_increase_staking_shares_when_no_code_linked() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let staking_acc = Staking::pot_account_id();

		let staking_shares = Referrals::trader_shares::<AccountId>(staking_acc);
		assert_eq!(staking_shares, 1_096_680_718_408);

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

fn init_omnipool_with_oracle_for_block_12() {
	init_omnipool();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	set_relaychain_block_number(12);
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

use sp_core::crypto::Ss58Codec;
pub const PARACHAIN_CODES: [(&str, &str); 12] = [
	("MOONBEAM", "7LCt6dFmtiRrwZv2YyEgQWW3GxsGX3Krmgzv9Xj7GQ9tG2j8"),
	("ASSETHUB", "7LCt6dFqtxzdKVB2648jWW9d85doiFfLSbZJDNAMVJNxh5rJ"),
	("INTERLAY", "7LCt6dFsW7xwUutdYad3oeQ1zfQvZ9THXbBupWLqpd72bmnM"),
	("CENTRIFUGE", "7LCt6dFsJVukxnxpix9KcTkwu2kWQnXARsy6BuBHEL54NcS6"),
	("ASTAR", "7LCt6dFnHxYDyomeCEC8nsnBUEC6omC6y7SZQk4ESzDpiDYo"),
	("BIFROST", "7LCt6dFs6sraSg31uKfbRH7soQ66GRb3LAkGZJ1ie3369crq"),
	("ZEITGEIST", "7LCt6dFCEKr7CctCKBb6CcQdV9iHDue3JcpxkkFCqJZbk3Xk"),
	("PHALA", "7LCt6dFt6z8V3Gg41U4EPCKEHZQAzEFepirNiKqXbWCwHECN"),
	("UNIQUE", "7LCt6dFtWEEr5WXfej1gmZbNUpj1Gx7u29J1yYAen6GsjQTj"),
	("NODLE", "7LCt6dFrJPdrNCKncokgeYZbQsSRgyrYwKrz2sMUGruDF9gJ"),
	("SUBSOCIAL", "7LCt6dFE2vLjshEThqtdwGAGMqg2XA39C1pMSCjG9wsKnR2Q"),
	("POLKADOT", "7KQx4f7yU3hqZHfvDVnSfe6mpgAT8Pxyr67LXHV6nsbZo3Tm"),
];

#[test]
fn verify_preregisters_codes() {
	Hydra::execute_with(|| {
		pallet_referrals::migration::preregister_parachain_codes::<hydradx_runtime::Runtime>();
		for (code, account) in PARACHAIN_CODES.into_iter() {
			let code =
				ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::try_from(code.as_bytes().to_vec())
					.unwrap();
			let a = Referrals::referral_account(code);
			assert_eq!(
				a.unwrap().to_ss58check_with_version(Ss58AddressFormat::custom(63)),
				account
			);
		}
	});
}
