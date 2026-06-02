// This file is part of HydraDX-node

// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg(feature = "runtime-benchmarks")]

use crate::{AccountId, AssetId, Balance, Currencies, FeeProcessor, Omnipool, Runtime};

use crate::benchmarking::set_period;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_runtime::DispatchResult;
use sp_std::vec;

const HDX: AssetId = 0;
const DAI: AssetId = 2;
const ONE: Balance = 1_000_000_000_000;

fn fund(who: AccountId, asset: AssetId, amount: Balance) -> DispatchResult {
	Currencies::update_balance(RawOrigin::Root.into(), who, asset, amount as i128)
}

runtime_benchmarks! {
	{ Runtime, pallet_fee_processor }

	// Worst case: a non-HDX pot balance is sold via Omnipool and distributed to all four
	// receivers (the slice routed to the referrals receiver is the heaviest leg).
	convert {
		crate::benchmarking::omnipool_liquidity_mining::initialize_omnipool(None)?;

		// Pre-create every account the conversion touches (receiver pots, treasury, referrals
		// pot) so that distributing to a not-yet-existing account — including the tiny fee-on-fee
		// slices from the Omnipool sell itself — never trips ED. On the live chain these are all
		// genesis-funded.
		let seed = 1_000 * ONE;
		for p in [
			FeeProcessor::pot_account_id(),
			pallet_staking::Pallet::<Runtime>::pot_account_id(),
			pallet_gigahdx::Pallet::<Runtime>::gigapot_account_id(),
			pallet_gigahdx_rewards::Pallet::<Runtime>::reward_accumulator_pot(),
			pallet_referrals::Pallet::<Runtime>::pot_account_id(),
			crate::Treasury::account_id(),
		] {
			fund(p, HDX, seed)?;
		}

		// Warm the EMA oracle with HDX/DAI trades across a period — the conversion's Omnipool
		// sell and the dynamic fee both read the oracle. Each HDX->DAI sell also accrues a DAI
		// fee into the fee-processor pot (the non-HDX path), which is what `convert` consumes.
		let trader: AccountId = account("trader", 0, 0);
		fund(trader.clone(), HDX, 100_000 * ONE)?;
		Omnipool::sell(RawOrigin::Signed(trader.clone()).into(), HDX, DAI, 100 * ONE, 0)?;
		set_period(24);
		Omnipool::sell(RawOrigin::Signed(trader.clone()).into(), HDX, DAI, 100 * ONE, 0)?;

		let pot = FeeProcessor::pot_account_id();
		fund(pot.clone(), DAI, 100 * ONE)?;

		let caller: AccountId = account("caller", 1, 0);
		fund(caller.clone(), HDX, seed)?;
	}: _(RawOrigin::Signed(caller), DAI)
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<AccountId>>::free_balance(DAI, &FeeProcessor::pot_account_id()),
			0,
			"the entire pot balance must be converted"
		);
	}
}

// NOTE: no `impl_benchmark_test_suite!` here. `convert` drives a real Omnipool sell whose
// downstream fee distribution touches accounts that only exist in the full chain genesis, so it
// is generated/validated via the benchmarking CLI against the runtime genesis rather than a
// minimal stand-alone externalities.
