#![cfg_attr(not(feature = "std"), no_std)]

#![allow(clippy::unnecessary_wraps)]

mod amounts;
mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use pallet_exchange::Module as Exchange;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::OnFinalize;
use frame_system::RawOrigin;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::{AssetId, Balance, Price};
use sp_runtime::DispatchError;

use sp_runtime::traits::{BlakeTwo256, Hash};

use amounts::INTENTION_AMOUNTS;

use frame_support::dispatch;
use pallet_amm as ammpool;

pub struct Module<T: Config>(pallet_exchange::Module<T>);

pub trait Config: pallet_exchange::Config + ammpool::Config {}

const INITIAL_ASSET_BALANCE: Balance = 1_000_000_000_000_000;

const MAX_INTENTIONS_IN_BLOCK: u32 = 1000;

const SEED: u32 = 0;
pub const MILLICENTS: Balance = 1_000_000_000;
pub const CENTS: Balance = 1_000 * MILLICENTS;
pub const DOLLARS: Balance = 100 * CENTS;

fn funded_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);

	<T as ammpool::Config>::Currency::update_balance(1, &caller, 1_000_000_000_000_000).unwrap();

	<T as ammpool::Config>::Currency::update_balance(2, &caller, 1_000_000_000_000_000).unwrap();

	caller
}

fn initialize_pool<T: Config>(
	caller: T::AccountId,
	asset_a: AssetId,
	asset_b: AssetId,
	amount: Balance,
	price: Price,
) -> dispatch::DispatchResultWithPostInfo {
	ammpool::Module::<T>::create_pool(RawOrigin::Signed(caller).into(), asset_a, asset_b, amount, price)?;

	Ok(().into())
}

const SELL_INTENTION_AMOUNT: Balance = 1_000_000_000;
const SELL_INTENTION_LIMIT: Balance = 1;
const BUY_INTENTION_AMOUNT: Balance = 1_000_000_000;
const BUY_INTENTION_LIMIT: Balance = 2_000_000_000;

fn feed_intentions<T: Config>(
	asset_a: AssetId,
	asset_b: AssetId,
	number: u32,
	amounts: &[u32],
) -> dispatch::DispatchResultWithPostInfo {
	for idx in 0..number / 2 {
		let user = funded_account::<T>("user", idx + 2);
		pallet_exchange::Module::<T>::sell(
			RawOrigin::Signed(user.clone()).into(),
			asset_a,
			asset_b,
			amounts[idx as usize] as u128,
			SELL_INTENTION_LIMIT,
			false,
		)?;

		let buyer = funded_account::<T>("user", idx + number + 1);
		pallet_exchange::Module::<T>::buy(
			RawOrigin::Signed(buyer.clone()).into(),
			asset_a,
			asset_b,
			amounts[idx as usize] as u128,
			amounts[idx as usize] as u128 * 2u128,
			false,
		)?;
	}

	Ok(().into())
}

fn validate_finalize<T: Config>(
	asset_a: AssetId,
	_asset_b: AssetId,
	number: u32,
	amounts: &[u32],
) -> Result<(), DispatchError> {
	for idx in 0..number / 2 {
		let user: T::AccountId = account("user", idx + 2, SEED);
		assert_eq!(
			<T as ammpool::Config>::Currency::free_balance(asset_a, &user),
			INITIAL_ASSET_BALANCE - amounts[idx as usize] as u128
		);

		let buyer: T::AccountId = account("user", idx + number + 1, SEED);
		assert_eq!(
			<T as ammpool::Config>::Currency::free_balance(asset_a, &buyer),
			INITIAL_ASSET_BALANCE + amounts[idx as usize] as u128
		);
	}

	Ok(())
}

