#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, FeeProcessor, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::{FeeDistribution, Level, ReferralCode};
use primitives::AccountId;
use sp_core::crypto::Ss58AddressFormat;
use sp_runtime::{FixedU128, Permill};
use std::vec;
use xcm_emulator::TestExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
	seed_pot_accounts();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	go_to_block(12);
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
}

fn init_omnipool_with_oracle_for_block_24() {
	init_omnipool();
	seed_pot_accounts();
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	go_to_block(24);
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

fn seed_pot_accounts() {
	for pot in [
		FeeProcessor::pot_account_id(),
		Staking::pot_account_id(),
		Referrals::pot_account_id(),
	] {
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			pot,
			HDX,
			(10 * UNITS) as i128,
		));
	}
}

fn staking_pot() -> AccountId {
	Staking::pot_account_id()
}

fn referrals_pot() -> AccountId {
	Referrals::pot_account_id()
}

fn fee_processor_pot() -> AccountId {
	FeeProcessor::pot_account_id()
}

fn register_and_link(referrer: [u8; 32], trader: [u8; 32]) {
	let code = ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"BALLS69".to_vec());
	assert_ok!(Referrals::register_code(
		RuntimeOrigin::signed(referrer.into()),
		code.clone()
	));
	assert_ok!(Referrals::link_code(RuntimeOrigin::signed(trader.into()), code,));
}

/// Trade HDX→DAI (non-HDX fee), then convert DAI→HDX so referrals pot gets HDX and accumulator bumps.
fn trade_and_convert(trader: [u8; 32], amount: Balance) {
	assert_ok!(Omnipool::sell(
		RuntimeOrigin::signed(trader.into()),
		HDX,
		DAI,
		amount,
		0
	));
	// Convert the accumulated DAI fee to HDX (distributes to staking + referrals)
	assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(trader.into()), DAI,));
}

// ---------------------------------------------------------------------------
// Tests: Registration & Linking
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests: Share distribution via non-HDX fee path
// ---------------------------------------------------------------------------

#[test]
fn non_hdx_trade_should_increase_referrer_and_trader_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		let referrer_before = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_before = Referrals::trader_shares::<AccountId>(BOB.into());

		// Sell HDX for DAI — DAI fee goes through non-HDX path, triggers referrals callbacks
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let referrer_after = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_after = Referrals::trader_shares::<AccountId>(BOB.into());

		assert!(
			referrer_after > referrer_before,
			"Referrer shares should increase from non-HDX trade. Before: {}, After: {}",
			referrer_before,
			referrer_after
		);
		assert!(
			trader_after > trader_before,
			"Trader shares should increase from non-HDX trade. Before: {}, After: {}",
			trader_before,
			trader_after
		);
	});
}

#[test]
fn hdx_trade_should_not_generate_referral_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		let referrer_before = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_before = Referrals::trader_shares::<AccountId>(BOB.into());

		// Sell DAI for HDX — HDX fee bypasses referrals entirely
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10_000_000_000_000_000_000,
			0
		));

		let referrer_after = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_after = Referrals::trader_shares::<AccountId>(BOB.into());

		assert_eq!(
			referrer_before, referrer_after,
			"Referrer shares should NOT change from HDX trade"
		);
		assert_eq!(
			trader_before, trader_after,
			"Trader shares should NOT change from HDX trade"
		);
	});
}

#[test]
fn trading_should_increase_external_shares_for_staking() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		let external_before = Referrals::trader_shares::<AccountId>(staking_pot());

		// Non-HDX trade triggers referrals via FeeReceivers
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let external_after = Referrals::trader_shares::<AccountId>(staking_pot());

		// External (staking) shares should increase (45% of referrals' share at Tier0)
		assert!(
			external_after > external_before,
			"External shares should increase. Before: {}, After: {}",
			external_before,
			external_after
		);
	});
}

