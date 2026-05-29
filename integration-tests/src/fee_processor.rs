#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, traits::Hooks};
use frame_system::RawOrigin;
use hydradx_runtime::{
	Currencies, FeeProcessor, Omnipool, Referrals, Router, Runtime, RuntimeOrigin, Staking, System, Tokens,
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
		gigahdx_pot(),
		gigahdx_rewards_pot(),
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

fn gigahdx_pot() -> AccountId {
	pallet_gigahdx::Pallet::<Runtime>::gigapot_account_id()
}

fn gigahdx_rewards_pot() -> AccountId {
	pallet_gigahdx_rewards::Pallet::<Runtime>::reward_accumulator_pot()
}

// ---------------------------------------------------------------------------
// Tests: HDX fee distribution (HdxFeeReceivers: HdxStakingFeeReceiver 10%)
// ---------------------------------------------------------------------------

#[test]
fn hdx_fee_distributes_to_hdx_receivers() {
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

		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_before);

		// Referrals participates in the HDX fee path (ReferralsFeeReceiver is in HdxFeeReceivers),
		// so both pots receive their identical 5%/50% slice directly, without any conversion step.
		assert_eq!(
			staking_increase, 188932,
			"Staking pot should receive its HDX-path slice"
		);
		assert_eq!(
			referrals_increase, 188932,
			"Referrals pot should receive its HDX-path slice"
		);
	});
}

