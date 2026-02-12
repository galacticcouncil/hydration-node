#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, FeeProcessor, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
use orml_traits::MultiCurrency;
use pallet_referrals::{FeeDistribution, Level, ReferralCode};
use primitives::AccountId;
use sp_runtime::Permill;
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
	for pot in [FeeProcessor::pot_account_id(), staking_pot(), referrals_pot()] {
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

// ---------------------------------------------------------------------------
// Tests: HDX fee distribution
// ---------------------------------------------------------------------------

#[test]
fn hdx_fee_from_omnipool_trade_is_distributed_to_staking_and_referrals_pots() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

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

		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		let referrals_after = Currencies::free_balance(HDX, &referrals_pot());

		let staking_increase = staking_after.saturating_sub(staking_before);
		let referrals_increase = referrals_after.saturating_sub(referrals_before);

		// HdxFeeReceivers: Staking 50%, Referrals 30% → ratio 50:30 ≈ 62.5%:37.5%
		assert!(staking_increase > 0, "Staking pot should receive HDX fees");
		assert!(referrals_increase > 0, "Referrals pot should receive HDX fees");

		let total_fees = staking_increase + referrals_increase;
		let staking_pct = Permill::from_rational(staking_increase, total_fees);
		let referrals_pct = Permill::from_rational(referrals_increase, total_fees);

		// Allow 1% tolerance due to rounding
		assert!(
			staking_pct >= Permill::from_percent(61) && staking_pct <= Permill::from_percent(64),
			"Staking should get ~62.5%, got {:?}",
			staking_pct
		);
		assert!(
			referrals_pct >= Permill::from_percent(36) && referrals_pct <= Permill::from_percent(39),
			"Referrals should get ~37.5%, got {:?}",
			referrals_pct
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Non-HDX fee accumulation and conversion
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

		// DAI fee should be accumulated in fee processor pot
		assert!(
			pot_dai_after > pot_dai_before,
			"Fee processor pot should accumulate DAI fees. Before: {}, After: {}",
			pot_dai_before,
			pot_dai_after
		);

		// Should be marked as pending conversion
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be marked pending for conversion"
		);
	});
}

#[test]
fn non_hdx_fee_is_converted_via_manual_convert_extrinsic() {
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

		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Manually trigger conversion
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI,));

		// DAI should be gone from pot (swapped to HDX)
		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_after < pot_dai,
			"DAI should be consumed from pot after conversion"
		);

		// Pending should be removed
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending"
		);

		// HDX should be distributed to receiver pots
		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		let referrals_after = Currencies::free_balance(HDX, &referrals_pot());

		assert!(
			staking_after > staking_before,
			"Staking pot should receive HDX from conversion"
		);
		assert!(
			referrals_after > referrals_before,
			"Referrals pot should receive HDX from conversion"
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

		let staking_before = Currencies::free_balance(HDX, &staking_pot());

		// Trigger on_idle with generous weight
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(hydradx_runtime::System::block_number(), weight);

		// Pending should be cleared
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be converted and no longer pending"
		);

		// Staking pot should have received HDX
		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		assert!(
			staking_after > staking_before,
			"Staking pot should receive converted HDX via on_idle"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Referrals callback integration
// ---------------------------------------------------------------------------

#[test]
fn referrals_callback_records_shares_for_linked_trader() {
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

		// Set non-zero referral rewards for HDX
		assert_ok!(Referrals::set_reward_percentage(
			RawOrigin::Root.into(),
			HDX,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(10),
				trader: Permill::from_percent(10),
				external: Permill::zero(),
			},
		));

		let referrer_shares_before = Referrals::referrer_shares(AccountId::from(ALICE));

		// BOB trades DAI->HDX which generates HDX fees, triggering callbacks
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		// Verify referrer shares were recorded via the on_fee_received callback
		let referrer_shares_after = Referrals::referrer_shares(AccountId::from(ALICE));

		assert!(
			referrer_shares_after > referrer_shares_before,
			"Referrer shares should increase after trade. Before: {}, After: {}",
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
		// LRNA trades generate protocol fees (in LRNA), not trade fees through fee processor
		// Sell LRNA for DAI — LRNA side fee should be skipped by OmnipoolHookAdapter
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

		// Fee processor should NOT have accumulated LRNA
		let pot_lrna_after = Currencies::free_balance(LRNA, &fee_processor_pot());
		assert_eq!(
			pot_lrna_before, pot_lrna_after,
			"Fee processor pot should not accumulate LRNA"
		);

		// LRNA should NOT be pending
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(LRNA),
			"LRNA should never be pending for conversion"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Multiple trades accumulate correctly
// ---------------------------------------------------------------------------

#[test]
fn multiple_trades_accumulate_fees_correctly() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

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

		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		let referrals_after = Currencies::free_balance(HDX, &referrals_pot());

		let staking_total = staking_after.saturating_sub(staking_initial);
		let referrals_total = referrals_after.saturating_sub(referrals_initial);

		// Both pots should have accumulated fees from all 3 trades
		assert!(staking_total > 0, "Staking pot should accumulate from multiple trades");
		assert!(
			referrals_total > 0,
			"Referrals pot should accumulate from multiple trades"
		);

		// HdxFeeReceivers: 50:30 ratio → ~62.5% staking
		let total = staking_total + referrals_total;
		let staking_pct = Permill::from_rational(staking_total, total);
		assert!(
			staking_pct >= Permill::from_percent(61) && staking_pct <= Permill::from_percent(64),
			"Staking percentage should remain ~62.5% across multiple trades, got {:?}",
			staking_pct
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

		// Trade that generates HDX fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		// Check for FeeReceived event
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

		// Trade that generates DAI fees (sell HDX for DAI)
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			10 * UNITS,
			0,
		));

		// Check for FeeReceived event with DAI asset
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

		// Generate DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		// Convert
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI,));

		// Check for Converted event
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
