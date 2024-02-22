use crate::{AccountId, Amount, AssetId, Balance, Currencies, NativeAssetId, Runtime};
use primitives::constants::currency::NATIVE_EXISTENTIAL_DEPOSIT;

use sp_std::prelude::*;

use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::{StaticLookup, UniqueSaturatedInto};
use sp_runtime::SaturatedConversion;

use frame_benchmarking::BenchmarkError;
use frame_support::assert_ok;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;

use super::*;

const SEED: u32 = 0;

const NATIVE: AssetId = NativeAssetId::get();

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
}

pub fn set_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, pallet_currencies }

	// `transfer` non-native currency
	transfer_non_native_currency {
		let amount: Balance = 1_000 * BSX;
		let from: AccountId = whitelisted_caller();
		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		set_balance(asset_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, asset_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_id, &to), amount);
	}

	// `transfer` native currency and in worst case
	#[extra]
	transfer_native_currency_worst_case {
		let existential_deposit = NATIVE_EXISTENTIAL_DEPOSIT;
		let amount: Balance = existential_deposit.saturating_mul(1000);
		let from: AccountId = whitelisted_caller();
		set_balance(NATIVE, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, NATIVE, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(NATIVE, &to), amount);
	}

	// `transfer_native_currency` in worst case
	// * will create the `to` account.
	// * will kill the `from` account.
	transfer_native_currency {
		let existential_deposit = NATIVE_EXISTENTIAL_DEPOSIT;
		let amount: Balance = existential_deposit.saturating_mul(1000);
		let from: AccountId = whitelisted_caller();
		set_balance(NATIVE, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(NATIVE, &to), amount);
	}

	// `update_balance` for non-native currency
	update_balance_non_native_currency {
		let balance: Balance = 2 * BSX;
		let amount: Amount = balance.unique_saturated_into();
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
	}: update_balance(RawOrigin::Root, who_lookup, asset_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(asset_id, &who), balance);
	}

	// `update_balance` for native currency
	// * will create the `who` account.
	update_balance_native_currency_creating {
		let existential_deposit = NATIVE_EXISTENTIAL_DEPOSIT;
		let balance: Balance = existential_deposit.saturating_mul(1000);
		let amount: Amount = balance.unique_saturated_into();
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
	}: update_balance(RawOrigin::Root, who_lookup, NATIVE, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(NATIVE, &who), balance);
	}

	// `update_balance` for native currency
	// * will kill the `who` account.
	update_balance_native_currency_killing {
		let existential_deposit = NATIVE_EXISTENTIAL_DEPOSIT;
		let balance: Balance = existential_deposit.saturating_mul(1000);
		let amount: Amount = balance.unique_saturated_into();
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
		set_balance(NATIVE, &who, balance);
	}: update_balance(RawOrigin::Root, who_lookup, NATIVE, -amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(NATIVE, &who), 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
