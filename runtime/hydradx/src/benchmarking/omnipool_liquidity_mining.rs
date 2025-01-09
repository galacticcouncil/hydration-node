// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg(feature = "runtime-benchmarks")]

use crate::benchmarking::{register_asset, register_asset_with_decimals};
use crate::*;
use frame_benchmarking::{account, BenchmarkError};
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use frame_support::traits::EnsureOrigin;
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use hydradx_traits::liquidity_mining::{GlobalFarmId, YieldFarmId};
use hydradx_traits::registry::{AssetKind, Create};
use hydradx_traits::stableswap::AssetAmount;
use orml_benchmarking::runtime_benchmarks;
use primitives::AssetId;
use sp_runtime::{traits::One, FixedU128, Permill};
use sp_runtime::{DispatchError, DispatchResult, Perquintill, TransactionOutcome};
use warehouse_liquidity_mining::LoyaltyCurve;
const ONE: Balance = 1_000_000_000_000;
const BTC_ONE: Balance = 100_000_000;
const HDX: AssetId = 0;
const LRNA: AssetId = 1;
const DAI: AssetId = 2;
const BSX: AssetId = 1_000_001;
const ETH: AssetId = 1_000_002;
const BTC: AssetId = 1_000_003;
const DOT: AssetId = 1_000_004;

const G_FARM_TOTAL_REWARDS: Balance = 10_000_000 * ONE;
const REWARD_CURRENCY: AssetId = HDX;

fn fund(to: AccountId, currency: AssetId, amount: Balance) -> DispatchResult {
	Currencies::deposit(currency, &to, amount)
}

pub const INITIAL_BALANCE: Balance = 10_000_000 * crate::benchmarking::xyk_liquidity_mining::ONE;

//TODO: this is use in many places, refactor
fn funded_account(name: &'static str, index: u32, assets: &[AssetId]) -> AccountId {
	let account: AccountId = account(name, index, 0);
	//Necessary to pay ED in insufficient asset
	Currencies::update_balance(
		RawOrigin::Root.into(),
		account.clone(),
		HDX,
		500_000_000_000_000_000_000i128,
	)
	.unwrap();

	for asset in assets {
		Currencies::update_balance(
			RawOrigin::Root.into(),
			account.clone(),
			*asset,
			500_000_000_000_000_000_000i128,
		)
		.unwrap();
	}
	account
}

fn fund_treasury() -> DispatchResult {
	let account = Treasury::account_id();

	Currencies::update_balance(
		RawOrigin::Root.into(),
		account.clone(),
		HDX,
		500_000_000_000_000_000_000i128,
	)
	.unwrap();
	Currencies::update_balance(
		RawOrigin::Root.into(),
		account.clone(),
		REWARD_CURRENCY,
		INITIAL_BALANCE as i128,
	)
	.unwrap();

	Ok(())
}

fn create_funded_account(name: &'static str, index: u32, balance: Balance, asset: AssetId) -> AccountId {
	let account: AccountId = account(name, index, 0);
	//Necessary to pay ED in insufficient asset
	Currencies::update_balance(RawOrigin::Root.into(), account.clone(), DAI, INITIAL_BALANCE as i128).unwrap();
	Currencies::update_balance(RawOrigin::Root.into(), account.clone(), LRNA, INITIAL_BALANCE as i128).unwrap();
	Currencies::update_balance(RawOrigin::Root.into(), account.clone(), 0, INITIAL_BALANCE as i128).unwrap();
	assert_ok!(Currencies::update_balance(
		RawOrigin::Root.into(),
		account.clone(),
		asset,
		balance.try_into().unwrap(),
	));
	account.clone()
}

fn initialize_global_farm(owner: AccountId) -> DispatchResult {
	OmnipoolLiquidityMining::create_global_farm(
		RawOrigin::Root.into(),
		G_FARM_TOTAL_REWARDS,
		BlockNumberFor::<crate::Runtime>::from(100_000_u32),
		BlockNumberFor::<crate::Runtime>::from(1_u32),
		REWARD_CURRENCY,
		owner,
		Perquintill::from_percent(20),
		1_000,
		FixedU128::one(),
	)?;

	seed_lm_pot()
}

