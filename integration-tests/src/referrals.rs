#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::{Currencies, FeeProcessor, Omnipool, Referrals, Runtime, RuntimeOrigin, Staking, Tokens};
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
		let ref_pot_dai_before = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		let ref_pot_hdx_before = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		// Non-HDX fee path: the referrals receiver takes a fixed 5% slice of the trade fee in the
		// raw asset (DAI) into its pot at trade time.
		let ref_pot_dai = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(ref_pot_dai - ref_pot_dai_before, 7627218441026);
		// Referrals self-converts its raw asset into the HDX reward asset.
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));
		let ref_pot_hdx_after = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_eq!(ref_pot_hdx_after - ref_pot_hdx_before, 282133376);
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
		let ref_pot_dai_before = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		let ref_pot_hdx_before = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			u128::MAX,
		));
		// The asset-out (DAI) fee feeds the referrals pot a fixed 5% raw slice at trade time.
		let ref_pot_dai = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(ref_pot_dai - ref_pot_dai_before, 286937264028545);
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));
		let ref_pot_hdx_after = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_eq!(ref_pot_hdx_after - ref_pot_hdx_before, 10615145018);
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
		assert_eq!(referrer_shares, 171246068);
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
		assert_eq!(trader_shares, 114164045);
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
		assert_eq!(total_shares, 285410113);
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
		let old_balance = Currencies::free_balance(HDX, &ALICE.into());
		let old_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		assert!(Referrals::referrer_shares::<AccountId>(ALICE.into()) > old_shares);

		// The referrals pot holds its raw DAI slice; converting it to the reward asset (HDX) is
		// what funds the claimable pool.
		let referrals_dai = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(referrals_dai, 8057636306083);
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));
		assert_eq!(Currencies::free_balance(DAI, &Referrals::pot_account_id()), 0);

		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		assert_eq!(new_balance - old_balance, 600178825715);
		assert_eq!(Referrals::referrer_shares::<AccountId>(ALICE.into()), 0);
	});
}

#[test]
fn claiming_rewards_should_pay_trader_their_rebate() {
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
		// Trader (BOB) accrued a rebate share from their own trade.
		assert!(Referrals::trader_shares::<AccountId>(BOB.into()) > 0);

		// Convert the referrals pot's raw DAI slice into the reward asset (HDX) so the claim pays out.
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));

		// Capture BOB's balance after the trade + conversion, right before the claim, so the
		// delta isolates the trader rebate.
		let old_balance = Currencies::free_balance(HDX, &BOB.into());
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB.into())));

		let new_balance = Currencies::free_balance(HDX, &BOB.into());
		// Trader rebate is the trader-share fraction of the converted 5% slice held in the pot.
		assert_eq!(new_balance - old_balance, 400119214931);
		assert_eq!(Referrals::trader_shares::<AccountId>(BOB.into()), 0);
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

		// Referrals self-converts its raw DAI slice into the reward asset (HDX) before the claim.
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));
		assert_eq!(Currencies::free_balance(DAI, &Referrals::pot_account_id()), 0);

		//Act
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		//Assert that user receives claim amounts
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		let claimed_amount = new_balance - old_balance;
		assert_eq!(claimed_amount, 600178825715);
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

		// Referrals self-converts its raw DAI slice into the reward asset (HDX) before the claim.
		assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE.into()), DAI));
		assert_eq!(Currencies::free_balance(DAI, &Referrals::pot_account_id()), 0);

		//Act
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE.into())));

		//Assert that user receives claim amounts
		let new_balance = Currencies::free_balance(HDX, &ALICE.into());
		let claimed_amount = new_balance - old_balance;
		assert_eq!(claimed_amount, 600178825715);
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
fn trading_in_omnipool_should_not_transfer_any_fee_when_no_code_linked() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		let ref_pot_dai_before = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		let ref_pot_hdx_before = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0
		));
		// No code linked → referrals consumes none of its offered slice, so the pot receives
		// nothing and the un-taken slice stays with the Omnipool trade fee.
		let ref_pot_dai = Currencies::free_balance(DAI, &Referrals::pot_account_id());
		assert_eq!(ref_pot_dai - ref_pot_dai_before, 0);
		let ref_pot_hdx_after = Currencies::free_balance(HDX, &Referrals::pot_account_id());
		assert_eq!(ref_pot_hdx_after - ref_pot_hdx_before, 0);
		// No code linked → Level::None mints no shares.
		assert_eq!(Referrals::trader_shares::<AccountId>(Staking::pot_account_id()), 0);
		assert_eq!(Referrals::total_shares(), 0);
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
		// Tier0 split is referrer 60 : trader 40 (≈3:2, ±1 wei from independent flooring).
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 171246068);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 114164045);
		// Staking pot no longer receives external referral shares.
		assert_eq!(Referrals::trader_shares::<AccountId>(Staking::pot_account_id()), 0);
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares);
	});
}

