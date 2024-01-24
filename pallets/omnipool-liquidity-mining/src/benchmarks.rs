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

use crate::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::{Pallet as System, RawOrigin};
use hydradx_traits::Registry;
use orml_traits::MultiCurrencyExtended;
use pallet_liquidity_mining::Instance1;
use primitives::AssetId;
use sp_runtime::{traits::One, FixedU128, Permill};

const ONE: Balance = 1_000_000_000_000;
const BTC_ONE: Balance = 100_000_000;
const HDX: AssetId = 0;
const LRNA: AssetId = 1;
const DAI: AssetId = 2;
const BSX: AssetId = 1_000_001;
const ETH: AssetId = 1_000_002;
const BTC: AssetId = 1_000_003;

const G_FARM_TOTAL_REWARDS: Balance = 10_000_000 * ONE;
const REWARD_CURRENCY: AssetId = HDX;

type CurrencyOf<T> = <T as pallet::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;

fn fund<T: Config>(to: T::AccountId, currency: T::AssetId, amount: Balance) -> DispatchResult {
	CurrencyOf::<T>::deposit(currency, &to, amount)
}

const SEED: u32 = 0;
fn create_funded_account<T: Config>(
	name: &'static str,
	index: u32,
	amount: Balance,
	currency: T::AssetId,
) -> T::AccountId
where
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let caller: T::AccountId = account(name, index, SEED);

	fund::<T>(caller.clone(), currency, amount).unwrap();

	caller
}

fn initialize_global_farm<T: Config>(owner: T::AccountId) -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	T: pallet_liquidity_mining::Config<Instance1>,
{
	Pallet::<T>::create_global_farm(
		RawOrigin::Root.into(),
		G_FARM_TOTAL_REWARDS,
		BlockNumberFor::<T>::from(100_000_u32),
		BlockNumberFor::<T>::from(1_u32),
		REWARD_CURRENCY.into(),
		owner,
		Perquintill::from_percent(20),
		1_000,
		FixedU128::one(),
	)?;

	seed_lm_pot::<T>()
}

fn initialize_yield_farm<T: Config>(owner: T::AccountId, id: GlobalFarmId, asset: T::AssetId) -> DispatchResult
where
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	Pallet::<T>::create_yield_farm(RawOrigin::Signed(owner).into(), id, asset, FixedU128::one(), None)
}

fn initialize_omnipool<T: Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: pallet_ema_oracle::Config,
	T::AssetId: From<u32>,
{
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);

	let acc = OmnipoolPallet::<T>::protocol_account();

	<T as pallet_omnipool::Config>::Currency::update_balance(DAI.into(), &acc, stable_amount as i128)?;
	<T as pallet_omnipool::Config>::Currency::update_balance(HDX.into(), &acc, native_amount as i128)?;

	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		HDX.into(),
		native_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;
	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		DAI.into(),
		stable_price,
		Permill::from_percent(100),
		acc.clone(),
	)?;

	// Register new asset in asset registry
	T::AssetRegistry::create_asset(&b"BSX".to_vec(), Balance::one())?;
	T::AssetRegistry::create_asset(&b"ETH".to_vec(), Balance::one())?;
	T::AssetRegistry::create_asset(&b"BTC".to_vec(), Balance::one())?;

	// Create account for token provider and set balance
	let owner: T::AccountId = account("owner", 0, 1);

	let token_price = FixedU128::from((1, 5));
	let token_amount = 200_000_000_000_000u128;

	<T as pallet_omnipool::Config>::Currency::update_balance(BSX.into(), &acc, token_amount as i128)?;
	<T as pallet_omnipool::Config>::Currency::update_balance(ETH.into(), &acc, token_amount as i128)?;
	<T as pallet_omnipool::Config>::Currency::update_balance(BTC.into(), &acc, token_amount as i128)?;

	// Add the token to the pool
	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		BSX.into(),
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		ETH.into(),
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)?;

	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		BTC.into(),
		token_price,
		Permill::from_percent(100),
		owner,
	)?;

	//NOTE: This is necessary for oracle to provide price.
	set_period::<T>(10);

	do_lrna_hdx_trade::<T>()
}

//NOTE: This is necessary for oracle to provide price.
fn do_lrna_hdx_trade<T: Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T::AssetId: From<u32>,
{
	let trader = create_funded_account::<T>("tmp_trader", 0, 100 * ONE, REWARD_CURRENCY.into());

	fund::<T>(trader.clone(), LRNA.into(), 100 * ONE)?;

	OmnipoolPallet::<T>::sell(RawOrigin::Signed(trader).into(), LRNA.into(), HDX.into(), ONE, 0)
}

fn seed_lm_pot<T: Config>() -> DispatchResult
where
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	T: pallet_liquidity_mining::Config<Instance1>,
{
	let pot = pallet_liquidity_mining::Pallet::<T, Instance1>::pot_account_id().unwrap();

	fund::<T>(pot, HDX.into(), 100 * ONE)
}

