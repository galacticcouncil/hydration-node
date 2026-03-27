#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use hydradx_runtime::{
	Currencies, FeeProcessor, GigaHdx, GigaHdxVoting, Omnipool, Referrals, Router, Runtime, RuntimeOrigin, Staking,
	System, Tokens,
};
use orml_traits::MultiCurrency;
use pallet_fee_processor::WeightInfo;
use pallet_referrals::ReferralCode;
use primitives::AccountId;
use sp_runtime::{FixedU128, Permill};
use std::vec;
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
		assert!(
			gigapot_increase > 0,
			"Gigapot should receive the largest share of HDX fees"
		);
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
		assert!(
			referrals_increase > 0,
			"Referrals pot should receive HDX from conversion"
		);

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

		assert!(gigapot_total > 0, "Gigapot should accumulate from multiple HDX trades");
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

#[test]
fn tiny_hdx_fee_doesnt_round_to_zero_for_any_receivers() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		//Arrange
		let source: AccountId = BOB.into();
		let trader: AccountId = BOB.into();

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		let reward_before = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());

		//Act
		assert_ok!(FeeProcessor::process_trade_fee(source, trader, HDX, 10));

		//Assert
		let gigapot_increase = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_before);
		let reward_increase = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_before);
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);

		assert_eq!(gigapot_increase, 7);
		assert_eq!(reward_increase, 2);
		assert_eq!(staking_increase, 1);
	});
}

/// BUG: Failed on_idle conversion removes PendingConversions but leaves
/// funds in the pot. The fees are permanently orphaned.
///
/// Scenario: DAI fees accumulate from trades, then governance disables
/// DAI trading in the Omnipool. on_idle tries to convert, the Omnipool
/// swap fails, PendingConversions is removed, but DAI stays in the pot.
/// No future retry will happen. Funds are stuck forever.
#[test]
fn failed_on_idle_conversion_orphans_funds_when_trading_disabled() {
	TestNet::reset();

	Hydra::execute_with(|| {
		use pallet_omnipool::types::Tradability;

		init_omnipool_with_oracle_for_block_24();

		// Step 1: Generate DAI fees via a normal trade
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending for conversion"
		);

		let pot_dai = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(pot_dai > 0, "Pot should have DAI fees");

		// Step 2: Governance disables DAI trading in the Omnipool
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY, // no SELL/BUY
		));

		// Step 3: on_idle tries to convert DAI → HDX, but the swap fails
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		// BUG: PendingConversions removed even though conversion failed
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"BUG: PendingConversions should NOT be removed on failure — funds need retry"
		);
	});
}

