use crate::pallet;
use frame_support::traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use sp_runtime::Saturating;

// This migration multiplies the periods of schedules by 2 to account for 2x faster block times
//
// The migration does not use a StorageVersion, make sure it is removed from the Runtime Executive
// after it has been run.
pub struct MultiplySchedulesPeriodBy2<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MultiplySchedulesPeriodBy2<T> {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		let mut reads = 0u64;
		let mut writes = 0u64;

		let on_chain_version = StorageVersion::get::<crate::Pallet<T>>();
		let in_code_version = crate::Pallet::<T>::in_code_storage_version();
		reads.saturating_inc();

		if on_chain_version == in_code_version {
			// Already migrated
			return T::DbWeight::get().reads(reads);
		}

		for (key, mut schedule) in crate::Schedules::<T>::iter() {
			schedule.period = schedule.period.saturating_mul(2u32.into());
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
		StorageVersion::new(2).put::<crate::Pallet<T>>();

		log::info!("MultiplySchedulesPeriodBy2 processed schedules: {:?}", writes);
		T::DbWeight::get().reads_writes(reads, writes)
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
	fn multiply_schedules_period_by_2_works() {
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

				// Act
				MultiplySchedulesPeriodBy2::<Test>::on_runtime_upgrade();
				let updated_schedule = DCA::schedules(0).unwrap();

				// Assert
				assert_eq!(updated_schedule.period, 200);

				// Storage version has been updated
				let on_chain_version = StorageVersion::get::<DCA>();
				let in_code_version = DCA::in_code_storage_version();
				assert_eq!(on_chain_version, StorageVersion::new(2));
			});
	}
}