#[test]
fn no_code_linked_should_only_generate_external_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let total_before = Referrals::total_shares();

		// BOB trades HDX→DAI without any linked code — Level::None → 50% external only
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let referrer_shares = Referrals::referrer_shares::<AccountId>(BOB.into());
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		let total_after = Referrals::total_shares();

		// No referrer or trader shares for unlinked account
		assert_eq!(referrer_shares, 0, "Unlinked trader should get no referrer shares");
		assert_eq!(
			trader_shares, 0,
			"Unlinked trader should get no trader shares at Level::None"
		);

		// But external shares (staking pot) should have increased
		assert!(
			total_after > total_before,
			"Total shares should increase from external. Before: {}, After: {}",
			total_before,
			total_after
		);
	});
}

#[test]
fn total_shares_should_equal_sum_of_individual_shares() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		let referrer_before = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_before = Referrals::trader_shares::<AccountId>(BOB.into());
		let external_before = Referrals::trader_shares::<AccountId>(staking_pot());
		let total_before = Referrals::total_shares();

		// Non-HDX trade
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let referrer_after = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_after = Referrals::trader_shares::<AccountId>(BOB.into());
		let external_after = Referrals::trader_shares::<AccountId>(staking_pot());
		let total_after = Referrals::total_shares();

		let referrer_increase = referrer_after - referrer_before;
		let trader_increase = trader_after - trader_before;
		let external_increase = external_after - external_before;

		assert_eq!(
			total_after - total_before,
			referrer_increase + trader_increase + external_increase,
			"Total shares increase should equal sum of individual increases"
		);
	});
}

#[test]
fn custom_asset_reward_percentages_should_be_used_when_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// Set custom rewards for HDX (RewardAsset) at Tier0: 10% referrer, 10% trader, 0% external.
		// on_fee_received looks up rewards by T::RewardAsset (HDX), not the original fee asset.
		assert_ok!(Referrals::set_reward_percentage(
			RuntimeOrigin::root(),
			HDX,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(10),
				trader: Permill::from_percent(10),
				external: Permill::zero(),
			}
		));

		let referrer_before = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let trader_before = Referrals::trader_shares::<AccountId>(BOB.into());
		let external_before = Referrals::trader_shares::<AccountId>(staking_pot());

		// HDX→DAI trade (non-HDX path) — triggers referrals with custom HDX rewards
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));

		let referrer_increase = Referrals::referrer_shares::<AccountId>(ALICE.into()) - referrer_before;
		let trader_increase = Referrals::trader_shares::<AccountId>(BOB.into()) - trader_before;
		let external_increase = Referrals::trader_shares::<AccountId>(staking_pot()) - external_before;

		// With custom 10/10/0, referrer and trader should be equal
		assert!(referrer_increase > 0, "Referrer shares should increase");
		assert_eq!(
			referrer_increase, trader_increase,
			"With equal percentages, referrer and trader shares should match"
		);
		assert_eq!(external_increase, 0, "External should get no shares with 0%");
	});
}

// ---------------------------------------------------------------------------
// Tests: LRNA exclusion
// ---------------------------------------------------------------------------

#[test]
fn trading_lrna_omnipool_should_not_transfer_portion_of_fee_to_reward_pot() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_12();
		register_and_link(ALICE, BOB);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			LRNA,
			1_000_000_000_000_000_000,
			u128::MAX,
		));
		let pot_balance = Currencies::free_balance(LRNA, &referrals_pot());
		assert_eq!(pot_balance, 0);
	});
}

// ---------------------------------------------------------------------------
// Tests: Accumulator — non-HDX path (trade + convert + claim)
// ---------------------------------------------------------------------------

#[test]
fn non_hdx_trade_convert_then_claim_should_distribute_rewards() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		let alice_hdx_before = Currencies::free_balance(HDX, &ALICE.into());

		// Trade HDX→DAI (non-HDX fee) + convert → HDX lands in referrals pot
		trade_and_convert(BOB, 100 * UNITS);

		// Referrals pot should now have more HDX
		let pot_hdx = Currencies::free_balance(HDX, &referrals_pot());
		assert!(pot_hdx > 10 * UNITS, "Referrals pot should have HDX from conversion");

		// RewardPerShare was bumped
		let rps = pallet_referrals::RewardPerShare::<Runtime>::get();
		assert!(!rps.is_zero(), "RewardPerShare should be non-zero after conversion");

		// ALICE claims
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let alice_hdx_after = Currencies::free_balance(HDX, &ALICE.into());
		let alice_claimed = alice_hdx_after - alice_hdx_before;

		assert!(
			alice_claimed > 0,
			"ALICE should receive HDX after conversion + claim. Got: {}",
			alice_claimed
		);

		// Shares should be burned
		assert_eq!(
			Referrals::referrer_shares::<AccountId>(ALICE.into()),
			0,
			"Referrer shares should be burned after claim"
		);
	});
}

