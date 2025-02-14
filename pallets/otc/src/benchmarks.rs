// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
#![cfg(feature = "runtime-benchmarks")]
use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_traits::{AssetKind, Create};
use orml_traits::MultiCurrencyExtended;
use sp_std::vec;
use sp_std::vec::Vec;
pub const ONE: Balance = 1_000_000_000_000;

benchmarks! {
	where_clause { where
		T::AssetId: From<u32>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config,
		u32: From<<T as pallet::Config>::AssetId>,
		T::AssetRegistry: Create<Balance, Error=DispatchError, AssetId = T::AssetId>
	}
  place_order {
		let (dot, dai) = seed_registry::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1, vec!(dot, dai))?;
  }:  _(RawOrigin::Signed(owner.clone()), dai.into(), dot.into(), 20 * ONE, 100 * ONE, true)
	verify {
		assert_eq!(T::Currency::reserved_balance_named(&NAMED_RESERVE_ID, dot.into(), &owner), 100 * ONE);
	}

	partial_fill_order {
		let (dot, dai) = seed_registry::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1, vec!(dot, dai))?;
		let filler: T::AccountId = create_account_with_balances::<T>("filler", 2, vec!(dot, dai))?;

		assert_ok!(
			crate::Pallet::<T>::place_order(RawOrigin::Signed(owner.clone()).into(), dai.into(), dot.into(), 20 * ONE, 100 * ONE, true)
		);
  }:  _(RawOrigin::Signed(filler.clone()), 0u32, 10 * ONE)
	verify {
		assert_eq!(T::Currency::reserved_balance_named(&NAMED_RESERVE_ID, dot.into(), &owner), 50 * ONE);
	}

	fill_order {
		let (dot, dai) = seed_registry::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1, vec!(dot, dai))?;
		let filler: T::AccountId = create_account_with_balances::<T>("filler", 2, vec!(dot, dai))?;

		assert_ok!(
			crate::Pallet::<T>::place_order(RawOrigin::Signed(owner.clone()).into(), dai.into(), dot.into(), 20 * ONE, 100 * ONE, true)
		);
  }:  _(RawOrigin::Signed(filler.clone()), 0u32)
	verify {
		assert_eq!(T::Currency::reserved_balance_named(&NAMED_RESERVE_ID, dot.into(), &owner), 0);
	}

	cancel_order {
		let (dot, dai) = seed_registry::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1, vec!(dot, dai))?;
		assert_ok!(
			crate::Pallet::<T>::place_order(RawOrigin::Signed(owner.clone()).into(), dai.into(), dot.into(), 20 * ONE, 100 * ONE, true)
		);
  }:  _(RawOrigin::Signed(owner.clone()), 0u32)
	verify {
		assert_eq!(T::Currency::reserved_balance_named(&NAMED_RESERVE_ID, dot.into(), &owner), 0);
	}
}

fn seed_registry<T: Config>() -> Result<(u32, u32), DispatchError>
where
	u32: From<<T as pallet::Config>::AssetId>,
	T::AssetRegistry: Create<Balance, Error = DispatchError>,
{
	use frame_support::storage::with_transaction;
	use sp_runtime::TransactionOutcome;

	// Register new asset in asset registry
	let name = b"DOT".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	let dot = with_transaction(|| {
		TransactionOutcome::Commit(<T as crate::Config>::AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			ONE,
			None,
			None,
			None,
			None,
		))
	})?;
	let name = b"DAI".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	let dai = with_transaction(|| {
		TransactionOutcome::Commit(<T as crate::Config>::AssetRegistry::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			ONE,
			None,
			None,
			None,
			None,
		))
	})?;

	Ok((dot.into(), dai.into()))
}

fn create_account_with_balances<T: Config>(
	name: &'static str,
	index: u32,
	assets: Vec<u32>,
) -> Result<T::AccountId, DispatchError>
where
	T::AssetId: From<u32>,
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
{
	let account_id: T::AccountId = account(name, index, index);

	let token_amount: Balance = 200 * ONE;

	for asset in assets.iter() {
		T::Currency::update_balance((*asset).into(), &account_id, token_amount as i128)?;
	}

	Ok(account_id)
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
