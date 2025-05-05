#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens, Treasury};
use orml_traits::MultiCurrency;
use pallet_broadcast::types::Asset;
use pallet_broadcast::types::Destination;
use pallet_broadcast::types::Fee;
use pallet_broadcast::types::Filler;
use pallet_broadcast::types::TradeOperation;
use pallet_referrals::{FeeDistribution, ReferralCode};
use primitives::AccountId;
use sp_core::crypto::Ss58AddressFormat;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use std::vec;
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
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(pot_balance, 52326586502342568);
	});
}

#[test]
fn buying_in_omnipool_should_transfer_portion_of_asset_out_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(pot_balance, 54629772405101271);
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
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(referrer_shares, 141354464);
	});
}
#[test]
fn trading_in_omnipool_should_increase_trader_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(trader_shares, 94236309);
	});
}
#[test]
fn trading_in_omnipool_should_increase_external_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(external_shares, 1957821548045);
	});
}

#[test]
fn trading_in_omnipool_should_increase_total_shares_correctly() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(total_shares, 1958057138818);
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

//Since we use router account for executing trade,
//we have to verify if trader rewards is accrued for the actual trader, not in the router account
#[test]
fn claim_should_work_when_trade_happens_via_router() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_12();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));

		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let old_balance = Currencies::free_balance(HDX, &ALICE.into());

		//Do a trade to accrue some rewards
		assert_ok!(hydradx_runtime::Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0,
			vec![].try_into().unwrap()
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert!(pot_balance > 0);

		//Act
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 0);

		//Assert that user receives claim amounts
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		let claimed_amount = new_balance - old_balance;
		assert!(claimed_amount > 0);
		assert_eq!(claimed_amount, claimed_amount)
	});

	//We check if the same happens with normal omni trade
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool_with_oracle_for_block_12();
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));

		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let old_balance = Currencies::free_balance(HDX, &ALICE.into());
		//We do some trade to accrue some rewards
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert!(pot_balance > 0);

		//Act
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 0);

		//Assert that user receives claim amounts
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		let claimed_amount = new_balance - old_balance;
		assert!(claimed_amount > 0);
		assert_eq!(claimed_amount, claimed_amount);
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
		init_omnipool_with_oracle_for_block_24();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let pot_balance = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(pot_balance, 52326586502342569);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_global_rewards_when_not_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(referrer_shares, 141354464);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 94236309);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1957821548045);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_asset_rewards_when_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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
		assert_eq!(referrer_shares, 94236309);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 47118154);
		let external_shares = Referrals::trader_shares::<AccountId>(Staking::pot_account_id());
		assert_eq!(external_shares, 1956172412620);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares + external_shares);
	});
}

#[test]
fn buying_hdx_in_omnipool_should_transfer_correct_fee() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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

		expect_hydra_last_events(vec![
			pallet_omnipool::Event::BuyExecuted {
				who: BOB.into(),
				asset_in: DAI,
				asset_out: HDX,
				amount_in: 27007201989029080,
				amount_out: 1_000_000_000_000,
				hub_amount_in: 1217484967,
				hub_amount_out: 1216876226,
				asset_fee_amount: 9204958426,
				protocol_fee_amount: 608742,
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: Filler::Omnipool,
				operation: TradeOperation::ExactOut,
				inputs: vec![Asset::new(DAI, 27007201989029080)],
				outputs: vec![Asset::new(LRNA, 1217484967)],
				fees: vec![
					Fee::new(LRNA, 304371, Destination::Burned),
					Fee::new(LRNA, 304371, Destination::Account(Treasury::account_id())),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: Filler::Omnipool,
				operation: TradeOperation::ExactOut,
				inputs: vec![Asset::new(LRNA, 1216876225)],
				outputs: vec![Asset::new(HDX, 1_000_000_000_000)],
				fees: vec![
					Fee::new(HDX, 1, Destination::Account(Omnipool::protocol_account())),
					Fee::new(HDX, 9204958425, Destination::Account(Staking::pot_account_id())),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
		]);

		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), 0);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 9204958425);
	});
}

#[test]
fn buying_with_hdx_in_omnipool_should_transfer_correct_fee() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
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

		let expected_taken_fee = 2366144540787107;

		expect_hydra_last_events(vec![
			pallet_omnipool::Event::BuyExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 37584731096189,
				amount_out: 1_000_000_000_000_000_000,
				hub_amount_in: 45316936737,
				hub_amount_out: 45400948440,
				asset_fee_amount: 4732289081574215,
				protocol_fee_amount: 22658468,
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: pallet_broadcast::types::Filler::Omnipool,
				operation: pallet_broadcast::types::TradeOperation::ExactOut,
				inputs: vec![Asset::new(HDX, 37584731096189)],
				outputs: vec![Asset::new(LRNA, 45316936737)],
				fees: vec![
					Fee::new(LRNA, 11329234, Destination::Burned),
					Fee::new(LRNA, 11329234, Destination::Account(Treasury::account_id())),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: pallet_broadcast::types::Filler::Omnipool,
				operation: pallet_broadcast::types::TradeOperation::ExactOut,
				inputs: vec![Asset::new(LRNA, 45294278269)],
				outputs: vec![Asset::new(DAI, 1_000_000_000_000_000_000)],
				fees: vec![
					Fee::new(
						DAI,
						2366144540787108,
						Destination::Account(Omnipool::protocol_account()),
					),
					Fee::new(
						DAI,
						expected_taken_fee,
						Destination::Account(Referrals::pot_account_id()),
					),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
		]);

		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), expected_taken_fee);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 0);
	});
}

#[test]
fn trading_in_omnipool_should_increase_staking_shares_when_no_code_linked() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		let staking_acc = Staking::pot_account_id();

		let staking_shares = Referrals::trader_shares::<AccountId>(staking_acc);
		assert_eq!(staking_shares, 1958057138820);

		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, staking_shares);
	});
}

#[test]
fn transfer_using_mutate_should_emit_event() {
	// In our 1.1.0 upgrade, we introduced in issue where events weren't emitted
	// for the native asset from fungibles::Mutate trait.
	// This tests verifies the fix.
	use frame_support::traits::fungibles::Mutate;
	use frame_support::traits::tokens::Preservation;

	Hydra::execute_with(|| {
		assert_ok!(<Runtime as pallet_referrals::Config>::Currency::transfer(
			HDX,
			&ALICE.into(),
			&BOB.into(),
			1_000_000_000_000,
			Preservation::Preserve
		));

		expect_hydra_last_events(vec![pallet_balances::Event::Transfer {
			from: ALICE.into(),
			to: BOB.into(),
			amount: 1_000_000_000_000,
		}
		.into()])
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

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		ETH,
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

fn init_omnipool_with_oracle_for_block_24() {
	init_omnipool();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	set_relaychain_block_number(24);
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

use pallet_broadcast::types::ExecutionType;
use scraper::ALICE;
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
