// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_parameters::{pallet, TwoSecBlocksSince};
use sp_core::Get;
use sp_std::marker::PhantomData;

const MIGRATION_DONE_KEY: &[u8] = b"HydrationTwoSecBlocksSinceMigrationDone";

/// Records the 2-second block transition once for all runtime consumers.
pub struct SetTwoSecBlocksSince<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> OnRuntimeUpgrade for SetTwoSecBlocksSince<T> {
	fn on_runtime_upgrade() -> Weight {
		if sp_io::storage::get(MIGRATION_DONE_KEY).is_some() {
			log::warn!("SetTwoSecBlocksSince already executed");
			return T::DbWeight::get().reads(1);
		}

		let current_block = frame_system::Pallet::<T>::block_number();
		let transition_block = TwoSecBlocksSince::<T>::get();
		let mut writes = 1u64;
		if transition_block == u32::MAX.into() {
			TwoSecBlocksSince::<T>::put(current_block);
			writes = writes.saturating_add(1);
			log::info!("SetTwoSecBlocksSince set transition block to: {current_block:?}");
		} else {
			log::warn!("SetTwoSecBlocksSince preserved existing transition block: {transition_block:?}");
		}
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());

		T::DbWeight::get().reads_writes(2, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{Runtime, System};

	#[test]
	fn migration_should_set_shared_transition_block_only_once() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(500);
			SetTwoSecBlocksSince::<Runtime>::on_runtime_upgrade();
			assert_eq!(TwoSecBlocksSince::<Runtime>::get(), 500);

			System::set_block_number(1_000);
			SetTwoSecBlocksSince::<Runtime>::on_runtime_upgrade();
			assert_eq!(TwoSecBlocksSince::<Runtime>::get(), 500);
		});
	}

	#[test]
	fn migration_should_preserve_existing_shared_transition_block() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			System::set_block_number(500);
			TwoSecBlocksSince::<Runtime>::put(123);

			SetTwoSecBlocksSince::<Runtime>::on_runtime_upgrade();

			assert_eq!(TwoSecBlocksSince::<Runtime>::get(), 123);
		});
	}
}
