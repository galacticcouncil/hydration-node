// Copyright (C) 2020-2024  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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

use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::fungibles::Mutate;
use frame_system::RawOrigin;
use hydradx_traits::{router::AssetPair, AssetKind, Create};

pub const ONE: Balance = 1_000_000_000_000;

benchmarks! {
	where_clause { where
		AssetId: From<u32>,
		<T as Config>::Currency: Mutate<T::AccountId, AssetId = AssetId, Balance = Balance>,
		T: Config,
		T: pallet_evm_accounts::Config,
		T: pallet_asset_registry::Config,
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
		AssetId: From<<T as pallet_asset_registry::Config>::AssetId>,
		<T as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	}

	liquidate {
		let hdx = 0;
		let dot = seed_registry::<T>()?;
		let caller: T::AccountId = account("acc", 1, 1);
		pallet_evm_accounts::Pallet::<T>::bind_evm_address(RawOrigin::Signed(Pallet::<T>::account_id()).into())?;
		let evm_address = pallet_evm_accounts::Pallet::<T>::evm_address(&caller);

		<T as Config>::Currency::set_balance(hdx, &Pallet::<T>::account_id(), 1_000_000_000 * ONE);
		<T as Config>::Currency::set_balance(dot, &Pallet::<T>::account_id(), 1_000_000_000 * ONE);

		// when this benchmark is executed as a test, it uses EvmMock which simply transfers assets
		// to/from the provided contract address. Send some funds to the address so it can work.
		let mm_contract_address = EvmAddress::from_slice(hex_literal::hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38").as_slice());
		let mm_account = pallet_evm_accounts::Pallet::<T>::account_id(mm_contract_address);
		<T as Config>::Currency::set_balance(hdx, &mm_account, 1_000_000_000 * ONE);

		let route = <T as Config>::Router::get_route(AssetPair {
			asset_in: hdx,
			asset_out: dot,
		});

	}:  _(RawOrigin::Signed(caller), hdx, dot, evm_address, 100 * ONE, route)

	set_borrowing_contract {
		let address = EvmAddress::from_slice(hex_literal::hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38").as_slice());
	}: _(RawOrigin::Root, address)

	impl_benchmark_test_suite!(Pallet, tests::mock::ExtBuilder::default().build(), tests::mock::Test);
}

#[allow(clippy::multiple_bound_locations)]
fn seed_registry<T: Config>() -> Result<AssetId, DispatchError>
where
	T: pallet_asset_registry::Config,
	AssetId: From<<T as pallet_asset_registry::Config>::AssetId>,
	<T as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	use frame_support::{sp_runtime::TransactionOutcome, storage::with_transaction};

	// Register new asset in asset registry
	let name = b"DOT".to_vec().try_into().map_err(|_| "BoundedConvertionFailed")?;
	let dot = with_transaction(|| {
		TransactionOutcome::Commit(pallet_asset_registry::Pallet::<T>::register_sufficient_asset(
			None,
			Some(name),
			AssetKind::Token,
			ONE,
			None,
			None,
			None,
			None,
		))
		// When running as a benchmarking test, this fails because the asset is already registered.
		// Set it to the asset id configured in the mock file
	})
	.unwrap_or(3u32.into());

	Ok(dot.into())
}