/// BUG: MinConversionAmount doesn't account for asset decimals.
///
/// MinConversionAmount = 1_000_000_000_000 (assumes 12 decimals like HDX).
/// For assets with fewer decimals (e.g., 6), even a meaningful trade fee
/// will be below this threshold. The conversion fails with AmountTooLow
/// and the fee is orphaned in the pot.
///
/// This reproduces the ConversionFailed events seen on lark testnet
/// (blocks 25209-25821).
#[test]
fn conversion_fails_for_low_decimal_asset_due_to_min_amount_not_accounting_for_decimals() {
	TestNet::reset();

	Hydra::execute_with(|| {
		use frame_support::storage::with_transaction;
		use hydradx_traits::registry::{AssetKind, Create};
		use sp_runtime::TransactionOutcome;

		init_omnipool_with_oracle_for_block_24();

		// Register a new asset with 6 decimals (like USDC)
		let low_dec_asset = with_transaction(|| {
			TransactionOutcome::Commit(hydradx_runtime::AssetRegistry::register_sufficient_asset(
				None,
				Some(b"USDC6".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1_000, // ED
				None,
				Some(6), // 6 decimals
				None,
				None,
			))
		})
		.unwrap();

		let one_usdc: Balance = 1_000_000; // 1 token with 6 decimals

		// Fund omnipool and BOB with the low-decimal asset
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			Omnipool::protocol_account(),
			low_dec_asset,
			(10_000 * one_usdc) as i128,
		));
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			low_dec_asset,
			(10_000 * one_usdc) as i128,
		));

		// Add to omnipool
		assert_ok!(Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			low_dec_asset,
			FixedU128::from_rational(1, 1), // 1:1 price
			Permill::from_percent(100),
			AccountId::from(ALICE),
		));
		set_zero_reward_for_referrals(low_dec_asset);

		// Populate oracle for the new asset
		do_trade_to_populate_oracle(low_dec_asset, HDX, one_usdc);
		go_to_block(48);
		do_trade_to_populate_oracle(low_dec_asset, HDX, one_usdc);

		// Trade HDX → low_dec_asset: generates a fee in the 6-decimal asset
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			low_dec_asset,
			UNITS / 10, // small trade to stay within pool limits
			0,
		));

		let pot_balance = Currencies::free_balance(low_dec_asset, &fee_processor_pot());
		assert!(
			pot_balance > 0,
			"Fee pot should have low-decimal asset fees: {}",
			pot_balance
		);
		assert!(
			pot_balance < UNITS, // below MinConversionAmount (1_000_000_000_000)
			"Fee ({}) should be below MinConversionAmount ({}) due to 6 decimals",
			pot_balance,
			UNITS
		);

		// Trigger on_idle — conversion fails because raw balance < MinConversionAmount
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		// Conversion should succeed — MinConversionAmount should account for decimals
		let events = last_hydra_events(500);
		let conversion_failed = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::ConversionFailed { .. })
			)
		});
		assert!(
			!conversion_failed,
			"BUG: ConversionFailed emitted for 6-decimal asset — MinConversionAmount doesn't account for decimals"
		);
	});
}

#[test]
fn zero_amount_fee_is_noop() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		//Arrange
		let source: AccountId = BOB.into();
		let trader: AccountId = BOB.into();

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		let reward_before = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());

		//Act
		assert_ok!(FeeProcessor::process_trade_fee(source, trader, HDX, 0));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &gigapot()), gigapot_before);
		assert_eq!(Currencies::free_balance(HDX, &giga_reward_pot()), reward_before);
		assert_eq!(Currencies::free_balance(HDX, &staking_pot()), staking_before);
	});
}

#[test]
fn dust_accumulates_over_repeated_trades() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let source: AccountId = BOB.into();
		let trader: AccountId = BOB.into();

		let pot_before = Currencies::free_balance(HDX, &fee_processor_pot());

		// 100 trades of 13 HDX each: 70%→9, 20%→2, 10%→1 = 12 distributed, 1 dust per trade
		for _ in 0..100 {
			assert_ok!(FeeProcessor::process_trade_fee(source.clone(), trader.clone(), HDX, 13));
		}

		let pot_after = Currencies::free_balance(HDX, &fee_processor_pot());
		let dust = pot_after - pot_before;

		assert_eq!(
			dust, 100,
			"Pot should accumulate 100 planck dust from 100 trades of 13 HDX"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Buy trades and mixed fee paths (A5-A6)
// ---------------------------------------------------------------------------

#[test]
fn buy_trade_distributes_fees_same_as_sell() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let pot_dai_before = Currencies::free_balance(DAI, &fee_processor_pot());

		// Buy DAI using HDX — generates DAI fee (non-HDX path)
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			UNITS,
			u128::MAX,
		));

		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_after > pot_dai_before,
			"Fee processor pot should accumulate DAI fees from buy trade"
		);

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be marked pending for conversion from buy trade"
		);

		// Convert and verify all 4 receiver pots get HDX
		let gigapot_pre = Currencies::free_balance(HDX, &gigapot());
		let reward_pre = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_pre = Currencies::free_balance(HDX, &staking_pot());
		let referrals_pre = Currencies::free_balance(HDX, &referrals_pot());

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let gigapot_increase = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_pre);
		let reward_increase = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_pre);
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_pre);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_pre);

		assert!(
			gigapot_increase > 0,
			"Gigapot should receive HDX from buy trade conversion"
		);
		assert!(
			reward_increase > 0,
			"GigaReward should receive HDX from buy trade conversion"
		);
		assert!(
			staking_increase > 0,
			"Staking should receive HDX from buy trade conversion"
		);
		assert!(
			referrals_increase > 0,
			"Referrals should receive HDX from buy trade conversion"
		);

		assert!(gigapot_increase > reward_increase, "Gigapot (60%) > Reward (20%)");
		assert!(reward_increase > staking_increase, "Reward (20%) > Staking (10%)");
	});
}

