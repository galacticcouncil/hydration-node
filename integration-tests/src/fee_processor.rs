#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, FeeProcessor, GigaHdx, GigaHdxVoting, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::ReferralCode;
use primitives::AccountId;
use xcm_emulator::TestExt;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
		1_000_000_000_000 * UNITS,
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
	// Ensure all pot accounts have at least ED so small fee transfers don't fail
	for pot in [
		FeeProcessor::pot_account_id(),
		staking_pot(),
		referrals_pot(),
		gigapot(),
		giga_reward_pot(),
	] {
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			pot,
			HDX,
			UNITS as i128,
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

fn gigapot() -> AccountId {
	GigaHdx::gigapot_account_id()
}

fn giga_reward_pot() -> AccountId {
	GigaHdxVoting::giga_reward_pot_account()
}

// ---------------------------------------------------------------------------
// Tests: HDX fee distribution (HdxFeeReceivers: GigaHdx 70%, GigaReward 20%, Staking 10%)
// ---------------------------------------------------------------------------

#[test]
fn hdx_fee_distributes_to_gigapot_reward_pot_and_staking() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		let reward_before = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Sell DAI for HDX — the trade fee on the HDX side will be in HDX
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		let gigapot_increase = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_before);
		let reward_increase = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_before);
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_before);

		// HdxFeeReceivers = (HdxGigaHdxFeeReceiver 70%, GigaHdxRewardFeeReceiver 20%, HdxStakingFeeReceiver 10%)
		assert!(gigapot_increase > 0, "Gigapot should receive the largest share of HDX fees");
		assert!(reward_increase > 0, "GigaReward pot should receive HDX fees");
		assert!(staking_increase > 0, "Staking pot should receive HDX fees");
		assert_eq!(referrals_increase, 0, "Referrals pot should NOT receive any HDX fees");

		// Verify proportions: gigapot (70%) > reward (20%) > staking (10%)
		assert!(
			gigapot_increase > reward_increase,
			"Gigapot (70%) should receive more than reward pot (20%): {} vs {}",
			gigapot_increase,
			reward_increase
		);
		assert!(
			reward_increase > staking_increase,
			"Reward pot (20%) should receive more than staking (10%): {} vs {}",
			reward_increase,
			staking_increase
		);
	});
}

#[test]
fn hdx_fee_does_not_generate_referral_shares() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		// Set up referral code
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"FEETEST".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));

		let referrer_shares_before = Referrals::referrer_shares(AccountId::from(ALICE));
		let trader_shares_before = Referrals::trader_shares(AccountId::from(BOB));

		// DAI->HDX trade generates HDX fees — should NOT trigger referrals callbacks
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		let referrer_shares_after = Referrals::referrer_shares(AccountId::from(ALICE));
		let trader_shares_after = Referrals::trader_shares(AccountId::from(BOB));

		assert_eq!(
			referrer_shares_before, referrer_shares_after,
			"Referrer shares should NOT increase from HDX fee trade"
		);
		assert_eq!(
			trader_shares_before, trader_shares_after,
			"Trader shares should NOT increase from HDX fee trade"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Non-HDX fee (FeeReceivers: GigaHdx 60%, GigaReward 20%, Staking 10%, Referrals 10%)
// ---------------------------------------------------------------------------

#[test]
fn non_hdx_fee_from_omnipool_trade_is_accumulated_in_fee_processor_pot() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let pot_dai_before = Currencies::free_balance(DAI, &fee_processor_pot());

		// Sell HDX for DAI — the trade fee on the DAI side will be in DAI (non-HDX)
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			10 * UNITS,
			0,
		));

		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());

		assert!(
			pot_dai_after > pot_dai_before,
			"Fee processor pot should accumulate DAI fees. Before: {}, After: {}",
			pot_dai_before,
			pot_dai_after
		);

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be marked pending for conversion"
		);
	});
}

#[test]
fn non_hdx_fee_conversion_distributes_to_all_receivers() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		// Execute a trade that generates DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		let pot_dai = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(pot_dai > 0, "Fee processor pot should have DAI");

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		let reward_before = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Manually trigger conversion
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		// DAI should be gone from pot (swapped to HDX)
		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_after < pot_dai,
			"DAI should be consumed from pot after conversion"
		);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending"
		);

		// HDX should be distributed to ALL four receiver pots
		// FeeReceivers: GigaHdx 60%, GigaReward 20%, Staking 10%, Referrals 10%
		let gigapot_increase = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_before);
		let reward_increase = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_before);
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_before);

		assert!(gigapot_increase > 0, "Gigapot should receive HDX from conversion");
		assert!(reward_increase > 0, "GigaReward pot should receive HDX from conversion");
		assert!(staking_increase > 0, "Staking pot should receive HDX from conversion");
		assert!(referrals_increase > 0, "Referrals pot should receive HDX from conversion");

		// Verify proportions: gigapot (60%) > reward (20%) > staking (10%) = referrals (10%)
		assert!(
			gigapot_increase > reward_increase,
			"Gigapot (60%) should receive more than reward pot (20%): {} vs {}",
			gigapot_increase,
			reward_increase
		);
		assert!(
			reward_increase > staking_increase,
			"Reward pot (20%) should receive more than staking (10%): {} vs {}",
			reward_increase,
			staking_increase
		);
		// Staking and referrals both get 10%, but referrals on_fee_received callback
		// redistributes some HDX to referrers, so the pot balance may be slightly less.
		let diff = staking_increase.abs_diff(referrals_increase);
		let tolerance = staking_increase / 10; // 10% tolerance
		assert!(
			diff <= tolerance,
			"Staking (10%) and referrals (10%) should receive roughly equal amounts: {} vs {} (diff: {})",
			staking_increase,
			referrals_increase,
			diff
		);
	});
}

