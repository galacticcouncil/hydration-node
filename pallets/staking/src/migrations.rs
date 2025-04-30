use crate::pallet;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_core::Get;
use sp_runtime::traits::BlockNumberProvider;

// This migration sets SixSecBlocksSince which is used to correctly calculate the periods in staking
// after the migration to 6s block time
pub struct SetSixSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetSixSecBlocksSince<T> {
	fn on_runtime_upgrade() -> Weight {
		let current_block_height = T::BlockNumberProvider::current_block_number();

		crate::SixSecBlocksSince::<T>::mutate(|block_height| {
			if *block_height == u32::max_value().into() {
				*block_height = current_block_height
			}

			log::info!("SixSecBlocksSince set to: {:?}", current_block_height);
		});

		T::DbWeight::get().reads_writes(1, 1)
	}
}

#[cfg(all(feature = "try-runtime", test))]
mod test {
	use super::*;
	use crate::migrations::SetSixSecBlocksSince;
	use crate::tests::mock::{set_block_number, ExtBuilder, Staking, Test};
	use frame_system::pallet_prelude::BlockNumberFor;

	#[test]
	fn set_six_blocks_since_executes_when_storage_not_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);

			// Act
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::six_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
		});
	}

	#[test]
	fn set_six_blocks_since_does_not_execute_when_storage_is_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Act
			set_block_number(1000);
			SetSixSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::six_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
		});
	}
}