#[test]
fn hdx_fee_generates_referral_shares() {
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

		// DAI->HDX trade generates HDX fees. ReferralsFeeReceiver is part of HdxFeeReceivers,
		// so its on_pre_fee_deposit fires and referrer/trader shares accrue (Tier0 3%/2% split).
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			10 * UNITS,
			0,
		));

		let referrer_increase = Referrals::referrer_shares(AccountId::from(ALICE)) - referrer_shares_before;
		let trader_increase = Referrals::trader_shares(AccountId::from(BOB)) - trader_shares_before;

		// Tier0 split is referrer 3% : trader 2% = 3:2.
		assert_eq!(
			referrer_increase, 5667,
			"Referrer shares should accrue from HDX fee trade"
		);
		assert_eq!(trader_increase, 3778, "Trader shares should accrue from HDX fee trade");
		assert_eq!(
			referrer_increase * 2,
			trader_increase * 3,
			"Referrer:trader must be 3:2"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Non-HDX fee (FeeReceivers: StakingFeeReceiver 10%, ReferralsFeeReceiver 10%)
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

		// HDX should be distributed to all configured receiver pots
		// FeeReceivers: StakingFeeReceiver 10%, ReferralsFeeReceiver 10%
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_before);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_before);

		assert!(staking_increase > 0, "Staking pot should receive HDX from conversion");
		assert!(
			referrals_increase > 0,
			"Referrals pot should receive HDX from conversion"
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
fn non_hdx_conversion_distributes_exact_proportional_amounts_to_each_receiver() {
	TestNet::reset();

	Hydra::execute_with(|| {
		use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
		use sp_runtime::Rounding;

		init_omnipool_with_oracle_for_block_24();

		// Generate DAI fees via a trade
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
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());
		let gigahdx_before = Currencies::free_balance(HDX, &gigahdx_pot());
		let rewards_before = Currencies::free_balance(HDX, &gigahdx_rewards_pot());

		// Trigger conversion
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		// Pull hdx_out from the Converted event — this is the actual swap output the
		// non-HDX distribution math operates on.
		let events = last_hydra_events(200);
		let hdx_out = events
			.iter()
			.find_map(|e| match e {
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::Converted {
					asset_id,
					hdx_out,
					..
				}) if *asset_id == DAI => Some(*hdx_out),
				_ => None,
			})
			.expect("Converted event must be emitted");
		assert!(hdx_out > 0, "Swap should produce non-zero HDX");

		// Runtime non-HDX config: GigaHdx 15% + GigaHdxRewards 25% + Staking 5% + Referrals 5%
		// → total_pct = 50%. Each receiver's exact share of the converted hdx_out is
		//   floor(hdx_out * pct / total_pct).
		let total_pct = Permill::from_percent(50).deconstruct() as u128;
		let share = |pct: u32| {
			multiply_by_rational_with_rounding(
				hdx_out,
				Permill::from_percent(pct).deconstruct() as u128,
				total_pct,
				Rounding::Down,
			)
			.unwrap()
		};
		let gigahdx_share = share(15);
		let rewards_share = share(25);
		let staking_share = share(5);
		let referrals_share = share(5);

		// Captured before the conversion. Each pot's increase is its share of `hdx_out` PLUS a
		// share of the secondary HDX-side trade fee generated by the DAI→HDX conversion swap
		// itself (that fee re-enters via the HDX path and distributes in the same 15:25:5:5 split).
		let gigahdx_increase = Currencies::free_balance(HDX, &gigahdx_pot()) - gigahdx_before;
		let rewards_increase = Currencies::free_balance(HDX, &gigahdx_rewards_pot()) - rewards_before;
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()) - staking_before;
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()) - referrals_before;

		// Every receiver gets at least its primary `hdx_out` share, plus a strictly positive
		// secondary slice — so each strictly exceeds the primary share.
		assert!(
			gigahdx_increase > gigahdx_share,
			"GigaHdx must exceed its 15/50 primary share"
		);
		assert!(
			rewards_increase > rewards_share,
			"Rewards must exceed its 25/50 primary share"
		);
		assert!(
			staking_increase > staking_share,
			"Staking must exceed its 5/50 primary share"
		);
		assert!(
			referrals_increase > referrals_share,
			"Referrals must exceed its 5/50 primary share"
		);

		// Staking (5%) and Referrals (5%) are configured identically in both the non-HDX and HDX
		// paths, so their realized inflows are exactly equal.
		assert_eq!(
			staking_increase, referrals_increase,
			"Staking and Referrals (both 5%) must receive identical amounts"
		);

		// Deterministic exact distribution (primary + secondary), in the 15:25:5:5 proportion.
		assert_eq!(gigahdx_increase, 1101298710708, "GigaHdx total inflow");
		assert_eq!(rewards_increase, 1835497851180, "Rewards total inflow");
		assert_eq!(staking_increase, 367099570236, "Staking total inflow");
		assert_eq!(referrals_increase, 367099570236, "Referrals total inflow");

		// PendingConversions cleared, DAI fully drained.
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending"
		);
		assert_eq!(
			Currencies::free_balance(DAI, &fee_processor_pot()),
			0,
			"DAI pot should be fully drained — do_convert uses the entire pot balance"
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

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be converted and no longer pending"
		);

		let staking_after = Currencies::free_balance(HDX, &staking_pot());
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

		let staking_total = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_initial);
		let referrals_total = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_initial);

		// Both staking and referrals are in HdxFeeReceivers, so both accumulate across the 3 trades
		// (3 × the single-trade slice of 188932).
		assert_eq!(
			staking_total, 566796,
			"Staking pot should accumulate from multiple HDX trades"
		);
		assert_eq!(
			referrals_total, 566796,
			"Referrals pot should accumulate from multiple HDX trades"
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
fn tiny_hdx_fee_distributes_to_configured_receivers() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		//Arrange
		let source: AccountId = BOB.into();
		let trader: AccountId = BOB.into();

		let gigahdx_before = Currencies::free_balance(HDX, &gigahdx_pot());
		let rewards_before = Currencies::free_balance(HDX, &gigahdx_rewards_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		//Act: HdxFeeReceivers = GigaHdx 15% + GigaHdxRewards 25% + HdxStaking 5% + Referrals 5%
		// (total 50%). fee = 100 → take = 50, distributed as 15 / 25 / 5 / 5.
		assert_ok!(FeeProcessor::process_trade_fee(source, trader, HDX, 100));

		//Assert: each receiver gets its exact slice of the taken fee.
		assert_eq!(Currencies::free_balance(HDX, &gigahdx_pot()) - gigahdx_before, 15);
		assert_eq!(
			Currencies::free_balance(HDX, &gigahdx_rewards_pot()) - rewards_before,
			25
		);
		assert_eq!(Currencies::free_balance(HDX, &staking_pot()) - staking_before, 5);
		assert_eq!(Currencies::free_balance(HDX, &referrals_pot()) - referrals_before, 5);
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
fn failed_on_idle_conversion_drops_pending_and_keeps_funds_in_pot() {
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

		// Pending entry is dropped on failure — funds wait in the pot until a new
		// fee for the same asset re-marks it pending.
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"PendingConversions should be removed on failure to avoid retry churn"
		);
		let pot_dai_after = Currencies::free_balance(DAI, &fee_processor_pot());
		assert_eq!(pot_dai, pot_dai_after, "DAI fees should remain in pot");
	});
}

