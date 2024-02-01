use crate::{AccountId, AssetId, Balance, Currencies, MultiTransactionPayment, Runtime, Tokens};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_system::RawOrigin;

use frame_support::assert_ok;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use primitives::Price;

use super::*;

use sp_runtime::traits::{SaturatedConversion, StaticLookup};

const SEED: u32 = 0;
const HDX: AssetId = 0;
const UNIT: Balance = 1_000_000_000_000;

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
		let amount: Balance = 2 * UNIT;

		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::add_currency(RawOrigin::Root.into(), fee_asset, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;
		pallet_transaction_multi_payment::pallet::AcceptedCurrencyPrice::<Runtime>::insert(fee_asset, Price::from(1));

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone().into()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 1, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from.clone()), to_lookup, asset_id, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), amount);
		assert_eq!(frame_system::Pallet::<Runtime>::account(to).sufficients, 1);

		//NOTE: make sure from was killed
		assert_eq!(orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id), false);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 1);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 0);
	}

	transfer_all {
		let amount: Balance = 1 * UNIT;

		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::add_currency(RawOrigin::Root.into(), fee_asset, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;
		pallet_transaction_multi_payment::pallet::AcceptedCurrencyPrice::<Runtime>::insert(fee_asset, Price::from(1));

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone().into()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to);
	}: _(RawOrigin::Signed(from.clone()), to_lookup, asset_id, false)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &from), 0);

		//NOTE: make sure from was killed
		assert_eq!(orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id), false);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 1);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 0);
	}

	transfer_keep_alive {
		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, 2 * UNIT);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::add_currency(RawOrigin::Root.into(), fee_asset, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;
		pallet_transaction_multi_payment::pallet::AcceptedCurrencyPrice::<Runtime>::insert(fee_asset, Price::from(1));

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone().into()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, asset_id, UNIT)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), UNIT);

		//NOTE: make sure none was killed
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 2);
	}

	force_transfer {
		let amount = 2 * UNIT;

		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let from: AccountId = account("from", 0, SEED);
		let from_lookup = lookup_of_account(from.clone());
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::add_currency(RawOrigin::Root.into(), fee_asset, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;
		pallet_transaction_multi_payment::pallet::AcceptedCurrencyPrice::<Runtime>::insert(fee_asset, Price::from(1));

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone().into()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Root, from_lookup, to_lookup, asset_id, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), amount);

		//NOTE: make sure from was killed
		assert_eq!(orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id), false);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 1);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 0);
	}

	//NOTE: set balance bypass MutationHooks so sufficiency check is never triggered.
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