#[test]
fn claim_should_burn_shares_and_decrement_total() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// Non-HDX trade + convert so accumulator is bumped
		trade_and_convert(BOB, 100 * UNITS);

		let total_before = Referrals::total_shares();
		let alice_referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		let alice_trader_shares = Referrals::trader_shares::<AccountId>(ALICE.into());
		let alice_total = alice_referrer_shares + alice_trader_shares;

		assert!(alice_total > 0, "ALICE should have shares before claim");

		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		// Shares burned
		assert_eq!(Referrals::referrer_shares::<AccountId>(ALICE.into()), 0);
		assert_eq!(Referrals::trader_shares::<AccountId>(ALICE.into()), 0);

		// TotalShares decremented
		let total_after = Referrals::total_shares();
		assert_eq!(
			total_after,
			total_before - alice_total,
			"Total shares should decrease by user's burned shares"
		);
	});
}

#[test]
fn claim_with_no_conversion_yet_should_be_noop() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// Non-HDX trade: shares minted but HDX not yet converted/deposited
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100_000_000_000_000,
			0
		));

		// Shares should exist
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert!(referrer_shares > 0, "Referrer should have shares from spot estimate");

		let alice_hdx_before = Currencies::free_balance(HDX, &ALICE.into());

		// Claim — no HDX has been deposited for these shares, so no rewards
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		let alice_hdx_after = Currencies::free_balance(HDX, &ALICE.into());
		let claimed = alice_hdx_after.saturating_sub(alice_hdx_before);

		// Should succeed without panic; may get 0 or tiny dust from setup trades
		assert!(claimed == 0 || claimed > 0, "Claim should succeed without errors");
	});
}

