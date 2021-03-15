#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::OrderedSet;
use pallet_transaction_multi_payment::Module as MultiPaymentModule;
use primitives::{Amount, AssetId, Balance, Price};

use frame_support::dispatch;
use pallet_amm as ammpool;

pub struct Module<T: Config>(pallet_transaction_multi_payment::Module<T>);

pub trait Config:
	pallet_transaction_payment::Config + pallet_transaction_multi_payment::Config + ammpool::Config
{
}

const SEED: u32 = 0;
const ASSET_ID: u32 = 3;
const HDX: u32 = 0;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId
where
	T::MultiCurrency: MultiCurrencyExtended<T::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>,
{
	let caller: T::AccountId = account(name, index, SEED);

	T::MultiCurrency::update_balance(ASSET_ID, &caller, 2000).unwrap();
	T::MultiCurrency::update_balance(HDX, &caller, 2000).unwrap();

	caller
}

fn initialize_pool<T: Config>(
	caller: T::AccountId,
	asset: AssetId,
	amount: Balance,
	price: Price,
) -> dispatch::DispatchResultWithPostInfo {
	ammpool::Module::<T>::create_pool(RawOrigin::Signed(caller).into(), HDX, asset, amount, price)?;
	Ok(().into())
}

benchmarks! {
	swap_currency {
		let maker = funded_account::<T>("maker", 1);
		initialize_pool::<T>(maker.clone(), ASSET_ID, 1000, Price::from(1))?;
		MultiPaymentModule::<T>::add_new_member(&maker);
		MultiPaymentModule::<T>::add_currency(RawOrigin::Signed(maker).into(), ASSET_ID)?;

		let caller = funded_account::<T>("caller", 2);
		MultiPaymentModule::<T>::set_currency(RawOrigin::Signed(caller.clone()).into(), ASSET_ID)?;

	}: { MultiPaymentModule::<T>::swap_currency(&caller, 10)? }
	verify{
		assert_eq!(MultiPaymentModule::<T>::get_currency(caller.clone()), Some(ASSET_ID));
		assert_eq!(T::MultiCurrency::free_balance(ASSET_ID, &caller),2000 - 10 -1 );
	}

	set_currency {
		let maker = funded_account::<T>("maker", 1);
		MultiPaymentModule::<T>::add_new_member(&maker);
		MultiPaymentModule::<T>::add_currency(RawOrigin::Signed(maker).into(), ASSET_ID)?;

		let caller = funded_account::<T>("caller", 123);

		let currency_id: u32 = ASSET_ID;

	}: { MultiPaymentModule::<T>::set_currency(RawOrigin::Signed(caller.clone()).into(), currency_id)? }
	verify{
		assert_eq!(MultiPaymentModule::<T>::get_currency(caller), Some(currency_id));
	}

	add_currency {
		let caller = funded_account::<T>("maker", 1);
		MultiPaymentModule::<T>::add_new_member(&caller);
	}: { MultiPaymentModule::<T>::add_currency(RawOrigin::Signed(caller.clone()).into(), 10)? }
	verify {
		assert_eq!(MultiPaymentModule::<T>::currencies(), OrderedSet::from(vec![10]));
	}

	remove_currency {
		let caller = funded_account::<T>("maker", 1);
		MultiPaymentModule::<T>::add_new_member(&caller);
		MultiPaymentModule::<T>::add_currency(RawOrigin::Signed(caller.clone()).into(), 10)?;

		assert_eq!(MultiPaymentModule::<T>::currencies(), OrderedSet::from(vec![10]));

	}: { MultiPaymentModule::<T>::remove_currency(RawOrigin::Signed(caller.clone()).into(), 10)? }
	verify {
		assert_eq!(MultiPaymentModule::<T>::currencies(), OrderedSet::<AssetId>::new())
	}

	add_member{
		let member = funded_account::<T>("newmember", 10);
	}: { MultiPaymentModule::<T>::add_member(RawOrigin::Root.into(), member.clone())? }
	verify {
		assert_eq!(MultiPaymentModule::<T>::authorities(), vec![member]);
	}

	remove_member{
		let member = funded_account::<T>("newmember", 10);
		MultiPaymentModule::<T>::add_new_member(&member);
	}: { MultiPaymentModule::<T>::remove_member(RawOrigin::Root.into(), member.clone())? }
	verify {
		assert_eq!(MultiPaymentModule::<T>::authorities(), vec![]);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::default().base_weight(5).build().execute_with(|| {
			assert_ok!(test_benchmark_swap_currency::<Test>());
			assert_ok!(test_benchmark_set_currency::<Test>());
			assert_ok!(test_benchmark_add_currency::<Test>());
			assert_ok!(test_benchmark_remove_currency::<Test>());
			assert_ok!(test_benchmark_add_member::<Test>());
			assert_ok!(test_benchmark_remove_member::<Test>());
		});
	}
}
