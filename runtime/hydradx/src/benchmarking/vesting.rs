use crate::{
	assets::MaxVestingSchedules, AccountId, AssetId, Balance, BlockNumber, Currencies, NativeAssetId, Runtime, System,
	Vesting,
};

use super::BSX;

use sp_std::prelude::*;

use frame_benchmarking::{account, whitelisted_caller};
use frame_support::assert_ok;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_traits::MultiCurrencyExtended;
use orml_vesting::VestingSchedule;

use primitives::constants::currency::NATIVE_EXISTENTIAL_DEPOSIT;
use sp_runtime::traits::{AccountIdConversion, SaturatedConversion, StaticLookup};
pub type Schedule = VestingSchedule<BlockNumber, Balance>;

const SEED: u32 = 0;
const NATIVE: AssetId = NativeAssetId::get();

fn get_vesting_account() -> AccountId {
	crate::VestingPalletId::get().into_account_truncating()
}

fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
}

fn set_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, orml_vesting }

	vested_transfer {
		let schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: NATIVE_EXISTENTIAL_DEPOSIT,
		};

		let from: AccountId = get_vesting_account();
		set_balance(NATIVE, &from, schedule.total_amount().unwrap() + BSX);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Root, to_lookup, schedule.clone())
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::total_balance(NATIVE, &to),
			schedule.total_amount().unwrap()
		);
	}

	claim {
		let i in 1 .. MaxVestingSchedules::get();

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: NATIVE_EXISTENTIAL_DEPOSIT,
		};

		let from: AccountId = get_vesting_account();
		set_balance(NATIVE, &from, schedule.total_amount().unwrap() * i as u128 + BSX);

		let to: AccountId = whitelisted_caller();
		let to_lookup = lookup_of_account(to.clone());

		for _ in 0..i {
			schedule.start = i;
			Vesting::vested_transfer(RawOrigin::Root.into(), to_lookup.clone(), schedule.clone())?;
		}
		System::set_block_number(schedule.end().unwrap() + 1u32);
	}: _(RawOrigin::Signed(to.clone()))
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(NATIVE, &to),
			schedule.total_amount().unwrap() * i as u128,
		);
	}

	update_vesting_schedules {
		let i in 1 .. MaxVestingSchedules::get();

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: NATIVE_EXISTENTIAL_DEPOSIT,
		};

		let to: AccountId = account("to", 0, SEED);
		set_balance(NATIVE, &to, schedule.total_amount().unwrap() * i as u128);
		let to_lookup = lookup_of_account(to.clone());

		let mut schedules = vec![];
		for _ in 0..i {
			schedule.start = i;
			schedules.push(schedule.clone());
		}
	}: _(RawOrigin::Root, to_lookup, schedules)
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(NATIVE, &to),
			schedule.total_amount().unwrap() * i as u128
		);
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