fn initialize_yield_farm(owner: AccountId, id: GlobalFarmId, asset: AssetId) -> DispatchResult {
	OmnipoolLiquidityMining::create_yield_farm(RawOrigin::Signed(owner).into(), id, asset, FixedU128::one(), None)
}

fn initialize_omnipool(additional_asset: Option<AssetId>) -> DispatchResult {
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);

	let acc = Omnipool::protocol_account();

	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), DAI, stable_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), HDX, native_amount as Amount)?;

	fund(acc.clone(), HDX, 10_000_000_000_000_000 * ONE)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		HDX,
		native_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;
	Omnipool::add_token(
		RawOrigin::Root.into(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;

	let name = b"BSX".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	// Register new asset in asset registry
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			Balance::one(),
			None,
			None,
			None,
			None,
		))
	})?;
	let name = b"ETH".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			Balance::one(),
			None,
			None,
			None,
			None,
		))
	})?;
	let name = b"BTC".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			Balance::one(),
			None,
			None,
			None,
			None,
		))
	})?;

	let name = b"DOT".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			Balance::one(),
			None,
			None,
			None,
			None,
		))
	})?;

	// Create account for token provider and set balance
	let owner: AccountId = funded_account("owner2", 0, &[]);

	let token_price = FixedU128::from((1, 5));
	let token_amount = 200_000_000_000_000u128;

	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), BSX, token_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), ETH, token_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), BTC, token_amount as Amount)?;
	Currencies::update_balance(RawOrigin::Root.into(), acc.clone(), DOT, token_amount as Amount)?;

	// Add the token to the pool
	Omnipool::add_token(
		RawOrigin::Root.into(),
		BSX,
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		ETH,
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		BTC,
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	Omnipool::add_token(
		RawOrigin::Root.into(),
		DOT,
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	if let Some(asset_id) = additional_asset {
		Currencies::update_balance(
			RawOrigin::Root.into(),
			acc.clone(),
			asset_id,
			(token_amount * 100) as Amount,
		)?;
		Omnipool::add_token(
			RawOrigin::Root.into(),
			asset_id,
			token_price,
			Permill::from_percent(100),
			owner,
		)?;
	}

	//NOTE: This is necessary for oracle to provide price.
	set_period(10);

	do_lrna_hdx_trade()
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_hdx_trade() -> DispatchResult {
	let trader = funded_account("tmp_trader", 0, &[REWARD_CURRENCY]);

	fund(trader.clone(), LRNA, 100 * ONE)?;

	Omnipool::sell(RawOrigin::Signed(trader).into(), LRNA, HDX, ONE, 0)
}

fn seed_lm_pot() -> DispatchResult {
	let pot = OmnipoolWarehouseLM::pot_account_id().unwrap();

	fund(pot, HDX, 100 * ONE)
}

fn omnipool_add_liquidity(lp: AccountId, asset: AssetId, amount: Balance) -> Result<u128, DispatchError> {
	let current_position_id = Omnipool::next_position_id();

	Omnipool::add_liquidity(RawOrigin::Signed(lp).into(), asset, amount)?;

	Ok(current_position_id)
}

fn lm_deposit_shares(who: AccountId, g_id: GlobalFarmId, y_id: YieldFarmId, position_id: ItemId) -> DispatchResult {
	OmnipoolLiquidityMining::deposit_shares(RawOrigin::Signed(who).into(), g_id, y_id, position_id)
}

fn set_period(to: u32) {
	//NOTE: predefined global farm has period size = 1 block.
	while System::block_number() < to {
		let b = System::block_number();

		<pallet_circuit_breaker::Pallet<Runtime> as OnFinalize<BlockNumberFor<crate::Runtime>>>::on_finalize(b);
		<frame_system::Pallet<Runtime> as OnFinalize<BlockNumberFor<crate::Runtime>>>::on_finalize(b);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<crate::Runtime>>>::on_finalize(b);

		<pallet_circuit_breaker::Pallet<Runtime> as OnInitialize<BlockNumberFor<crate::Runtime>>>::on_initialize(
			b + 1_u32,
		);
		<frame_system::Pallet<Runtime> as OnInitialize<BlockNumberFor<crate::Runtime>>>::on_initialize(b + 1_u32);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<crate::Runtime>>>::on_initialize(b + 1_u32);

		System::set_block_number(b + 1_u32);
	}
}

runtime_benchmarks! {
	{Runtime, pallet_omnipool_liquidity_mining }

	create_global_farm {
		let planned_yielding_periods = BlockNumberFor::<crate::Runtime>::from(100_000_u32);
		let blocks_per_period = BlockNumberFor::<crate::Runtime>::from(100_u32);
		let reward_currency = register_asset(b"REW".to_vec(), 10000 * ONE).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let owner = funded_account("owner", 0, &[REWARD_CURRENCY]);
		let yield_per_period = Perquintill::from_percent(20);
		let min_deposit = 1_000;
		let price_adjustment = FixedU128::from(10_u128);

	}: _(RawOrigin::Root,  G_FARM_TOTAL_REWARDS, planned_yielding_periods, blocks_per_period, REWARD_CURRENCY, owner, yield_per_period, min_deposit, FixedU128::one())

	update_global_farm {
		let owner = funded_account("owner", 0, &[REWARD_CURRENCY]);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = funded_account("lp_1", 1, &[BTC]);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		set_period(100);
		lm_deposit_shares(lp, global_farm_id, yield_farm_id, position_id)?;
		set_period(200);

		let planned_yielding_periods = BlockNumberFor::<crate::Runtime>::from(100_000_u32);
		let yield_per_period = Perquintill::from_percent(20);
		let min_deposit = 1_000;

	}: _(RawOrigin::Root, global_farm_id, planned_yielding_periods, yield_per_period, min_deposit)


	terminate_global_farm {
		let owner = funded_account("owner", 0, &[REWARD_CURRENCY]);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = funded_account("lp_1", 1, &[BTC]);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		set_period(100);
		lm_deposit_shares(lp, global_farm_id, yield_farm_id, position_id)?;

		OmnipoolLiquidityMining::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, BTC)?;
		OmnipoolLiquidityMining::terminate_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, yield_farm_id, BTC)?;

		set_period(200);
	}: _(RawOrigin::Signed(owner), global_farm_id)


	create_yield_farm {
		fund_treasury().unwrap(); //To prevent BelowMinimum error

		let owner = funded_account("owner", 0, &[HDX, REWARD_CURRENCY]);
		let global_farm_id = 1;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = funded_account("lp_1", 1, &[BTC]);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		set_period(100);
		lm_deposit_shares(lp, global_farm_id, 2, position_id)?;

		set_period(1000);

	}:  _(RawOrigin::Signed(owner), global_farm_id, ETH, FixedU128::one(), Some(LoyaltyCurve::default()))


	update_yield_farm {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		lm_deposit_shares(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period(200);
	}: _(RawOrigin::Signed(owner), global_farm_id, BTC, FixedU128::from(2_u128))

	stop_yield_farm {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		lm_deposit_shares(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period(200);
	}: _(RawOrigin::Signed(owner), global_farm_id, BTC)

	resume_yield_farm {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let global_farm_id = 1;
		let eth_farm_id = 2;
		let btc_farm_id = 3;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, ETH)?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		OmnipoolLiquidityMining::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, ETH)?;

		set_period(200);

		lm_deposit_shares(lp, global_farm_id, btc_farm_id, position_id)?;

		set_period(400);
	}: _(RawOrigin::Signed(owner), global_farm_id, eth_farm_id, ETH, FixedU128::from(2))

	terminate_yield_farm {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner.clone(), global_farm_id, BTC)?;

		let lp = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let position_id = omnipool_add_liquidity(lp.clone(), BTC, 10 * BTC_ONE)?;

		lm_deposit_shares(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period(200);

		OmnipoolLiquidityMining::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, BTC)?;

		set_period(300);
	}: _(RawOrigin::Signed(owner), global_farm_id, yield_farm_id, BTC)

	deposit_shares {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, global_farm_id, BTC)?;

		let lp1 = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;

		lm_deposit_shares(lp1, global_farm_id, yield_farm_id, lp1_position_id)?;

		let lp2 = create_funded_account("lp_2", 1, 10 * BTC_ONE, BTC);
		let lp2_position_id = omnipool_add_liquidity(lp2.clone(), BTC, 10 * BTC_ONE)?;
		set_period(200);


	}: _(RawOrigin::Signed(lp2), global_farm_id, yield_farm_id, lp2_position_id)


	redeposit_shares {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(None)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, BTC)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, BTC)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, BTC)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, BTC)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, BTC)?;

		let lp1 = create_funded_account("lp_1", 5, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;

		let lp2 = create_funded_account("lp_2", 6, 1_000 * ONE, BTC);
		let lp2_position_id = omnipool_add_liquidity(lp2.clone(), BTC, 10 * BTC_ONE)?;

		set_period(200);

		lm_deposit_shares(lp1.clone(), 1, 2, lp1_position_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, deposit_id)?;

		//Deposit into the global-farm so it will be updated
		lm_deposit_shares(lp2, 9, 10, lp2_position_id)?;

		set_period(400);
	}: _(RawOrigin::Signed(lp1), 9, 10, deposit_id)

	claim_rewards {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(None)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, BTC)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, BTC)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, BTC)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, BTC)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, BTC)?;

		let lp1 = create_funded_account("lp_1", 5, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;

		//NOTE: This is necessary because paid rewards are lower than ED.
		fund(lp1.clone(), REWARD_CURRENCY, 100 * ONE)?;

		set_period(200);

		lm_deposit_shares(lp1.clone(), 1, 2, lp1_position_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 9, 10, deposit_id)?;

		set_period(400);

	}:  _(RawOrigin::Signed(lp1), deposit_id, 10)

	withdraw_shares {
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let global_farm_id = 1;
		let yield_farm_id = 2;
		let deposit_id = 1;

		initialize_omnipool(None)?;

		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, global_farm_id, BTC)?;

		let lp1 = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;

		//NOTE: This is necessary because paid rewards are lower than ED.
		fund(lp1.clone(), REWARD_CURRENCY, 100 * ONE)?;

		set_period(200);

		lm_deposit_shares(lp1.clone(), 1, 2, lp1_position_id)?;
		set_period(400);
	}: _(RawOrigin::Signed(lp1), deposit_id, yield_farm_id)

	join_farms {
		let c in 1..get_max_entries();

		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(None)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, BTC)?;
		let lp1 = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp1, 1, 2, lp1_position_id)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, BTC)?;
		let lp2 = create_funded_account("lp_2", 1, 10 * BTC_ONE, BTC);
		let lp2_position_id = omnipool_add_liquidity(lp2.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp2, 3, 4, lp2_position_id)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, BTC)?;
		let lp3 = create_funded_account("lp_3", 1, 10 * BTC_ONE, BTC);
		let lp3_position_id = omnipool_add_liquidity(lp3.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp3, 5, 6, lp3_position_id)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, BTC)?;
		let lp4 = create_funded_account("lp_4", 1, 10 * BTC_ONE, BTC);
		let lp4_position_id = omnipool_add_liquidity(lp4.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp4, 7, 8, lp4_position_id)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, BTC)?;
		let lp5 = create_funded_account("lp_5", 1, 10 * BTC_ONE, BTC);
		let lp5_position_id = omnipool_add_liquidity(lp5.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp5, 9, 10, lp5_position_id)?;

		let lp6 = create_funded_account("lp_6", 5, 10 * BTC_ONE, BTC);
		let lp6_position_id = omnipool_add_liquidity(lp6.clone(), BTC, 10 * BTC_ONE)?;

		set_period(200);
		let farms_entries = [(1,2), (3,4), (5,6), (7,8), (9, 10)];
		let farms = farms_entries[0..c as usize].to_vec();

	}: _(RawOrigin::Signed(lp6), farms.try_into().unwrap(), lp6_position_id)

	add_liquidity_and_join_farms {
		let c in 1..get_max_entries();

		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(None)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, BTC)?;
		let lp1 = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp1, 1, 2, lp1_position_id)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, BTC)?;
		let lp2 = create_funded_account("lp_2", 1, 10 * BTC_ONE, BTC);
		let lp2_position_id = omnipool_add_liquidity(lp2.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp2, 3, 4, lp2_position_id)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, BTC)?;
		let lp3 = create_funded_account("lp_3", 1, 10 * BTC_ONE, BTC);
		let lp3_position_id = omnipool_add_liquidity(lp3.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp3, 5, 6, lp3_position_id)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, BTC)?;
		let lp4 = create_funded_account("lp_4", 1, 10 * BTC_ONE, BTC);
		let lp4_position_id = omnipool_add_liquidity(lp4.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp4, 7, 8, lp4_position_id)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, BTC)?;
		let lp5 = create_funded_account("lp_5", 1, 10 * BTC_ONE, BTC);
		let lp5_position_id = omnipool_add_liquidity(lp5.clone(), BTC, 10 * BTC_ONE)?;
		lm_deposit_shares(lp5, 9, 10, lp5_position_id)?;

		let lp6 = create_funded_account("lp_6", 5, 10 * BTC_ONE, BTC);

		set_period(200);
		let farms_entries = [(1,2), (3,4), (5,6), (7,8), (9, 10)];
		let farms = farms_entries[0..c as usize].to_vec();

	}: _(RawOrigin::Signed(lp6), farms.try_into().unwrap(), BTC, 10 * BTC_ONE, Some(10 * BTC_ONE))

	exit_farms {
		let c in 1..get_max_entries();

		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(None)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, BTC)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, BTC)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, BTC)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, BTC)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, BTC)?;

		let lp1 = create_funded_account("lp_1", 5, 10 * BTC_ONE, BTC);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), BTC, BTC_ONE)?;

		set_period(200);

		lm_deposit_shares(lp1.clone(), 1, 2, lp1_position_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, deposit_id)?;
		OmnipoolLiquidityMining::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 9, 10, deposit_id)?;

		let deposit_id = 1;
		let farm_entries = [2,4,6,8,10];
		let farms = farm_entries[0..c as usize].to_vec();

		set_period(250);
	}: _(RawOrigin::Signed(lp1),deposit_id, farms.try_into().unwrap())

	add_liquidity_stableswap_omnipool_and_join_farms  {
		let c in 1..get_max_entries();

		//Init stableswap first
		let caller: AccountId = account("caller", 0, 1);
		let lp_provider: AccountId = account("provider", 0, 1);
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<AssetId>> = vec![];
		let mut asset_ids: Vec<AssetId> = Vec::new() ;
		for idx in 0..pallet_stableswap::MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			let asset_id = register_asset_with_decimals(
				name,
				1u128,
				18u8
			).unwrap();
			asset_ids.push(asset_id);
			Currencies::update_balance(RawOrigin::Root.into(), caller.clone(),asset_id,  1_000_000_000_000_000i128)?;
			Currencies::update_balance(RawOrigin::Root.into(), lp_provider.clone(),asset_id, 1_000_000_000_000_000_000_000i128)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}

		let name : Vec<u8> = b"PO2".to_vec();
		let pool_id = register_asset_with_decimals(
			name,
			1u128,
			18u8
		).unwrap();

		let amplification = 100u16;
		let trade_fee = Permill::from_percent(1);
		let successful_origin = <crate::Runtime as pallet_stableswap::Config>::AuthorityOrigin::try_successful_origin().unwrap();
		Stableswap::create_pool(successful_origin,
			pool_id,
			asset_ids,
			amplification,
			trade_fee,
		)?;

		// Worst case is adding additional liquidity and not initial liquidity
		Stableswap::add_liquidity(RawOrigin::Signed(caller).into(),
			pool_id,
			initial.try_into().unwrap(),
		)?;

		let lp1 = create_funded_account("lp_1", 1, 10 * BTC_ONE, BTC);
		let deposit_id = 1;

		//Init LM farms
		let owner = create_funded_account("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner2 = create_funded_account("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner3 = create_funded_account("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner4 = create_funded_account("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);
		let owner5 = create_funded_account("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY);

		let deposit_id = 1;

		initialize_omnipool(Some(pool_id))?;

		CircuitBreaker::set_add_liquidity_limit(RuntimeOrigin::root(), pool_id, Some((99, 100))).unwrap();
		let liquidity_added = 100_000_000_000_000_u128;
		let omni_lp_provider: AccountId = create_funded_account("provider", 1, liquidity_added * 10, pool_id);
		Omnipool::add_liquidity(RawOrigin::Signed(omni_lp_provider.clone()).into(), pool_id, liquidity_added)?;

		//gId: 1, yId: 2
		initialize_global_farm(owner.clone())?;
		initialize_yield_farm(owner, 1, pool_id)?;
		let lp1 = create_funded_account("lp_1", 1, 10 * ONE, pool_id);
		let lp1_position_id = omnipool_add_liquidity(lp1.clone(), pool_id, 10 * ONE)?;
		lm_deposit_shares(lp1, 1, 2, lp1_position_id)?;

		//gId: 3, yId: 4
		initialize_global_farm(owner2.clone())?;
		initialize_yield_farm(owner2, 3, pool_id)?;
		let lp2 = create_funded_account("lp_2", 1, 10 * ONE, pool_id);
		let lp2_position_id = omnipool_add_liquidity(lp2.clone(), pool_id, 10 * ONE)?;
		lm_deposit_shares(lp2, 3, 4, lp2_position_id)?;

		//gId: 5, yId: 6
		initialize_global_farm(owner3.clone())?;
		initialize_yield_farm(owner3, 5, pool_id)?;
		let lp3 = create_funded_account("lp_3", 1, 10 * ONE, pool_id);
		let lp3_position_id = omnipool_add_liquidity(lp3.clone(), pool_id, 10 * ONE)?;
		lm_deposit_shares(lp3, 5, 6, lp3_position_id)?;

		//gId: 7, yId: 8
		initialize_global_farm(owner4.clone())?;
		initialize_yield_farm(owner4, 7, pool_id)?;
		let lp4 = create_funded_account("lp_4", 1, 10 * ONE, pool_id);
		let lp4_position_id = omnipool_add_liquidity(lp4.clone(), pool_id, 10 * ONE)?;
		lm_deposit_shares(lp4, 7, 8, lp4_position_id)?;

		//gId: 9, yId: 10
		initialize_global_farm(owner5.clone())?;
		initialize_yield_farm(owner5, 9, pool_id)?;
		let lp5 = create_funded_account("lp_5", 1, 10 * ONE, pool_id);
		let lp5_position_id = omnipool_add_liquidity(lp5.clone(), pool_id, 10 * ONE)?;
		lm_deposit_shares(lp5, 9, 10, lp5_position_id)?;

		let lp6 = create_funded_account("lp_6", 5, 10 * ONE, pool_id);

		set_period(200);
		let farms_entries = [(1,2), (3,4), (5,6), (7,8), (9, 10)];
		let farms = farms_entries[0..c as usize].to_vec();

	}: _(RawOrigin::Signed(lp_provider),pool_id, added_liquidity.try_into().unwrap(), Some(farms.try_into().unwrap()))

}

fn get_max_entries() -> u32 {
	<Runtime as pallet_omnipool_liquidity_mining::Config>::MaxFarmEntriesPerDeposit::get() as u32
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(
					Some(1),
					Some(b"LRNA".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
				(
					Some(2),
					Some(b"DAI".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
			],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		<pallet_omnipool_liquidity_mining::GenesisConfig<crate::Runtime> as BuildStorage>::assimilate_storage(
			&pallet_omnipool_liquidity_mining::GenesisConfig::<crate::Runtime>::default(),
			&mut t,
		)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
