use frame_support::{traits::{Get, OnRuntimeUpgrade}, weights::RuntimeDbWeight, Parameter};
use sp_runtime::Saturating;
use sp_runtime::traits::{AtLeast32BitUnsigned, Member};
use crate::pallet;

// This migration multiplies the periods of schedules by 2 to account for 2x faster block times
//
// The migration does not use a StorageVersion, make sure it is removed from the Runtime migrations
// after it has been run.
pub struct MultiplySchedulesPeriodBy2<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for MultiplySchedulesPeriodBy2<T> {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        let mut schedules_len = 0;

        for (key, mut schedule) in crate::Schedules::<T>::iter() {
            schedule.period = schedule.period.saturating_mul(2u32.into());
            crate::Schedules::<T>::insert(key, schedule);
            schedules_len.saturating_inc();

            /// At the time of the migration there are ~70 schedules.
            /// Setting a safe limit which can be executed in 1 block
            if schedules_len == 150 {
                log::info!("Hit limit of 150 schedules, exiting loop");
                break;
            }
        }

        log::info!("MultiplySchedulesPeriodBy2 processed schedules: {:?}", schedules_len);
        T::DbWeight::get().reads_writes(schedules_len, schedules_len)
    }
}
