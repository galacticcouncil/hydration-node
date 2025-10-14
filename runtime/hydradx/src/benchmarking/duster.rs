use crate::{AccountId, AssetId, Balance, Duster, NativeAssetId, Runtime, Tokens};

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::whitelisted_caller;
use frame_benchmarking::BenchmarkError;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{GetByKey, MultiCurrency, MultiCurrencyExtended};
use sp_runtime::traits::SaturatedConversion;

const SEED: u32 = 1;

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Tokens as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, pallet_duster }

	dust_account{
		let caller: AccountId = account("caller", 0, SEED);
		let burner_acc: AccountId = account("burner", 2, SEED);

		let amount: Balance = 1_000 * BSX;
		let to_dust_account: AccountId = whitelisted_caller();
		let contract_address = deploy_token_contract(to_dust_account.clone());
		let asset_id = bind_erc20(contract_address); //Dusting AToken is the worst case scenario
		assert_eq!(crate::Currencies::free_balance(asset_id, &to_dust_account), 1000000000000000000000000000);
		let ed = 10000;
		set_ed(asset_id, ed);
		crate::Currencies::transfer(
			RawOrigin::Signed(to_dust_account.clone()).into(),
			burner_acc,
			asset_id,
			1000000000000000000000000000 - ed + 1u128,
		).map_err(|_| BenchmarkError::Stop("Failed to transfer"))?;
		let dust_amount = 9999;
		assert_eq!(crate::Currencies::free_balance(asset_id, &to_dust_account), dust_amount);

		let dest_account = <Runtime as pallet_duster::Config>::TreasuryAccountId::get();

		let current_balance = crate::Currencies::free_balance(asset_id, &dest_account.clone());

	}: { pallet_duster::Pallet::<Runtime>::dust_account(RawOrigin::Signed(caller.clone()).into(), to_dust_account.clone(),asset_id)? }
	verify {
		assert_eq!(crate::Currencies::free_balance(asset_id, &to_dust_account), 0u128);
		assert_eq!(crate::Currencies::free_balance(asset_id, &dest_account), current_balance + dust_amount);
	}

	whitelist_account{
		let caller: AccountId = account("caller", 0, SEED);
		let nondustable_account: AccountId = account("dust", 0, SEED);
	}: { pallet_duster::Pallet::<Runtime>::whitelist_account(RawOrigin::Root.into(), nondustable_account.clone())? }
	verify {
		assert!(pallet_duster::Pallet::<Runtime>::whitelisted(&nondustable_account).is_some());
	}

	remove_from_whitelist{
		let caller: AccountId = account("caller", 0, SEED);
		let nondustable_account: AccountId = account("dust", 0, SEED);
		pallet_duster::Pallet::<Runtime>::whitelist_account(RawOrigin::Root.into(), nondustable_account.clone())?;

	}: { pallet_duster::Pallet::<Runtime>::remove_from_whitelist(RawOrigin::Root.into(), nondustable_account.clone())? }
	verify {
		assert!(pallet_duster::Pallet::<Runtime>::whitelisted(&nondustable_account).is_none());
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