// ---------------------------------------------------------------------------
// Tests: on_idle conversion (B1-B4)
// ---------------------------------------------------------------------------

fn init_omnipool_with_eth_and_oracle() {
	init_omnipool();
	let stable_price = FixedU128::from_inner(45_000_000_000);
	assert_ok!(Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		ETH,
		stable_price,
		Permill::from_percent(100),
		AccountId::from(ALICE),
	));
	set_zero_reward_for_referrals(ETH);
	seed_pot_accounts();
	// Use smaller oracle trades for ETH to avoid MaxInRatioExceeded
	let eth_oracle_amount = UNITS / 100;
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	do_trade_to_populate_oracle(ETH, HDX, eth_oracle_amount);
	go_to_block(24);
	do_trade_to_populate_oracle(DAI, HDX, UNITS);
	do_trade_to_populate_oracle(ETH, HDX, eth_oracle_amount);
}

#[test]
fn on_idle_converts_multiple_pending_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_eth_and_oracle();

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			ETH,
			100 * UNITS,
			0,
		));

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(ETH),
			"ETH should be pending"
		);

		let gigapot_before = Currencies::free_balance(HDX, &gigapot());

		// Trigger on_idle with enough weight for multiple conversions
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be converted"
		);
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(ETH),
			"ETH should be converted"
		);

		let gigapot_after = Currencies::free_balance(HDX, &gigapot());
		assert!(
			gigapot_after > gigapot_before,
			"Gigapot should receive HDX from both conversions"
		);
	});
}

#[test]
fn on_idle_weight_exhaustion_converts_partial() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_eth_and_oracle();

		// Generate large fees for both assets
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			ETH,
			100 * UNITS,
			0,
		));

		assert_eq!(
			pallet_fee_processor::PendingConversions::<Runtime>::count(),
			2,
			"Exactly 2 assets should be pending"
		);

		// Give weight for only 1 conversion (1.5x convert weight — enough for 1, not 2)
		let convert_weight = <Runtime as pallet_fee_processor::Config>::WeightInfo::convert();
		let ref_time = convert_weight.ref_time() + convert_weight.ref_time() / 2;
		let weight = frame_support::weights::Weight::from_parts(ref_time, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		assert_eq!(
			pallet_fee_processor::PendingConversions::<Runtime>::count(),
			1,
			"Only 1 asset should be processed with limited weight, leaving 1 still pending"
		);
	});
}

#[test]
fn conversion_swap_generates_hdx_fee_distributed_to_hdx_receivers() {
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

		// Convert: the DAI→HDX swap itself generates an HDX fee
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		// Verify that the conversion generated a nested HDX FeeReceived event
		let events = last_hydra_events(500);

		let hdx_fee_during_conversion = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::FeeReceived {
					asset,
					..
				}) if *asset == HDX
			)
		});

		assert!(
			hdx_fee_during_conversion,
			"Conversion swap should generate a nested HDX fee (re-entrant process_trade_fee)"
		);

		// Both a Converted event and HDX FeeReceived event should exist
		let converted = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::Converted { .. })
			)
		});
		assert!(converted, "Converted event should be emitted");
	});
}

#[test]
fn double_convert_same_asset_second_fails() {
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

		// First convert succeeds
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending after conversion"
		);

		// Second convert fails — no balance left in pot (or below MinConversionAmount)
		assert!(
			FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI).is_err(),
			"Second convert should fail — no DAI fees remain"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Attack vectors / edge cases (D1-D3)
