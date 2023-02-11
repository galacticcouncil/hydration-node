#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_traits::Registry;

pub const ONE: Balance = 1_000_000_000_000;

const HDX: u32 = 0;
const DAI: u32 = 1;

benchmarks! {
	where_clause { where
		T::AssetId: From<u32>,
		T::Currency: MultiCurrencyExtended<T::AccountId, Amount=i128>,
		T: crate::pallet::Config,
	}
  place_order {
		prepare::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1)?;
  }:  _(RawOrigin::Signed(owner.clone()), DAI.into(), HDX.into(), 20 * ONE, 100 * ONE, true)
	verify {
		let reserve_id = named_reserve_identifier(0);
		assert_eq!(T::Currency::reserved_balance_named(&reserve_id, HDX.into(), &owner), 100 * ONE);
	}

	fill_order {
		prepare::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1)?;
		let filler: T::AccountId = create_account_with_balances::<T>("filler", 2)?;

		assert_ok!(
			crate::Pallet::<T>::place_order(RawOrigin::Signed(owner.clone()).into(), DAI.into(), HDX.into(), 20 * ONE, 100 * ONE, true)
		);
  }:  _(RawOrigin::Signed(filler.clone()), 0u32, DAI.into(), 10 * ONE)
	verify {
		let reserve_id = named_reserve_identifier(0);
		assert_eq!(T::Currency::reserved_balance_named(&reserve_id, HDX.into(), &owner), 50 * ONE);
	}

	cancel_order {
		prepare::<T>()?;

		let owner: T::AccountId = create_account_with_balances::<T>("owner", 1)?;
		assert_ok!(
			crate::Pallet::<T>::place_order(RawOrigin::Signed(owner.clone()).into(), DAI.into(), HDX.into(), 20 * ONE, 100 * ONE, true)
		);
  }:  _(RawOrigin::Signed(owner.clone()), 0u32)
	verify {
		let reserve_id = named_reserve_identifier(0);
		assert_eq!(T::Currency::reserved_balance_named(&reserve_id, HDX.into(), &owner), 0);
	}
}

fn prepare<T: Config>() -> DispatchResult {
	// Register new asset in asset registry
	T::AssetRegistry::create_asset(&b"HDX".to_vec(), ONE)?;
	T::AssetRegistry::create_asset(&b"DAI".to_vec(), ONE)?;

	Ok(())
}

fn create_account_with_balances<T: Config>(name: &'static str, index: u32) -> Result<T::AccountId, DispatchError>
where
	T::AssetId: From<u32>,
	T::Currency: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
{
	let account_id: T::AccountId = account(name, index, index);

	let token_amount: Balance = 200 * ONE;
	T::Currency::update_balance(0.into(), &account_id, token_amount as i128)?;
	T::Currency::update_balance(1.into(), &account_id, token_amount as i128)?;

	Ok(account_id)
}

pub fn named_reserve_identifier(order_id: OrderId) -> [u8; 8] {
	let prefix = b"otc";
	let mut result = [0; 8];
	result[0..3].copy_from_slice(prefix);
	result[3..7].copy_from_slice(&order_id.to_be_bytes());

	let hashed = BlakeTwo256::hash(&result);
	let mut hashed_array = [0; 8];
	hashed_array.copy_from_slice(&hashed.as_ref()[..8]);
	hashed_array
}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(Pallet, super::ExtBuilder::default().build(), super::Test);
}