benchmarks! {
	known_overhead_for_on_finalize {
		let t: u32 = 5;
	}: {  Exchange::<T>::on_finalize(t.into()); }
	verify {
	}

	sell_intention {
		let caller = funded_account::<T>("caller", 1);

		let nbr_intentions_appended: u32  = MAX_INTENTIONS_IN_BLOCK;

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance =  DOLLARS;
		let limit : Balance =  DOLLARS;

		initialize_pool::<T>(caller.clone(), asset_a, asset_b, amount, Price::from(10))?;

		feed_intentions::<T>(asset_a, asset_b, nbr_intentions_appended, &INTENTION_AMOUNTS)?;

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), nbr_intentions_appended);

	}: {  Exchange::<T>::sell(RawOrigin::Signed(caller.clone()).into(), asset_a, asset_b, amount ,limit, false)? }
	verify{
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), nbr_intentions_appended + 1);
	}

	buy_intention {
		let caller = funded_account::<T>("caller", 1);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = DOLLARS;
		let limit : Balance = DOLLARS;

		let nbr_intentions_appended: u32  = MAX_INTENTIONS_IN_BLOCK;

		initialize_pool::<T>(caller.clone(), asset_a, asset_b, amount, Price::from(1))?;

		feed_intentions::<T>(asset_a, asset_b, nbr_intentions_appended, &INTENTION_AMOUNTS)?;

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), nbr_intentions_appended);

	}: {  Exchange::<T>::buy(RawOrigin::Signed(caller.clone()).into(), asset_a, asset_b, amount / 10 ,limit, false)? }
	verify{
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), nbr_intentions_appended + 1);
	}

	on_finalize {
		let t in 0 .. MAX_INTENTIONS_IN_BLOCK; // Intention component
		let caller = funded_account::<T>("caller", 1);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 100_000_000_000_000;

		// First generate random amounts
		// This is basically used to generate intentions with different amounts
		// it is because algorithm does sort the intention by amount, so we need something not sorted./
		let random_seed = BlakeTwo256::hash(b"Sixty-nine");

		initialize_pool::<T>(caller, asset_a, asset_b, amount, Price::from(1))?;

		feed_intentions::<T>(asset_a, asset_b, t, &INTENTION_AMOUNTS)?;

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), t);

	}: {  Exchange::<T>::on_finalize(t.into()); }
	verify {
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 0);
		validate_finalize::<T>(asset_a, asset_b, t, &INTENTION_AMOUNTS)?;
	}

	on_finalize_buys_no_matches {
		let t in 0 .. 100; // Intention component
		let caller = funded_account::<T>("caller", 1);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 100_000_000_000_000;

		initialize_pool::<T>(caller, asset_a, asset_b, amount, Price::from(1))?;

		for idx in 0 .. t {
			let user = funded_account::<T>("user", idx + 100);
			pallet_exchange::Module::<T>::buy(
				RawOrigin::Signed(user.clone()).into(),
				asset_a,
				asset_b,
				BUY_INTENTION_AMOUNT,
				BUY_INTENTION_LIMIT,
				false,
			)?;
		}

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), t);

	}: {  Exchange::<T>::on_finalize(t.into()); }
	verify {
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 0);
		for idx in 0..t  {
			let user: T::AccountId = account("user", idx + 100, SEED);
			assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &user), INITIAL_ASSET_BALANCE + SELL_INTENTION_AMOUNT);
		}
	}

	on_finalize_sells_no_matches {
		let t in 0 .. 100; // Intention component
		let caller = funded_account::<T>("caller", 1);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 100_000_000_000_000;

		initialize_pool::<T>(caller, asset_a, asset_b, amount, Price::from(10))?;

		for idx in 0 .. t {
			let user = funded_account::<T>("user", idx + 100);
			pallet_exchange::Module::<T>::sell(
				RawOrigin::Signed(user.clone()).into(),
				asset_a,
				asset_b,
				SELL_INTENTION_AMOUNT,
				SELL_INTENTION_LIMIT,
				false,
			)?;
		}

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), t);

	}: {  Exchange::<T>::on_finalize(t.into()); }
	verify {
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 0);
		for idx in 0..t  {
			let user: T::AccountId = account("user", idx + 100, SEED);
			assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &user), INITIAL_ASSET_BALANCE - SELL_INTENTION_AMOUNT);
		}
	}

	sell_extrinsic {
		let creator = funded_account::<T>("creator", 100);
		let seller = funded_account::<T>("seller", 101);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10_000_000_000;
		let min_bought : Balance = 1_000;
		let discount = false;

		initialize_pool::<T>(creator, asset_a, asset_b, amount, Price::from(1))?;

	}: { ammpool::Module::<T>::sell(RawOrigin::Signed(seller.clone()).into(), asset_a, asset_b, 1_000_000_000, min_bought, false)?; }
	verify {
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &seller), 999_999_000_000_000);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_b, &seller), 1000000907437716);
	}

	on_finalize_for_one_sell_extrinsic {
		let creator = funded_account::<T>("creator", 100);
		let seller = funded_account::<T>("seller", 101);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10_000_000_000;
		let discount = false;

		initialize_pool::<T>(creator, asset_a, asset_b, amount, Price::from(1))?;

		pallet_exchange::Module::<T>::sell(
			RawOrigin::Signed(seller.clone()).into(),
			asset_a,
			asset_b,
			SELL_INTENTION_AMOUNT,
			SELL_INTENTION_LIMIT,
			false,
		)?;

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 1);

	}: {  Exchange::<T>::on_finalize(1u32.into()); }
	verify {
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 0);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &seller), 999_999_000_000_000);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_b, &seller), 1000000907437716);
	}

	buy_extrinsic {
		let creator = funded_account::<T>("creator", 100);
		let buyer = funded_account::<T>("seller", 101);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10_000_000_000;
		let max_sold: Balance = 2_000_000_000;
		let discount = false;

		initialize_pool::<T>(creator, asset_a, asset_b, amount, Price::from(1))?;

	}: { ammpool::Module::<T>::buy(RawOrigin::Signed(buyer.clone()).into(), asset_a, asset_b, 1_000_000_000, max_sold, false)?; }
	verify {
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &buyer), 1000001000000000);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_b, &buyer), 999998886419204);
	}

	on_finalize_for_one_buy_extrinsic {
		let t:u32 = 5;

		let creator = funded_account::<T>("creator", 100);
		let buyer = funded_account::<T>("seller", 101);

		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;
		let amount : Balance = 10_000_000_000;
		let max_sold: Balance = 2_000_000_000;
		let discount = false;

		initialize_pool::<T>(creator, asset_a, asset_b, amount, Price::from(1))?;

		pallet_exchange::Module::<T>::buy(
			RawOrigin::Signed(buyer.clone()).into(),
			asset_a,
			asset_b,
			1_000_000_000,
			max_sold,
			false,
		)?;

		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 1);

	}: {  Exchange::<T>::on_finalize(t.into()); }
	verify {
		assert_eq!(pallet_exchange::Module::<T>::get_intentions_count((asset_a, asset_b)), 0);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_a, &buyer), 1000001000000000);
		assert_eq!(<T as ammpool::Config>::Currency::free_balance(asset_b, &buyer), 999998886419204);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Test};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_known_overhead_for_on_finalize::<Test>());
			assert_ok!(test_benchmark_sell_intention::<Test>());
			assert_ok!(test_benchmark_buy_intention::<Test>());
			assert_ok!(test_benchmark_on_finalize::<Test>());
			assert_ok!(test_benchmark_on_finalize_buys_no_matches::<Test>());
			assert_ok!(test_benchmark_on_finalize_sells_no_matches::<Test>());
			assert_ok!(test_benchmark_sell_extrinsic::<Test>());
			assert_ok!(test_benchmark_on_finalize_for_one_sell_extrinsic::<Test>());
			assert_ok!(test_benchmark_buy_extrinsic::<Test>());
			assert_ok!(test_benchmark_on_finalize_for_one_buy_extrinsic::<Test>());
		});
	}
}
