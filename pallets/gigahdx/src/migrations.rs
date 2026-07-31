use crate::pallet;
use frame_support::{
	traits::{OnRuntimeUpgrade, StorageVersion},
	weights::Weight,
};
use sp_core::Get;

pub struct SetTwoSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetTwoSecBlocksSince<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(1),
			"GigaHdx storage version must be v1 before setting TwoSecBlocksSince"
		);

		Ok(sp_std::vec::Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
		let on_chain_version = StorageVersion::get::<crate::Pallet<T>>();
		if on_chain_version >= StorageVersion::new(2) {
			return T::DbWeight::get().reads(1);
		}

		if on_chain_version != StorageVersion::new(1) {
			log::warn!("GigaHdx 2s block migration skipped: expected storage version 1, got {on_chain_version:?}");
			return T::DbWeight::get().reads(1);
		}

		let current_block_height = frame_system::Pallet::<T>::block_number();
		let mut writes = 0u64;

		let two_sec_blocks_since = crate::TwoSecBlocksSince::<T>::get();
		if two_sec_blocks_since == u32::MAX.into() {
			crate::TwoSecBlocksSince::<T>::put(current_block_height);
			writes += 1;

			log::info!("GigaHdx TwoSecBlocksSince set to: {current_block_height:?}");
		} else {
			log::info!("GigaHdx TwoSecBlocksSince already set to: {two_sec_blocks_since:?}");
		}

		StorageVersion::new(2).put::<crate::Pallet<T>>();
		writes += 1;

		T::DbWeight::get().reads_writes(2, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert!(
			crate::TwoSecBlocksSince::<T>::get() != u32::MAX.into(),
			"GigaHdx TwoSecBlocksSince must be initialized"
		);
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			StorageVersion::new(2),
			"GigaHdx storage version must be v2 after setting TwoSecBlocksSince"
		);

		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{
		tests::mock::{ExtBuilder, GigaHdx, System, Test},
		TwoSecBlocksSince,
	};
	use frame_support::traits::OnRuntimeUpgrade;

	#[test]
	fn set_two_sec_blocks_since_executes_when_storage_not_set() {
		ExtBuilder::default().build().execute_with(|| {
			System::set_block_number(500);
			StorageVersion::new(1).put::<GigaHdx>();

			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			assert_eq!(GigaHdx::two_sec_blocks_since(), 500);
			assert_eq!(StorageVersion::get::<GigaHdx>(), StorageVersion::new(2));
		});
	}

	#[test]
	fn set_two_sec_blocks_since_does_not_overwrite_existing_value() {
		ExtBuilder::default().build().execute_with(|| {
			System::set_block_number(500);
			StorageVersion::new(1).put::<GigaHdx>();
			TwoSecBlocksSince::<Test>::put(123);

			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			assert_eq!(GigaHdx::two_sec_blocks_since(), 123);
			assert_eq!(StorageVersion::get::<GigaHdx>(), StorageVersion::new(2));
		});
	}
}