/// Low-decimal assets (e.g. 6 decimals like USDC) used to fail conversion
/// because of a hard `MinimumTradingLimit` denominated in HDX-decimals (12).
/// That gate is gone — we always attempt a swap and rely on the on_idle
/// drop-on-failure semantics if anything goes wrong.
#[test]
fn low_decimal_asset_converts_without_min_amount_gate() {
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

		// Fund omnipool and BOB with the low-decimal asset.
		// Hub-reserve must be large enough that selling `UNITS` of LRNA into the
		// pool stays under MaxInRatio (≈1/3); with a 1:1 price + 6-decimal asset
		// vs 12-decimal LRNA we need ~1e13 raw asset units to comfortably absorb
		// a 1 LRNA inflow.
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			Omnipool::protocol_account(),
			low_dec_asset,
			(10_000_000_000 * one_usdc) as i128,
		));
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			low_dec_asset,
			(10_000_000_000 * one_usdc) as i128,
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

		// Populate oracle for the new asset — `do_trade_to_populate_oracle` uses
		// `amount` as the LRNA-leg sell amount, which is denominated in LRNA
		// decimals (12), independent of the target asset's decimals.
		do_trade_to_populate_oracle(low_dec_asset, HDX, UNITS);
		go_to_block(48);
		do_trade_to_populate_oracle(low_dec_asset, HDX, UNITS);

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
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(low_dec_asset),
			"Low-decimal asset should be marked pending"
		);

		let staking_before = Currencies::free_balance(HDX, &staking_pot());

		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		// No ConversionFailed event — there's no min-amount gate any more.
		let events = last_hydra_events(500);
		let conversion_failed = events.iter().any(|e| {
			matches!(
				e,
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::ConversionFailed { .. })
			)
		});
		assert!(
			!conversion_failed,
			"ConversionFailed should not fire for low-decimal asset"
		);

		// Pot drained, pending entry cleared, HDX delivered.
		assert_eq!(
			Currencies::free_balance(low_dec_asset, &fee_processor_pot()),
			0,
			"Pot should be drained"
		);
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(low_dec_asset),
			"Pending entry cleared after successful conversion"
		);
		assert!(
			Currencies::free_balance(HDX, &staking_pot()) > staking_before,
			"Staking pot should receive HDX"
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

		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		//Act
		assert_ok!(FeeProcessor::process_trade_fee(source, trader, HDX, 0));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &staking_pot()), staking_before);
		assert_eq!(Currencies::free_balance(HDX, &referrals_pot()), referrals_before);
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

		// Convert and verify configured receiver pots get HDX
		let staking_pre = Currencies::free_balance(HDX, &staking_pot());
		let referrals_pre = Currencies::free_balance(HDX, &referrals_pot());

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_pre);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_pre);

		assert!(
			staking_increase > 0,
			"Staking should receive HDX from buy trade conversion"
		);
		assert!(
			referrals_increase > 0,
			"Referrals should receive HDX from buy trade conversion"
		);
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

		let staking_before = Currencies::free_balance(HDX, &staking_pot());

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

		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		assert!(
			staking_after > staking_before,
			"Staking pot should receive HDX from both conversions"
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
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let staking_after = Currencies::free_balance(HDX, &staking_pot());
		assert!(
			staking_after > staking_before,
			"Staking pot should receive HDX after unfreezing and converting"
		);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should no longer be pending after successful conversion"
		);
	});
}