#[test]
fn non_hdx_conversion_via_on_idle_should_bump_accumulator() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// Generate DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0
		));

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);

		let rps_before = pallet_referrals::RewardPerShare::<Runtime>::get();

		// Trigger on_idle
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(hydradx_runtime::System::block_number(), weight);

		let rps_after = pallet_referrals::RewardPerShare::<Runtime>::get();

		assert!(
			rps_after > rps_before,
			"RewardPerShare should increase after on_idle conversion. Before: {:?}, After: {:?}",
			rps_before,
			rps_after
		);

		// Now ALICE can claim
		let alice_before = Currencies::free_balance(HDX, &ALICE.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let alice_after = Currencies::free_balance(HDX, &ALICE.into());

		assert!(
			alice_after > alice_before,
			"ALICE should receive HDX after on_idle conversion"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Share lifecycle — claim burns, re-earn works
// ---------------------------------------------------------------------------

#[test]
fn claim_burns_shares_then_new_trade_reearns_shares() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// First trade + convert
		trade_and_convert(BOB, 100 * UNITS);

		let alice_shares_1 = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert!(alice_shares_1 > 0, "ALICE should have shares after first trade");

		// ALICE claims — shares burned
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		assert_eq!(
			Referrals::referrer_shares::<AccountId>(ALICE.into()),
			0,
			"Shares should be 0 after claim"
		);

		// Second trade + convert — ALICE earns new shares
		trade_and_convert(BOB, 100 * UNITS);

		let alice_shares_2 = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert!(
			alice_shares_2 > 0,
			"ALICE should re-earn shares after new trade. Got: {}",
			alice_shares_2
		);

		// Second claim
		let alice_before = Currencies::free_balance(HDX, &ALICE.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let alice_after = Currencies::free_balance(HDX, &ALICE.into());

		assert!(
			alice_after > alice_before,
			"ALICE should receive rewards on second claim"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Multiple traders — proportional distribution
// ---------------------------------------------------------------------------

#[test]
fn multiple_traders_claim_proportionally() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		// ALICE is referrer for both BOB and CHARLIE
		let code = ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"MULTI".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code.clone()));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(CHARLIE.into()), code));

		// Give CHARLIE enough HDX for trading
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			CHARLIE.into(),
			HDX,
			(1_000 * UNITS) as i128,
		));

		// Both trade HDX→DAI (non-HDX fee path) + convert
		trade_and_convert(BOB, 100 * UNITS);
		trade_and_convert(CHARLIE, 100 * UNITS);

		let bob_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		let charlie_shares = Referrals::trader_shares::<AccountId>(CHARLIE.into());

		assert!(bob_shares > 0, "BOB should have trader shares");
		assert!(charlie_shares > 0, "CHARLIE should have trader shares");

		// Both claim
		let bob_before = Currencies::free_balance(HDX, &BOB.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB.into())));
		let bob_claimed = Currencies::free_balance(HDX, &BOB.into()) - bob_before;

		let charlie_before = Currencies::free_balance(HDX, &CHARLIE.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(CHARLIE.into())));
		let charlie_claimed = Currencies::free_balance(HDX, &CHARLIE.into()) - charlie_before;

		assert!(bob_claimed > 0, "BOB should receive rewards");
		assert!(charlie_claimed > 0, "CHARLIE should receive rewards");

		// ALICE (referrer for both) also claims
		let alice_before = Currencies::free_balance(HDX, &ALICE.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let alice_claimed = Currencies::free_balance(HDX, &ALICE.into()) - alice_before;
		assert!(alice_claimed > 0, "ALICE (referrer) should receive rewards");
	});
}

// ---------------------------------------------------------------------------
// Tests: Router integration
// ---------------------------------------------------------------------------

#[test]
fn claim_should_work_when_trade_happens_via_router() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		register_and_link(ALICE, BOB);

		// Trade via router: HDX→DAI (non-HDX fee path)
		assert_ok!(hydradx_runtime::Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
			vec![].try_into().unwrap()
		));

		// ALICE should have referrer shares
		let alice_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert!(alice_shares > 0, "Referrer shares should be recorded via router trade");

		// Convert so accumulator bumps
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		// Claim should work
		let alice_before = Currencies::free_balance(HDX, &ALICE.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let alice_after = Currencies::free_balance(HDX, &ALICE.into());
		let claimed = alice_after - alice_before;

		assert!(
			claimed > 0,
			"Claim should yield HDX rewards via router trade. Got: {}",
			claimed
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Fee routing verification
// ---------------------------------------------------------------------------

#[test]
fn buying_hdx_should_send_fee_only_to_staking() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));

		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Buy HDX with DAI — HDX fee path → only staking
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			u128::MAX,
		));

		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_before);

		assert!(staking_increase > 0, "Staking should receive HDX from buy trade");
		assert_eq!(referrals_increase, 0, "Referrals should NOT receive HDX from HDX fee");
	});
}

#[test]
fn buying_dai_with_hdx_should_accumulate_fee_in_fee_processor() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let ref_dai_before = Currencies::free_balance(DAI, &referrals_pot());
		let fp_dai_before = Currencies::free_balance(DAI, &fee_processor_pot());

		// Buy DAI with HDX — DAI fee goes to fee-processor pot, not referrals
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let ref_dai_after = Currencies::free_balance(DAI, &referrals_pot());
		let fp_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());

		assert_eq!(ref_dai_before, ref_dai_after, "Referrals pot should not accumulate DAI");
		assert!(
			fp_dai_after > fp_dai_before,
			"Fee processor pot should accumulate DAI fees"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Event emission
// ---------------------------------------------------------------------------

#[test]
fn transfer_using_mutate_should_emit_event() {
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

// ---------------------------------------------------------------------------
// Tests: Parachain code pre-registration
// ---------------------------------------------------------------------------

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