#[test]
fn non_hdx_fee_is_converted_on_idle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		// Execute a trade that generates DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());

		// Trigger on_idle with generous weight
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(hydradx_runtime::System::block_number(), weight);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be converted and no longer pending"
		);

		let gigapot_after = Currencies::free_balance(HDX, &gigapot());
		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		assert!(
			gigapot_after > gigapot_before,
			"Gigapot should receive converted HDX via on_idle"
		);
		assert!(
			staking_after > staking_before,
			"Staking pot should receive converted HDX via on_idle"
		);
	});
}

#[test]
fn non_hdx_fee_generates_referral_shares_for_linked_trader() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		// Set up referral code for ALICE and link BOB
		let code =
			ReferralCode::<<Runtime as pallet_referrals::Config>::CodeLength>::truncate_from(b"FEETEST".to_vec());
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE.into()),
			code.clone()
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB.into()), code));

		let referrer_shares_before = Referrals::referrer_shares(AccountId::from(ALICE));

		// HDX->DAI trade generates DAI fees — non-HDX path triggers referrals callbacks
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			10 * UNITS,
			0,
		));

		let referrer_shares_after = Referrals::referrer_shares(AccountId::from(ALICE));

		assert!(
			referrer_shares_after > referrer_shares_before,
			"Referrer shares should increase from non-HDX fee trade. Before: {}, After: {}",
			referrer_shares_before,
			referrer_shares_after
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: LRNA fees
// ---------------------------------------------------------------------------

#[test]
fn lrna_fees_are_not_processed_by_fee_processor() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let pot_lrna_before = Currencies::free_balance(LRNA, &fee_processor_pot());

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			LRNA,
			100 * UNITS,
			0,
		));

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			LRNA,
			DAI,
			10 * UNITS,
			0,
		));

		let pot_lrna_after = Currencies::free_balance(LRNA, &fee_processor_pot());
		assert_eq!(
			pot_lrna_before, pot_lrna_after,
			"Fee processor pot should not accumulate LRNA"
		);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(LRNA),
			"LRNA should never be pending for conversion"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Multiple trades
// ---------------------------------------------------------------------------

#[test]
fn multiple_hdx_trades_accumulate_in_all_hdx_receivers() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let gigapot_initial = Currencies::free_balance(HDX, &gigapot());
		let reward_initial = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_initial = Currencies::free_balance(HDX, &staking_pot());
		let referrals_initial = Currencies::free_balance(HDX, &referrals_pot());

		// Multiple HDX-generating trades
		for _ in 0..3 {
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				10 * UNITS,
				0,
			));
		}

		let gigapot_total = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_initial);
		let reward_total = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_initial);
		let staking_total = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_initial);
		let referrals_total = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_initial);

		assert!(
			gigapot_total > 0,
			"Gigapot should accumulate from multiple HDX trades"
		);
		assert!(
			reward_total > 0,
			"GigaReward pot should accumulate from multiple HDX trades"
		);
		assert!(
			staking_total > 0,
			"Staking pot should accumulate from multiple HDX trades"
		);
		assert_eq!(
			referrals_total, 0,
			"Referrals pot should NOT receive anything from HDX trades"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Event emission
// ---------------------------------------------------------------------------

#[test]
fn fee_received_event_is_emitted_for_hdx_fee() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		let events = last_hydra_events(100);
		let fee_received = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(
					pallet_fee_processor::Event::FeeReceived { asset, .. }
				) if *asset == HDX
			)
		});

		assert!(fee_received, "FeeReceived event should be emitted for HDX fee");
	});
}

#[test]
fn fee_received_event_is_emitted_for_non_hdx_fee() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			10 * UNITS,
			0,
		));

		let events = last_hydra_events(100);
		let fee_received = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(
					pallet_fee_processor::Event::FeeReceived { asset, .. }
				) if *asset == DAI
			)
		});

		assert!(fee_received, "FeeReceived event should be emitted for DAI fee");
	});
}

#[test]
fn converted_event_is_emitted_after_manual_conversion() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let events = last_hydra_events(100);
		let converted = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(
					pallet_fee_processor::Event::Converted { asset_id, .. }
				) if *asset_id == DAI
			)
		});

		assert!(converted, "Converted event should be emitted after conversion");
	});
}