#[test]
fn trading_in_omnipool_should_use_asset_rewards_when_set() {
	Hydra::execute_with(|| {
		init_omnipool_with_oracle_for_block_24();
		// The trade fee is in DAI (asset out of an HDX->DAI sell), so the per-asset override must
		// be keyed by DAI to take effect over the global tier split.
		assert_ok!(Referrals::set_reward_percentage(
			RuntimeOrigin::root(),
			DAI,
			pallet_referrals::Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(2),
				trader: Permill::from_percent(1),
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
		// Override split is referrer 2 : trader 1 = 2:1.
		let referrer_shares = Referrals::referrer_shares::<AccountId>(ALICE.into());
		assert_eq!(referrer_shares, 5708202);
		let trader_shares = Referrals::trader_shares::<AccountId>(BOB.into());
		assert_eq!(trader_shares, 2854101);
		assert_eq!(referrer_shares, trader_shares * 2, "referrer:trader must be 2:1");
		let total_shares = Referrals::total_shares();
		assert_eq!(total_shares, referrer_shares + trader_shares);
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
				amount_in: 27034239573270507,
				amount_out: 1_000_000_000_000,
				hub_amount_in: 1218703821,
				hub_amount_out: 1224869032,
				asset_fee_amount: 10215297085,
				protocol_fee_amount: 609351,
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: Filler::Omnipool,
				operation: TradeOperation::ExactOut,
				inputs: vec![Asset::new(DAI, 27034239573270507)],
				outputs: vec![Asset::new(LRNA, 1218703821)],
				fees: vec![Fee::new(
					LRNA,
					609351,
					Destination::Account(Omnipool::protocol_account()),
				)],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: Filler::Omnipool,
				operation: TradeOperation::ExactOut,
				inputs: vec![Asset::new(LRNA, 1218094470)],
				outputs: vec![Asset::new(HDX, 1_000_000_000_000)],
				fees: vec![
					Fee::new(HDX, 5618413398, Destination::Account(Omnipool::protocol_account())),
					Fee::new(HDX, 4596883687, Destination::Account(FeeProcessor::pot_account_id())),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
		]);

		// HDX fee path distributes the taken fee immediately to the configured HDX receivers,
		// so the staking pot receives its 5% slice without any conversion step.
		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), 0);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 510764854);
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
		let fee_processor_orig = Currencies::free_balance(DAI, &FeeProcessor::pot_account_id());
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(BOB.into()),
			DAI,
			HDX,
			1_000_000_000_000_000_000,
			u128::MAX,
		));

		let expected_taken_fee = 2582435376256921;

		expect_hydra_last_events(vec![
			pallet_omnipool::Event::BuyExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 37622382587443,
				amount_out: 1_000_000_000_000_000_000,
				hub_amount_in: 45362332344,
				hub_amount_out: 45481943470,
				asset_fee_amount: 5738745280570938,
				protocol_fee_amount: 22681166,
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: pallet_broadcast::types::Filler::Omnipool,
				operation: pallet_broadcast::types::TradeOperation::ExactOut,
				inputs: vec![Asset::new(HDX, 37622382587443)],
				outputs: vec![Asset::new(LRNA, 45362332344)],
				fees: vec![Fee::new(
					LRNA,
					22681166,
					Destination::Account(Omnipool::protocol_account()),
				)],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
			pallet_broadcast::Event::Swapped3 {
				swapper: BOB.into(),
				filler: Omnipool::protocol_account(),
				filler_type: pallet_broadcast::types::Filler::Omnipool,
				operation: pallet_broadcast::types::TradeOperation::ExactOut,
				inputs: vec![Asset::new(LRNA, 45339651178)],
				outputs: vec![Asset::new(DAI, 1_000_000_000_000_000_000)],
				fees: vec![
					Fee::new(
						DAI,
						3156309904314017,
						Destination::Account(Omnipool::protocol_account()),
					),
					Fee::new(
						DAI,
						expected_taken_fee,
						Destination::Account(FeeProcessor::pot_account_id()),
					),
				],
				operation_stack: vec![ExecutionType::Omnipool(0)],
			}
			.into(),
		]);

		// Non-HDX fee path with an unlinked trader: referrals consumes none of its offered slice,
		// so the referrals pot stays unchanged and the fee processor pot receives the full 45%
		// take. The staking pot only receives HDX later, after the fee processor converts.
		let fee_processor_balance = Currencies::free_balance(DAI, &FeeProcessor::pot_account_id());
		let ref_dai_balance = Currencies::free_balance(DAI, &ref_account);
		let staking_balance = Currencies::free_balance(HDX, &staking_acc);
		assert_eq!(fee_processor_balance.abs_diff(fee_processor_orig), expected_taken_fee);
		assert_eq!(ref_dai_balance.abs_diff(orig_balance), 0);
		assert_eq!(staking_balance.abs_diff(stak_orig_balance), 0);
	});
}

#[test]
fn trading_should_not_give_staking_pot_any_referral_shares() {
	// The `external` reward (which used to route referral shares to the staking pot) was
	// removed; staking is funded directly by the fee processor instead.
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

		assert_eq!(Referrals::trader_shares::<AccountId>(staking_acc), 0);
		assert_eq!(Referrals::total_shares(), 0);
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

/// Seed every pot the fee processor distributes to with at least ED, so that
/// small fee transfers during oracle population and the tested trade don't fail
/// with `BelowMinimum`. Must run before any trade triggers fee processing.
fn seed_pot_accounts() {
	for pot in [
		FeeProcessor::pot_account_id(),
		Staking::pot_account_id(),
		Referrals::pot_account_id(),
		pallet_gigahdx::Pallet::<Runtime>::gigapot_account_id(),
		pallet_gigahdx_rewards::Pallet::<Runtime>::reward_accumulator_pot(),
	] {
		assert_ok!(Currencies::update_balance(
			RawOrigin::Root.into(),
			pot,
			HDX,
			(10 * UNITS) as i128,
		));
	}
}

use pallet_broadcast::types::ExecutionType;
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
