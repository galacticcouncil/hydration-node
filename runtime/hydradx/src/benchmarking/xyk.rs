use crate::{AccountId, AssetId, Balance, Currencies, MultiTransactionPayment, Price, Runtime, RuntimeOrigin, XYK};

use super::*;

use frame_benchmarking::{account, BenchmarkError};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_std::prelude::*;

use hydradx_traits::router::{PoolType, TradeExecution};

const SEED: u32 = 1;

const INITIAL_BALANCE: Balance = 1_000_000_000_000_000;

fn funded_account<T: pallet_xyk::Config>(name: &'static str, index: u32, assets: &[AssetId]) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	//Necessary for ED for insufficient assets.
	T::Currency::update_balance(0, &caller, INITIAL_BALANCE as i128).unwrap();

	for a in assets {
		T::Currency::update_balance(*a, &caller, INITIAL_BALANCE as i128).unwrap();
	}

	caller
}

#[allow(clippy::result_large_err)]
fn init_fee_asset(fee_asset: AssetId) -> Result<(), BenchmarkError> {
	MultiTransactionPayment::add_currency(RawOrigin::Root.into(), fee_asset, Price::from(1))
		.map_err(|_| BenchmarkError::Stop("Failed to add fee asset as supported currency"))?;

	pallet_transaction_multi_payment::pallet::AcceptedCurrencyPrice::<Runtime>::insert(fee_asset, Price::from(1));

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, pallet_xyk }

	create_pool {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let caller = funded_account::<Runtime>("caller", 0, &[asset_a, asset_b, fee_asset]);

		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let amount_a : Balance = INITIAL_BALANCE;
		let amount_b : Balance = INITIAL_BALANCE;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 2);
	}: _(RawOrigin::Signed(caller.clone()), asset_a, amount_a, asset_b, amount_b)
	verify {
		assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
		assert_eq!(Currencies::free_balance(asset_b, &caller), 0);

		assert!(!orml_tokens::Accounts::<Runtime>::contains_key(caller.clone(), asset_a));
		assert!(!orml_tokens::Accounts::<Runtime>::contains_key(caller.clone(), asset_b));

		//NOTE: xyk shares are insufficinet so that's why not 0.
		assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
	}


	add_liquidity {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let caller = funded_account::<Runtime>("caller", 0, &[asset_a, asset_b, fee_asset]);
		let maker = funded_account::<Runtime>("maker", 1, &[asset_a, asset_b, fee_asset]);

		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let amount_a : Balance = INITIAL_BALANCE;
		let amount_b : Balance = INITIAL_BALANCE;

		let amount : Balance = INITIAL_BALANCE/2;
		let max_limit : Balance = INITIAL_BALANCE;


		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE - 10, asset_b, INITIAL_BALANCE - 10)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, INITIAL_BALANCE - amount)?;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 2);
	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, max_limit, 0)
	verify {
		assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
		assert_eq!(Currencies::free_balance(asset_b, &caller), 499_999_999_999_999_u128);// Due to rounding in favor of pool

		//NOTE: xyk shares are insufficinet.
		assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 2);
	}

	remove_liquidity {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		//Only for XYK shares
		assert_eq!(frame_system::Pallet::<Runtime>::account(maker.clone()).sufficients, 1);
	}: _(RawOrigin::Signed(maker.clone()), asset_a, asset_b, INITIAL_BALANCE, 0, 0)
	verify {
		assert_eq!(Currencies::free_balance(asset_a, &maker), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(asset_b, &maker), INITIAL_BALANCE);

		assert_eq!(frame_system::Pallet::<Runtime>::account(maker).sufficients, 2);
	}

	sell {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);
		let caller = funded_account::<Runtime>("caller", 1, &[asset_a, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let discount = false;
		let amount: Balance = 250_000_000_000_000;
		let min_bought: Balance = 1;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, INITIAL_BALANCE - amount)?;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 1);
	}: _(RawOrigin::Signed(caller.clone()), asset_a, asset_b, amount, min_bought, discount)
	verify{
		assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
		assert_eq!(Currencies::free_balance(asset_b, &caller), 199400000000000);

		//NOTE: `asset_a`'s ED was released `asset_b`'s ED was collected.
		assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
	}

	buy {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);
		let caller = funded_account::<Runtime>("caller", 1, &[asset_a, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let discount = false;
		let amount: Balance = 200_000_000_000_000;
		let max_sold: Balance = INITIAL_BALANCE;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, 749_249_999_999_999_u128)?;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 1);
	}: _(RawOrigin::Signed(caller.clone()), asset_b, asset_a, amount, max_sold, discount)
	verify{
		assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
		assert_eq!(Currencies::free_balance(asset_b, &caller), amount);

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
	}

	router_execution_sell {
		let c in 1..2;	// if c == 1, calculate_sell is executed
		let e in 0..1;	// if e == 1, execute_sell is executed

		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);
		let caller = funded_account::<Runtime>("caller", 1, &[asset_a, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let discount = false;
		let amount: Balance = 250_000_000_000_000;
		let min_bought: Balance = 1;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, INITIAL_BALANCE - amount)?;
		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 1);
	}: {
		for _ in 1..c {
			assert!(<XYK as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(PoolType::XYK, asset_a, asset_b, amount).is_ok());
		}
		if e != 0 {
			assert!(<XYK as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_sell(RawOrigin::Signed(caller.clone()).into(), PoolType::XYK, asset_a, asset_b, amount, min_bought).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
			assert_eq!(Currencies::free_balance(asset_b, &caller), 199400000000000);

			assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
		}
	}

	router_execution_buy {
		let c in 1..3;	// number of times calculate_buy is executed
		let e in 0..1;	// if e == 1, execute_buy is executed

		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);
		let caller = funded_account::<Runtime>("caller", 1, &[asset_a, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let discount = false;
		let amount: Balance = 200_000_000_000_000;
		let max_sold: Balance = INITIAL_BALANCE;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, 749_249_999_999_999_u128)?;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller.clone()).sufficients, 1);
	}: {
		for _ in 1..c {
			assert!(<XYK as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_in_given_out(PoolType::XYK, asset_a, asset_b, amount).is_ok());
		}
		if e != 0 {
			assert!(<XYK as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::execute_buy(RawOrigin::Signed(caller.clone()).into(), PoolType::XYK, asset_a, asset_b, amount, max_sold).is_ok());
		}
	}
	verify{
		if e != 0 {
			assert_eq!(Currencies::free_balance(asset_a, &caller), 0);
			assert_eq!(Currencies::free_balance(asset_b, &caller), amount);

			assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
		}
	}

	calculate_spot_price_with_fee {
		let asset_a = register_external_asset(b"TKNA".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let asset_b = register_external_asset(b"TKNB".to_vec()).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		let fee_asset = register_asset(b"FEE".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let maker = funded_account::<Runtime>("maker", 0, &[asset_a, asset_b, fee_asset]);
		let caller = funded_account::<Runtime>("caller", 1, &[asset_a, fee_asset]);


		init_fee_asset(fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(maker.clone()).into(), fee_asset)?;
		MultiTransactionPayment::set_currency(RawOrigin::Signed(caller.clone()).into(), fee_asset)?;

		let discount = false;
		let amount: Balance = 200_000_000_000_000;
		let max_sold: Balance = INITIAL_BALANCE;

		XYK::create_pool(RawOrigin::Signed(maker.clone()).into(), asset_a, INITIAL_BALANCE, asset_b, INITIAL_BALANCE)?;

		<Currencies as MultiCurrency<AccountId>>::transfer(asset_a, &caller, &maker, 749_249_999_999_999_u128)?;

		assert_eq!(frame_system::Pallet::<Runtime>::account(caller).sufficients, 1);
	}: {
		assert!(<XYK as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_spot_price_with_fee(PoolType::XYK, asset_a, asset_b).is_ok());
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