fn omnipool_add_liquidity<T: Config>(
	lp: T::AccountId,
	asset: T::AssetId,
	amount: Balance,
) -> Result<u128, DispatchError> {
	let current_position_id = OmnipoolPallet::<T>::next_position_id();

	OmnipoolPallet::<T>::add_liquidity(RawOrigin::Signed(lp).into(), asset, amount)?;

	Ok(current_position_id)
}

fn lm_deposit_shares<T: Config>(
	who: T::AccountId,
	g_id: GlobalFarmId,
	y_id: YieldFarmId,
	position_id: T::PositionItemId,
) -> DispatchResult {
	crate::Pallet::<T>::deposit_shares(RawOrigin::Signed(who).into(), g_id, y_id, position_id)
}

fn set_period<T: Config>(to: u32)
where
	T: pallet_ema_oracle::Config,
{
	//NOTE: predefined global farm has period size = 1 block.

	while System::<T>::block_number() < to.into() {
		let b = System::<T>::block_number();

		System::<T>::on_finalize(b);
		pallet_ema_oracle::Pallet::<T>::on_finalize(b);

		System::<T>::on_initialize(b + 1_u32.into());
		pallet_ema_oracle::Pallet::<T>::on_initialize(b + 1_u32.into());

		System::<T>::set_block_number(b + 1_u32.into());
	}
}

