#![cfg(test)]

use crate::{oracle::hydradx_run_to_block, polkadot_test_net::*};
use frame_support::assert_ok;
use frame_support::dispatch::GetDispatchInfo;
use frame_support::pallet_prelude::Weight;
use frame_support::weights::WeightToFee as WeightToFeeTrait;
use hydradx_runtime::Runtime;
use hydradx_runtime::TransactionPayment;
use hydradx_runtime::WeightToFee;
use pallet_dynamic_fees::types::FeeEntry;
use pallet_omnipool::traits::OmnipoolHooks;
use pallet_omnipool::WeightInfo;
use primitives::AssetId;
use sp_core::Encode;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;
const DOT_UNITS: u128 = 10_000_000_000;
const BTC_UNITS: u128 = 10_000_000;
const ETH_UNITS: u128 = 1_000_000_000_000_000_000;

//TODO: clean up in this test file

///original fee - 1.560005867338
///1 cent per swap, we don't want to be more expensive
///300 blocks to reach the max
///30k per hour
use frame_support::dispatch::DispatchInfo;

#[test]
fn fee_with_min_multiplier() {
	TestNet::reset();
	Hydra::execute_with(|| {
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(2);

		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
		));

		let call = pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: HDX,
			asset_out: DOT,
			amount: UNITS,
			min_buy_amount: 0,
		};

		let info = call.get_dispatch_info();

		/*pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MinimumMultiplier::get(),
		);*/

		let multiplier = pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::get();
		//assert_eq!(multiplier, 1.into());

		let rust_encoded_len = call.encoded_size() as u32;
		let rust_encoded_fees = TransactionPayment::compute_fee(rust_encoded_len, &info, 0); //638733816906
		assert_eq!(rust_encoded_fees / 4, 1596834542266);

		/*let post = PostDispatchInfo {
			actual_weight: Some(Weight::from_ref_time(55)), //520000033053
			pays_fee: Default::default(),
		};
		let rust_encoded_fees = TransactionPayment::compute_actual_fee(rust_encoded_len, &info, &post, 0);*/
		hydradx_run_to_block(3);
		hydradx_run_to_block(4);
		hydradx_run_to_block(5);
		hydradx_run_to_block(6);
	});
}

#[test]
fn fee_with_max_multiplier() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(10);

		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
		));

		let call = hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: DOT,
			asset_out: 2,
			amount: UNITS,
			min_buy_amount: 0,
		});

		let info = call.get_dispatch_info();

		pallet_transaction_payment::pallet::NextFeeMultiplier::<hydradx_runtime::Runtime>::put(
			hydradx_runtime::MaximumMultiplier::get(),
		);

		let rust_encoded_len = call.encoded_size() as u32;
		let rust_encoded_fees = TransactionPayment::compute_fee(rust_encoded_len, &info, 0); //6387338169067
		assert_eq!(rust_encoded_fees / 4, 5997338169067);
	});
}

use frame_support::dispatch::DispatchClass;

#[test]
fn fee_growth_simulator() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		init_oracle();
		let block_weight = hydradx_runtime::BlockWeights::get()
			.get(DispatchClass::Normal)
			.max_total
			.unwrap();

		for b in 2..=HOURS {
			hydradx_run_to_block(b);
			hydradx_runtime::System::set_block_consumed_resources(block_weight, 0);
			let call =
				hydradx_runtime::RuntimeCall::Omnipool(pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
					asset_in: DOT,
					asset_out: 2,
					amount: UNITS,
					min_buy_amount: 0,
				});

			let info = call.get_dispatch_info();
			let info_len = call.encoded_size() as u32;
			let fee = TransactionPayment::compute_fee(info_len, &info, 0);
			let next = TransactionPayment::next_fee_multiplier();

			//let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			println!("Swap tx fee: {fee:?} with multiplier: {next:?}");
		}
	});
}

#[test]
fn price_of_omnipool_swap_with_min_multiplier() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//TODO:
		//THIS MIGHT help to figure out the exact fee:
		//https://substrate.stackexchange.com/questions/4598/testing-transaction-fee-movements
		let weight = <Runtime as pallet_omnipool::Config>::WeightInfo::sell()
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_trade_weight())
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_liquidity_changed_weight());

		let fee: primitives::Balance = WeightToFee::weight_to_fee(&weight);

		let sell_old_weight = Weight::from_ref_time(255_333_000 as u64)
			.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(22 as u64))
			.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().writes(14 as u64))
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_trade_weight())
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_liquidity_changed_weight());

		let fee_old: Balance = WeightToFee::weight_to_fee(&sell_old_weight);

		assert_eq!(fee, fee_old);
	});
}
use sp_runtime::traits::SignedExtension;

