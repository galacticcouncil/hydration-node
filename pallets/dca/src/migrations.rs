use crate::pallet;
use frame_support::traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use sp_runtime::Saturating;

// This migration multiplies the periods of schedules by 3 to account for 3x faster block times
// when moving from 6s to 2s blocks.
pub struct MultiplySchedulesPeriodBy3<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MultiplySchedulesPeriodBy3<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(2),
			"DCA storage version must be v2 before multiplying schedule periods"
		);

		Ok(sp_std::vec::Vec::new())
	}

	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		let mut reads = 0u64;
		let mut writes = 0u64;

		let on_chain_version = StorageVersion::get::<crate::Pallet<T>>();
		let in_code_version = crate::Pallet::<T>::in_code_storage_version();
		reads.saturating_inc();

		if on_chain_version >= in_code_version {
			// Already migrated
			return T::DbWeight::get().reads(reads);
		}

		if on_chain_version != StorageVersion::new(2) {
			log::warn!("DCA schedule period migration skipped: expected storage version 2, got {on_chain_version:?}");
			return T::DbWeight::get().reads(reads);
		}

		for (key, mut schedule) in crate::Schedules::<T>::iter() {
			schedule.period = schedule.period.saturating_mul(3u32.into());
			crate::Schedules::<T>::insert(key, schedule);
			reads.saturating_inc();
			writes.saturating_inc();

			// At the time before the migration there are ~60 schedules.
			// Setting a safe limit which can be executed in 1 block
			if writes == 150 {
				log::info!("Hit limit of 150 schedules, exiting loop");
				break;
			}
		}

		// Increase on-chain StorageVersion
		StorageVersion::new(3).put::<crate::Pallet<T>>();
		writes.saturating_inc();

		log::info!("MultiplySchedulesPeriodBy3 processed schedules: {writes:?}");
		T::DbWeight::get().reads_writes(reads, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(3),
			"DCA storage version must be v3 after multiplying schedule periods"
		);

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::tests::mock::{RuntimeOrigin, Test, ALICE, DCA};
	use crate::tests::schedule::set_block_number;
	use crate::tests::{
		mock::{ExtBuilder, HDX, ONE},
		ScheduleBuilder,
	};
	use frame_support::assert_ok;

	#[test]
	fn multiply_schedules_period_by_3_works() {
		ExtBuilder::default()
			.with_endowed_accounts(vec![(ALICE, HDX, 10000 * ONE)])
			.build()
			.execute_with(|| {
				// Arrange
				let schedule = ScheduleBuilder::new().build();
				set_block_number(500);
				assert_ok!(DCA::schedule(
					RuntimeOrigin::signed(ALICE),
					schedule.clone(),
					Option::None
				));

				let stored_schedule = DCA::schedules(0).unwrap();
				assert_eq!(stored_schedule.period, 100);
				StorageVersion::new(2).put::<DCA>();

				// Act
				MultiplySchedulesPeriodBy3::<Test>::on_runtime_upgrade();
				let updated_schedule = DCA::schedules(0).unwrap();

				// Assert
				assert_eq!(updated_schedule.period, 300);

				// Storage version has been updated
				let on_chain_version = StorageVersion::get::<DCA>();
				assert_eq!(on_chain_version, StorageVersion::new(3));
			});
	}
}
