use crate::pallet;
use frame_support::traits::{Get, OnRuntimeUpgrade};
use sp_runtime::Saturating;

// This migration multiplies the periods of schedules by 2 to account for 2x faster block times
//
// The migration does not use a StorageVersion, make sure it is removed from the Runtime Executive
// after it has been run.
pub struct MultiplySchedulesPeriodBy2<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MultiplySchedulesPeriodBy2<T> {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		let mut schedules_len = 0;

		for (key, mut schedule) in crate::Schedules::<T>::iter() {
			schedule.period = schedule.period.saturating_mul(2u32.into());
			crate::Schedules::<T>::insert(key, schedule);
			schedules_len.saturating_inc();

			// At the time before the migration there are ~60 schedules.
			// Setting a safe limit which can be executed in 1 block
			if schedules_len == 150 {
				log::info!("Hit limit of 150 schedules, exiting loop");
				break;
			}
		}

		log::info!("MultiplySchedulesPeriodBy2 processed schedules: {:?}", schedules_len);
		T::DbWeight::get().reads_writes(schedules_len, schedules_len)
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
				let total_amount = 100 * ONE;
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
			});
	}
}
