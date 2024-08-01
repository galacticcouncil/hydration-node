use crate::{AccountId, AssetId, Balance, Currencies, MultiTransactionPayment, Runtime, Tokens, DOT_ASSET_LOCATION};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_system::RawOrigin;

use frame_support::assert_ok;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::FixedU128;

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
		let fee_asset = setup_insufficient_asset_with_dot()?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 1, SEED);
		let to_lookup = lookup_of_account(to.clone());
		set_period(10);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from.clone()).sufficients, 2);

	}: {
		Tokens::transfer(RawOrigin::Signed(from.clone()).into(), to_lookup, asset_id, amount).unwrap();
	}
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), amount);
		assert_eq!(frame_system::Pallet::<Runtime>::account(to).sufficients, 1);

		//NOTE: make sure from was killed
		assert!(!orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id));
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 1);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5); //Counter remains the same as first increased by on_funds, but then decreased on kill
	}

	transfer_all {
		let amount: Balance = UNIT;

		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = setup_insufficient_asset_with_dot()?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to);
		set_period(10);

		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from.clone()).sufficients, 2);

	}: _(RawOrigin::Signed(from.clone()), to_lookup, asset_id, false)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &from), 0);

		//NOTE: make sure from was killed
		assert!(!orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id));
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 1);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5); //Counter remains the same as first increased by on_funds, but then decreased on kill
	}


	transfer_keep_alive {
		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = setup_insufficient_asset_with_dot()?;

		let from: AccountId = account("from", 0, SEED);
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, 2 * UNIT);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
		set_period(10);

		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from.clone()).sufficients, 2);
	}: _(RawOrigin::Signed(from), to_lookup, asset_id, UNIT)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), UNIT);

		//NOTE: make sure none was killed
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 6); //Counter is increased in on_funds but not decreased on kill
	}

	force_transfer {
		let amount = 2 * UNIT;

		let asset_id = register_external_asset(b"TST".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = setup_insufficient_asset_with_dot()?;

		let from: AccountId = account("from", 0, SEED);
		let from_lookup = lookup_of_account(from.clone());
		<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(HDX, &from, (10_000 * UNIT) as i128)?;
		update_balance(asset_id, &from, amount);
		update_balance(fee_asset, &from, 1_000 * UNIT);

		MultiTransactionPayment::set_currency(
			RawOrigin::Signed(from.clone()).into(),
			fee_asset
		)?;

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
		set_period(10);

		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5);
		assert_eq!(frame_system::Pallet::<Runtime>::account(from.clone()).sufficients, 2);

	}: _(RawOrigin::Root, from_lookup, to_lookup, asset_id, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(asset_id, &to), amount);

		//NOTE: make sure from was killed
		assert!(!orml_tokens::Accounts::<Runtime>::contains_key(from.clone(), asset_id));
		assert_eq!(frame_system::Pallet::<Runtime>::account(from).sufficients, 1);
		assert_eq!(pallet_asset_registry::ExistentialDepositCounter::<Runtime>::get(), 5); //Counter remains the same as first increased by on_funds, but then decreased on kill
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
	use crate::NativeExistentialDeposit;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}

//TODO: make it  global func
fn setup_insufficient_asset_with_dot() -> Result<AssetId, BenchmarkError> {
	let dot = register_asset(b"DOT".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
	set_location(dot, DOT_ASSET_LOCATION).map_err(|_| BenchmarkError::Stop("Failed to set location for weth"))?;
	crate::benchmarking::dca::MultiPaymentPallet::<Runtime>::add_currency(
		RawOrigin::Root.into(),
		dot,
		FixedU128::from(1),
	)
	.map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;
	let insufficient_asset =
		register_external_asset(b"FCA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
	crate::benchmarking::dca::create_xyk_pool(insufficient_asset, dot);

	Ok(insufficient_asset)
}