// ---------------------------------------------------------------------------

#[test]
fn anyone_can_call_convert() {
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

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);

		// DAVE — unrelated, non-privileged account — can trigger conversion
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(DAVE.into()), DAI));

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"Conversion should succeed when called by any signed account"
		);
	});
}

#[test]
fn manual_convert_recovers_after_asset_unfrozen() {
	TestNet::reset();

	Hydra::execute_with(|| {
		use pallet_omnipool::types::Tradability;

		init_omnipool_with_oracle_for_block_24();

		// Generate DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		let pot_dai = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(pot_dai > 0, "Pot should have DAI fees");

		// Freeze DAI trading — no conversion route
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::FROZEN,
		));

		// Manual convert fails while frozen
		assert!(
			FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI).is_err(),
			"Convert should fail when asset trading is frozen"
		);

		// Pending entry still exists — manual convert doesn't remove it on failure
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"PendingConversions entry should survive manual convert failure"
		);

		// Unfreeze DAI
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY,
		));

		// Manual convert now succeeds — funds recovered
		let gigapot_before = Currencies::free_balance(HDX, &gigapot());
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let gigapot_after = Currencies::free_balance(HDX, &gigapot());
		assert!(
			gigapot_after > gigapot_before,
			"Gigapot should receive HDX after unfreezing and converting"
		);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending after successful conversion"
		);
	});
}

#[test]
fn on_idle_orphans_fees_when_asset_frozen() {
	TestNet::reset();

	Hydra::execute_with(|| {
		use pallet_omnipool::types::Tradability;

		init_omnipool_with_oracle_for_block_24();

		// Generate DAI fees
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));

		let pot_dai = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(pot_dai > 0, "Pot should have DAI fees");

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);

		// Freeze DAI trading
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::FROZEN,
		));

		// on_idle runs — conversion fails and removes the pending entry
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		// Pending entry is gone — on_idle removes it on failure
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"on_idle should remove PendingConversions entry even on failure"
		);

		// Funds are still in the pot but no longer tracked — orphaned
		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_after > 0,
			"DAI fees remain in pot but are orphaned — no pending entry to trigger future conversion"
		);

		// Unfreeze DAI — but manual convert still fails because no pending entry
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY,
		));

		assert!(
			FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI).is_err(),
			"Convert should fail — no pending entry exists even though funds are in pot"
		);
	});
}

#[test]
fn router_trade_fee_distribution_matches_direct_trade() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let pot_dai_before = Currencies::free_balance(DAI, &fee_processor_pot());

		// Trade via router: HDX→DAI (non-HDX fee path)
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
			vec![].try_into().unwrap()
		));

		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_after > pot_dai_before,
			"Fee processor pot should accumulate DAI fees from router trade"
		);

		// Convert and verify all 4 receiver pots get HDX
		let gigapot_pre = Currencies::free_balance(HDX, &gigapot());
		let reward_pre = Currencies::free_balance(HDX, &giga_reward_pot());
		let staking_pre = Currencies::free_balance(HDX, &staking_pot());
		let referrals_pre = Currencies::free_balance(HDX, &referrals_pot());

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let gigapot_increase = Currencies::free_balance(HDX, &gigapot()).saturating_sub(gigapot_pre);
		let reward_increase = Currencies::free_balance(HDX, &giga_reward_pot()).saturating_sub(reward_pre);
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_pre);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_pre);

		assert!(
			gigapot_increase > 0,
			"Gigapot should receive HDX from router trade conversion"
		);
		assert!(
			reward_increase > 0,
			"GigaReward should receive HDX from router trade conversion"
		);
		assert!(
			staking_increase > 0,
			"Staking should receive HDX from router trade conversion"
		);
		assert!(
			referrals_increase > 0,
			"Referrals should receive HDX from router trade conversion"
		);

		assert!(gigapot_increase > reward_increase, "Gigapot (60%) > Reward (20%)");
		assert!(reward_increase > staking_increase, "Reward (20%) > Staking (10%)");
	});
}