#[test]
fn new_fee_re_marks_pending_after_failure_and_converts_full_pot_balance() {
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

		let pot_dai_first = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(pot_dai_first > 0, "Pot should have DAI fees");

		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending"
		);

		// Freeze DAI trading so the next on_idle conversion fails.
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::FROZEN,
		));

		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		// Pending entry dropped, funds remain in the pot.
		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"Pending entry should be dropped after failure"
		);
		assert_eq!(
			Currencies::free_balance(DAI, &fee_processor_pot()),
			pot_dai_first,
			"DAI fees should remain in pot"
		);

		// Unfreeze DAI and produce a new trade — this re-inserts DAI into PendingConversions.
		assert_ok!(Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DAI,
			Tradability::SELL | Tradability::BUY | Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY,
		));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"New trade should re-mark DAI pending"
		);
		let pot_dai_second = Currencies::free_balance(DAI, &fee_processor_pot());
		assert!(
			pot_dai_second > pot_dai_first,
			"Second trade should have added more DAI to the pot"
		);

		// on_idle now converts the FULL pot balance (both batches) to HDX.
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be converted"
		);
		assert_eq!(
			Currencies::free_balance(DAI, &fee_processor_pot()),
			0,
			"Pot should be fully drained — do_convert uses the full pot balance"
		);
		assert!(
			Currencies::free_balance(HDX, &staking_pot()) > staking_before,
			"Staking pot should receive HDX"
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

		// Convert and verify configured receiver pots get HDX
		let staking_pre = Currencies::free_balance(HDX, &staking_pot());
		let referrals_pre = Currencies::free_balance(HDX, &referrals_pot());

		assert_ok!(FeeProcessor::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		let staking_increase = Currencies::free_balance(HDX, &staking_pot()).saturating_sub(staking_pre);
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()).saturating_sub(referrals_pre);

		assert!(
			staking_increase > 0,
			"Staking should receive HDX from router trade conversion"
		);
		assert!(
			referrals_increase > 0,
			"Referrals should receive HDX from router trade conversion"
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: HDX path delivers fees to all four configured receivers
// HdxFeeReceivers = (GigaHdx 15%, GigaHdxRewards 25%, HdxStaking 5%, Referrals 5%)
// ---------------------------------------------------------------------------

#[test]
fn buying_hdx_from_omnipool_credits_all_four_hdx_fee_pots() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let giga_before = Currencies::free_balance(HDX, &gigahdx_pot());
		let giga_rewards_before = Currencies::free_balance(HDX, &gigahdx_rewards_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Buy 100 HDX with DAI — fee leg is on `asset_out` (HDX), so this hits
		// the HDX path of process_trade_fee and distributes synchronously.
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			u128::MAX,
		));

		// Pull the exact HDX trade-fee amount from the FeeReceived event so the
		// per-receiver assertions are pinned to the same input. Oracle-population
		// trades in `init_omnipool_with_oracle_for_block_24` also emit FeeReceived
		// events in the same block, so iterate in reverse to grab the buy's event.
		let events = last_hydra_events(200);
		let fee_amount = events
			.iter()
			.rev()
			.find_map(|e| match e {
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::FeeReceived {
					asset,
					amount,
					..
				}) if *asset == HDX => Some(*amount),
				_ => None,
			})
			.expect("FeeReceived event for HDX must be emitted");
		assert!(fee_amount > 0, "Recorded fee take must be non-zero");

		// `amount` in FeeReceived is the post-mul_floor take (50% of the raw fee).
		// Each receiver gets floor(fee_amount * its_pct / total_pct).
		// total_pct = 50%, receiver_pct ∈ {15, 25, 5, 5}.
		let pct = |p: u32| Permill::from_percent(p).deconstruct() as u128;
		let total = pct(50);
		let share = |p: u32| {
			sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
				fee_amount,
				pct(p),
				total,
				sp_runtime::Rounding::Down,
			)
			.unwrap()
		};

		let giga_increase = Currencies::free_balance(HDX, &gigahdx_pot()) - giga_before;
		let giga_rewards_increase = Currencies::free_balance(HDX, &gigahdx_rewards_pot()) - giga_rewards_before;
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()) - staking_before;
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()) - referrals_before;

		assert_eq!(giga_increase, share(15), "gigaHDX pot must receive 15/50 share");
		assert_eq!(
			giga_rewards_increase,
			share(25),
			"gigaHDX rewards pot must receive 25/50 share"
		);
		assert_eq!(staking_increase, share(5), "legacy staking pot must receive 5/50 share");
		assert_eq!(referrals_increase, share(5), "referrals pot must receive 5/50 share");

		// Conservation: the four receiver shares plus any rounding dust account
		// for the full take. With three 5% slices and one 15%/25%, distinct
		// numerator/denominator pairs each round down independently, so dust ≤ 3.
		let sum = giga_increase + giga_rewards_increase + staking_increase + referrals_increase;
		assert!(
			sum <= fee_amount && fee_amount - sum <= 3,
			"sum of receiver shares ({}) must equal take ({}) within 3 wei rounding",
			sum,
			fee_amount
		);
	});
}