benchmarks! {
	where_clause { where
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config + pallet_ema_oracle::Config + pallet_liquidity_mining::Config<Instance1>,
	}

	create_global_farm {
		let planned_yielding_periods = BlockNumberFor::<T>::from(100_000_u32);
		let blocks_per_period = BlockNumberFor::<T>::from(100_u32);
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let yield_per_period = Perquintill::from_percent(20);
		let min_deposit = 1_000;
		let price_adjustment = FixedU128::from(10_u128);

	}: _(RawOrigin::Root,  G_FARM_TOTAL_REWARDS, planned_yielding_periods, blocks_per_period, REWARD_CURRENCY.into(), owner, yield_per_period, min_deposit, FixedU128::one())

	terminate_global_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		set_period::<T>(100);
		lm_deposit_shares::<T>(lp, global_farm_id, yield_farm_id, position_id)?;

		crate::Pallet::<T>::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, BTC.into())?;
		crate::Pallet::<T>::terminate_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, yield_farm_id, BTC.into())?;

		set_period::<T>(200);
	}: _(RawOrigin::Signed(owner), global_farm_id)


	create_yield_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		set_period::<T>(100);
		lm_deposit_shares::<T>(lp, global_farm_id, 2, position_id)?;

		set_period::<T>(200);
	}: _(RawOrigin::Signed(owner), global_farm_id, ETH.into(), FixedU128::one(), Some(LoyaltyCurve::default()))


	update_yield_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		lm_deposit_shares::<T>(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period::<T>(200);
	}: _(RawOrigin::Signed(owner), global_farm_id, BTC.into(), FixedU128::from(2_u128))

	stop_yield_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		lm_deposit_shares::<T>(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period::<T>(200);
	}: _(RawOrigin::Signed(owner), global_farm_id, BTC.into())

	resume_yield_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let eth_farm_id = 2;
		let btc_farm_id = 3;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, ETH.into())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		crate::Pallet::<T>::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, ETH.into())?;

		set_period::<T>(200);

		lm_deposit_shares::<T>(lp, global_farm_id, btc_farm_id, position_id)?;

		set_period::<T>(400);
	}: _(RawOrigin::Signed(owner), global_farm_id, eth_farm_id, ETH.into(), FixedU128::from(2))

	terminate_yield_farm {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner.clone(), global_farm_id, BTC.into())?;

		let lp = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let position_id = omnipool_add_liquidity::<T>(lp.clone(), BTC.into(), 10 * BTC_ONE)?;

		lm_deposit_shares::<T>(lp, global_farm_id, yield_farm_id, position_id)?;

		set_period::<T>(200);

		crate::Pallet::<T>::stop_yield_farm(RawOrigin::Signed(owner.clone()).into(), global_farm_id, BTC.into())?;

		set_period::<T>(300);
	}: _(RawOrigin::Signed(owner), global_farm_id, yield_farm_id, BTC.into())

	deposit_shares {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let global_farm_id = 1;
		let yield_farm_id = 2;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner, global_farm_id, BTC.into())?;

		let lp1 = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let lp1_position_id = omnipool_add_liquidity::<T>(lp1.clone(), BTC.into(), 10 * BTC_ONE)?;

		lm_deposit_shares::<T>(lp1, global_farm_id, yield_farm_id, lp1_position_id)?;

		let lp2 = create_funded_account::<T>("lp_2", 1, 10 * BTC_ONE, BTC.into());
		let lp2_position_id = omnipool_add_liquidity::<T>(lp2.clone(), BTC.into(), 10 * BTC_ONE)?;
		set_period::<T>(200);


	}: _(RawOrigin::Signed(lp2), global_farm_id, yield_farm_id, lp2_position_id)


	redeposit_shares {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner2 = create_funded_account::<T>("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner3 = create_funded_account::<T>("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner4 = create_funded_account::<T>("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner5 = create_funded_account::<T>("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());

		let deposit_id = 1;

		initialize_omnipool::<T>()?;

		//gId: 1, yId: 2
		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner, 1, BTC.into())?;

		//gId: 3, yId: 4
		initialize_global_farm::<T>(owner2.clone())?;
		initialize_yield_farm::<T>(owner2, 3, BTC.into())?;

		//gId: 5, yId: 6
		initialize_global_farm::<T>(owner3.clone())?;
		initialize_yield_farm::<T>(owner3, 5, BTC.into())?;

		//gId: 7, yId: 8
		initialize_global_farm::<T>(owner4.clone())?;
		initialize_yield_farm::<T>(owner4, 7, BTC.into())?;

		//gId: 9, yId: 10
		initialize_global_farm::<T>(owner5.clone())?;
		initialize_yield_farm::<T>(owner5, 9, BTC.into())?;

		let lp1 = create_funded_account::<T>("lp_1", 5, 10 * BTC_ONE, BTC.into());
		let lp1_position_id = omnipool_add_liquidity::<T>(lp1.clone(), BTC.into(), 10 * BTC_ONE)?;

		let lp2 = create_funded_account::<T>("lp_2", 6, 1_000 * ONE, BTC.into());
		let lp2_position_id = omnipool_add_liquidity::<T>(lp2.clone(), BTC.into(), 10 * BTC_ONE)?;

		set_period::<T>(200);

		lm_deposit_shares::<T>(lp1.clone(), 1, 2, lp1_position_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, deposit_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, deposit_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, deposit_id)?;

		//Deposit into the global-farm so it will be updated
		lm_deposit_shares::<T>(lp2, 9, 10, lp2_position_id)?;

		set_period::<T>(400);
	}: _(RawOrigin::Signed(lp1), 9, 10, deposit_id)

	claim_rewards {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner2 = create_funded_account::<T>("owner2", 1, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner3 = create_funded_account::<T>("owner3", 2, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner4 = create_funded_account::<T>("owner4", 3, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());
		let owner5 = create_funded_account::<T>("owner5", 4, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());

		let deposit_id = 1;

		initialize_omnipool::<T>()?;

		//gId: 1, yId: 2
		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner, 1, BTC.into())?;

		//gId: 3, yId: 4
		initialize_global_farm::<T>(owner2.clone())?;
		initialize_yield_farm::<T>(owner2, 3, BTC.into())?;

		//gId: 5, yId: 6
		initialize_global_farm::<T>(owner3.clone())?;
		initialize_yield_farm::<T>(owner3, 5, BTC.into())?;

		//gId: 7, yId: 8
		initialize_global_farm::<T>(owner4.clone())?;
		initialize_yield_farm::<T>(owner4, 7, BTC.into())?;

		//gId: 9, yId: 10
		initialize_global_farm::<T>(owner5.clone())?;
		initialize_yield_farm::<T>(owner5, 9, BTC.into())?;

		let lp1 = create_funded_account::<T>("lp_1", 5, 10 * BTC_ONE, BTC.into());
		let lp1_position_id = omnipool_add_liquidity::<T>(lp1.clone(), BTC.into(), 10 * BTC_ONE)?;

		//NOTE: This is necessary because paid rewards are lower than ED.
		fund::<T>(lp1.clone(), REWARD_CURRENCY.into(), 100 * ONE)?;

		set_period::<T>(200);

		lm_deposit_shares::<T>(lp1.clone(), 1, 2, lp1_position_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 3, 4, deposit_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 5, 6, deposit_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 7, 8, deposit_id)?;
		crate::Pallet::<T>::redeposit_shares(RawOrigin::Signed(lp1.clone()).into(), 9, 10, deposit_id)?;

		set_period::<T>(400);

	}: _(RawOrigin::Signed(lp1), deposit_id, 10)

	withdraw_shares {
		let owner = create_funded_account::<T>("owner", 0, G_FARM_TOTAL_REWARDS, REWARD_CURRENCY.into());

		let global_farm_id = 1;
		let yield_farm_id = 2;
		let deposit_id = 1;

		initialize_omnipool::<T>()?;

		initialize_global_farm::<T>(owner.clone())?;
		initialize_yield_farm::<T>(owner, global_farm_id, BTC.into())?;

		let lp1 = create_funded_account::<T>("lp_1", 1, 10 * BTC_ONE, BTC.into());
		let lp1_position_id = omnipool_add_liquidity::<T>(lp1.clone(), BTC.into(), 10 * BTC_ONE)?;

		//NOTE: This is necessary because paid rewards are lower than ED.
		fund::<T>(lp1.clone(), REWARD_CURRENCY.into(), 100 * ONE)?;

		set_period::<T>(200);

		lm_deposit_shares::<T>(lp1.clone(), 1, 2, lp1_position_id)?;
		set_period::<T>(400);
	}: _(RawOrigin::Signed(lp1), deposit_id, yield_farm_id)

	impl_benchmark_test_suite!(Pallet, crate::tests::mock::ExtBuilder::default().build(), crate::tests::mock::Test);
}
