use crate::pallet;
use frame_support::{
	traits::{OnRuntimeUpgrade, StorageVersion},
	weights::Weight,
};
use sp_core::Get;
use sp_runtime::traits::BlockNumberProvider;

// This migration sets SixSecBlocksSince which is used to correctly calculate the periods in staking
// after the migration to 6s block time
pub struct SetSixSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetSixSecBlocksSince<T> {
	fn on_runtime_upgrade() -> Weight {
		let current_block_height = T::BlockNumberProvider::current_block_number();

		crate::SixSecBlocksSince::<T>::mutate(|block_height| {
			if *block_height == u32::MAX.into() {
				*block_height = current_block_height
			}

			log::info!("SixSecBlocksSince set to: {current_block_height:?}");
		});

		T::DbWeight::get().reads_writes(1, 1)
	}
}

// This migration sets TwoSecBlocksSince which is used to correctly calculate the periods in staking
// after the migration to 2s block time.
pub struct SetTwoSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetTwoSecBlocksSince<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		assert!(
			StorageVersion::get::<crate::Pallet<T>>() < StorageVersion::new(3),
			"Staking storage version must be below v3 before setting TwoSecBlocksSince"
		);

		Ok(sp_std::vec::Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let on_chain_version = StorageVersion::get::<crate::Pallet<T>>();
		if on_chain_version >= StorageVersion::new(3) {
			return T::DbWeight::get().reads(1);
		}

		let current_block_height = T::BlockNumberProvider::current_block_number();
		let mut writes = 0u64;

		let two_sec_blocks_since = crate::TwoSecBlocksSince::<T>::get();
		if two_sec_blocks_since == u32::MAX.into() {
			crate::TwoSecBlocksSince::<T>::put(current_block_height);
			writes += 1;

			log::info!("TwoSecBlocksSince set to: {current_block_height:?}");
		} else {
			log::info!("TwoSecBlocksSince already set to: {two_sec_blocks_since:?}");
		}

		StorageVersion::new(3).put::<crate::Pallet<T>>();

		T::DbWeight::get().reads_writes(2, writes + 1)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(3),
			"Staking storage version must be v3 after setting TwoSecBlocksSince"
		);
		assert!(
			crate::TwoSecBlocksSince::<T>::get() != u32::MAX.into(),
			"TwoSecBlocksSince must be initialized"
		);

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::migrations::{SetSixSecBlocksSince, SetTwoSecBlocksSince};
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

	#[test]
	fn set_two_blocks_since_executes_when_storage_not_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			StorageVersion::new(2).put::<Staking>();

			// Act
			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::two_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
			assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(3));
		});
	}

	#[test]
	fn set_two_blocks_since_does_not_execute_when_storage_is_set() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			StorageVersion::new(2).put::<Staking>();
			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			// Act
			set_block_number(1000);
			StorageVersion::new(2).put::<Staking>();
			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::two_sec_blocks_since(), 500u32 as BlockNumberFor<Test>);
			assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(3));
		});
	}

	#[test]
	fn set_two_blocks_since_does_not_execute_when_storage_version_is_current() {
		ExtBuilder::default().build().execute_with(|| {
			// Arrange
			set_block_number(500);
			StorageVersion::new(3).put::<Staking>();

			// Act
			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			// Assert
			assert_eq!(Staking::two_sec_blocks_since(), u32::MAX as BlockNumberFor<Test>);
		});
	}
}