// ---------------------------------------------------------------------------
// Tests: Non-HDX path — fee accrues in pot, converts on next block via
// on_idle, then distributes to all four receivers (plus a nested HDX fee
// from the conversion swap itself).
// ---------------------------------------------------------------------------

#[test]
fn selling_for_dai_then_advancing_block_distributes_converted_hdx_to_all_four_pots() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();

		let giga_before = Currencies::free_balance(HDX, &gigahdx_pot());
		let giga_rewards_before = Currencies::free_balance(HDX, &gigahdx_rewards_pot());
		let staking_before = Currencies::free_balance(HDX, &staking_pot());
		let referrals_before = Currencies::free_balance(HDX, &referrals_pot());

		// Sell HDX → DAI: fee is on `asset_out` (DAI), so the non-HDX path takes
		// 50% × fee into the fee-processor pot and marks DAI pending.
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			100 * UNITS,
			0,
		));
		assert!(
			pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should be pending after the non-HDX-fee trade"
		);

		// Forward a block, then run on_idle — `go_to_block` doesn't invoke on_idle,
		// so we call it explicitly with generous weight. This is the path the chain
		// will take in production once the next block fires.
		hydradx_run_to_next_block();
		let weight = frame_support::weights::Weight::from_parts(1_000_000_000_000, u64::MAX);
		pallet_fee_processor::Pallet::<Runtime>::on_idle(System::block_number(), weight);

		assert!(
			!pallet_fee_processor::PendingConversions::<Runtime>::contains_key(DAI),
			"DAI should have been converted via on_idle"
		);

		// The primary distribution operates on `hdx_out` from the Converted event
		// — the actual HDX yielded by the DAI→HDX swap inside the pallet.
		let events = last_hydra_events(500);
		let hdx_out = events
			.iter()
			.rev()
			.find_map(|e| match e {
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::Converted {
					asset_id,
					hdx_out,
					..
				}) if *asset_id == DAI => Some(*hdx_out),
				_ => None,
			})
			.expect("Converted event for DAI must be emitted");
		assert!(hdx_out > 0, "Conversion must yield non-zero HDX");

		// The swap that produced `hdx_out` also charges an HDX trade fee on the
		// HDX leg, re-entering process_trade_fee as the HDX path. That secondary
		// event's `amount` is the post-mul_floor take (50% × raw fee), and the
		// same 4 receivers split it via HdxFeeReceivers. Multiple HDX FeeReceived
		// events from prior oracle-population trades persist in the events buffer,
		// so iterate in reverse to grab the latest (= conversion swap's) one.
		let secondary_take = events
			.iter()
			.rev()
			.find_map(|e| match e {
				hydradx_runtime::RuntimeEvent::FeeProcessor(pallet_fee_processor::Event::FeeReceived {
					asset,
					amount,
					..
				}) if *asset == HDX => Some(*amount),
				_ => None,
			})
			.expect("Nested HDX FeeReceived event must be emitted from the conversion swap");

		// Each receiver gets floor(hdx_out * pct / 50) from the non-HDX path
		// AND floor(secondary_take * pct / 50) from the nested HDX path.
		let pct = |p: u32| Permill::from_percent(p).deconstruct() as u128;
		let total = pct(50);
		let share = |source: u128, p: u32| {
			sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
				source,
				pct(p),
				total,
				sp_runtime::Rounding::Down,
			)
			.unwrap()
		};
		let expected = |p: u32| share(hdx_out, p) + share(secondary_take, p);

		let giga_increase = Currencies::free_balance(HDX, &gigahdx_pot()) - giga_before;
		let giga_rewards_increase = Currencies::free_balance(HDX, &gigahdx_rewards_pot()) - giga_rewards_before;
		let staking_increase = Currencies::free_balance(HDX, &staking_pot()) - staking_before;
		let referrals_increase = Currencies::free_balance(HDX, &referrals_pot()) - referrals_before;

		assert_eq!(
			giga_increase,
			expected(15),
			"gigaHDX pot must receive 15/50 of non-HDX hdx_out plus 15/50 of nested HDX take"
		);
		assert_eq!(
			giga_rewards_increase,
			expected(25),
			"gigaHDX rewards pot must receive 25/50 of both inflows"
		);
		assert_eq!(
			staking_increase,
			expected(5),
			"legacy staking pot must receive 5/50 of both inflows"
		);
		assert_eq!(
			referrals_increase,
			expected(5),
			"referrals pot must receive 5/50 of both inflows"
		);
	});
}