use frame_support::dispatch::Dispatchable;
use frame_system::Origin;
use primitives::constants::time::HOURS;

#[test]
fn fee_with_tx_payment_pallet() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		init_oracle();
		hydradx_run_to_block(10);
		let alice_balance = hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE));

		let call = pallet_omnipool::Call::<hydradx_runtime::Runtime>::sell {
			asset_in: 2,
			asset_out: DOT,
			amount: 10 * UNITS,
			min_buy_amount: 0,
		};

		let info = call.get_dispatch_info();

		/*let weight = <Runtime as pallet_omnipool::Config>::WeightInfo::sell()
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_trade_weight())
			.saturating_add(<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_liquidity_changed_weight());
		let info = DispatchInfo {
			weight: weight,
			class: Default::default(),
			pays_fee: Default::default(),
		};*/
		let len = call.encode().len();

		let pre_d = pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0)
			.pre_dispatch(
				&AccountId::from(ALICE),
				&hydradx_runtime::RuntimeCall::Omnipool(call.clone()),
				&info,
				len,
			)
			.expect("pre_dispatch error");

		let post_result = hydradx_runtime::RuntimeCall::Omnipool(call)
			.dispatch(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()))
			.expect("dispatch failure");
		let actual_fee = TransactionPayment::compute_actual_fee(len.try_into().unwrap(), &info, &post_result, 0);

		assert_ok!(pallet_transaction_payment::ChargeTransactionPayment::<
			hydradx_runtime::Runtime,
		>::post_dispatch(Some(pre_d), &info, &post_result, len, &Ok(())));

		let alice_balance_after = hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE));
		let fee = alice_balance - alice_balance_after;
		assert_eq!(6_374_151_498_891, fee)
	});
}

/*
#[ignore]
#[test]
fn price_of_dca_schedule() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let schedule1 = Schedule {
			owner: AccountId::from(ALICE),
			period: 1u32,
			total_amount: 1000 * UNITS,
			max_retries: None,
			stability_threshold: None,
			slippage: Some(Permill::from_percent(5)),
			order: Order::Sell {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 500 * UNITS,
				min_amount_out: Balance::MIN,
				route: create_bounded_vec(vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				}]),
			},
		};

		let weight = <Runtime as pallet_dca::Config>::WeightInfo::schedule()
			+ <Runtime as pallet_dca::Config>::AmmTradeWeights::calculate_buy_trade_amounts_weight(
				&schedule1
					.order
					.get_route_or_default::<<Runtime as pallet_dca::Config>::RouteProvider>(),
			);
		dbg!(weight);

		let fee: Balance = WeightToFee::weight_to_fee(&weight);

		assert_eq!(fee, UNITS);
	});
}*/

fn set_balance(who: hydradx_runtime::AccountId, currency: AssetId, amount: i128) {
	assert_ok!(hydradx_runtime::Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		who,
		currency,
		amount,
	));
}

fn init_omnipool() {
	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let eth_price = FixedU128::from_inner(71_145_071_145_071);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		ETH,
		eth_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));

	let btc_price = FixedU128::from_inner(9_647_109_647_109_650_000_000_000);
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		hydradx_runtime::RuntimeOrigin::root(),
		BTC,
		btc_price,
		Permill::from_percent(100),
		AccountId::from(BOB),
	));
	set_zero_reward_for_referrals(HDX);
	set_zero_reward_for_referrals(DAI);
	set_zero_reward_for_referrals(DOT);
	set_zero_reward_for_referrals(ETH);
}

/// This function executes one sell and buy with HDX for all assets in the omnipool. This is necessary to
/// oracle have a prices for the assets.
/// NOTE: It's necessary to change parachain block to oracle have prices.
fn init_oracle() {
	let trader = DAVE;

	set_balance(trader.into(), HDX, 1_000 * UNITS as i128);
	set_balance(trader.into(), DOT, 1_000 * DOT_UNITS as i128);
	set_balance(trader.into(), ETH, 1_000 * ETH_UNITS as i128);
	set_balance(trader.into(), BTC, 1_000 * BTC_UNITS as i128);

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		2 * DOT_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		2 * DOT_UNITS,
		u128::MAX
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		2 * ETH_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		ETH,
		HDX,
		2 * ETH_UNITS,
		u128::MAX
	));

	assert_ok!(hydradx_runtime::Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		2 * BTC_UNITS,
		0,
	));

	assert_ok!(hydradx_runtime::Omnipool::buy(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		BTC,
		HDX,
		2 * BTC_UNITS,
		u128::MAX
	));
}
