use crate::pallet;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_core::Get;

pub struct SetTwoSecBlocksSince<T: pallet::Config>(sp_std::marker::PhantomData<T>);
impl<T: pallet::Config> OnRuntimeUpgrade for SetTwoSecBlocksSince<T> {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<sp_std::vec::Vec<u8>, sp_runtime::TryRuntimeError> {
		Ok(sp_std::vec::Vec::new())
	}

	fn on_runtime_upgrade() -> Weight {
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

		T::DbWeight::get().reads_writes(1, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: sp_std::vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		assert!(
			crate::TwoSecBlocksSince::<T>::get() != u32::MAX.into(),
			"GigaHdx TwoSecBlocksSince must be initialized"
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

			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			assert_eq!(GigaHdx::two_sec_blocks_since(), 500);
		});
	}

	#[test]
	fn set_two_sec_blocks_since_does_not_overwrite_existing_value() {
		ExtBuilder::default().build().execute_with(|| {
			System::set_block_number(500);
			TwoSecBlocksSince::<Test>::put(123);

			SetTwoSecBlocksSince::<Test>::on_runtime_upgrade();

			assert_eq!(GigaHdx::two_sec_blocks_since(), 123);
		});
	}
}
