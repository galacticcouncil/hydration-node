use crate::{AccountId, AssetId, Balance, Runtime, Tokens};

use sp_std::prelude::*;

use frame_benchmarking::BenchmarkError;
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;

use frame_support::assert_ok;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;

use super::*;

use sp_runtime::traits::{SaturatedConversion, StaticLookup};

const SEED: u32 = 0;

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
}

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Tokens as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, orml_tokens }

	transfer {
		let amount: Balance = BSX;

		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = whitelisted_caller();
		update_balance(asset_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, asset_id, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), amount);
	}

	transfer_all {
		let amount: Balance = BSX;

		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = whitelisted_caller();
		update_balance(asset_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to);
	}: _(RawOrigin::Signed(from.clone()), to_lookup, asset_id, false)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &from), 0);
	}

	transfer_keep_alive {
		let from: AccountId = whitelisted_caller();
		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		update_balance(asset_id, &from, 2 * BSX);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, asset_id, BSX)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), BSX);
	}

	force_transfer {
		let from: AccountId = account("from", 0, SEED);
		let from_lookup = lookup_of_account(from.clone());
		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		update_balance(asset_id, &from, 2 * BSX);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Root, from_lookup, to_lookup, asset_id, BSX)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), BSX);
	}

	set_balance {
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());

		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

	}: _(RawOrigin::Root, who_lookup, asset_id, BSX, BSX)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &who), 2 * BSX);
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
