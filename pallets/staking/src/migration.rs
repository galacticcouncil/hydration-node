use frame_support::migrations::VersionedMigration;
use frame_support::{traits::Get, weights::Weight};

use crate::*;

pub mod versioned {
	use super::*;

	pub type V1ToV2<T> = VersionedMigration<
		1,
		2,
		v2::VersionUncheckedMigrateV1ToV2<T>,
		crate::pallet::Pallet<T>,
		<T as frame_system::Config>::DbWeight,
	>;
}

// This migration clear existing staking votes.
// It is necessary to clear the votes because staking supports opengov from v2.0.0.
pub mod v2 {
	use super::*;
	use frame_support::traits::OnRuntimeUpgrade;

	const TARGET: &str = "runtime::staking::migration::v2";

	pub struct VersionUncheckedMigrateV1ToV2<T>(PhantomData<T>);

	impl<T: Config> OnRuntimeUpgrade for VersionUncheckedMigrateV1ToV2<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
			let existing_votes = PositionVotes::<T>::iter().count();
			let processed_votes = ProcessedVotes::<T>::iter().count();
			log::info!(
				target: TARGET,
				"pre-upgrade state contains '{}' existing votes and '{} processed votes.",
				existing_votes,
				processed_votes

			);
			Ok(Vec::new())
		}

		fn on_runtime_upgrade() -> Weight {
			log::info!(
				target: TARGET,
				"running storage migration from version 1 to version 2."
			);

			let existing_votes = PositionVotes::<T>::iter().count();
			let processed_votes = ProcessedVotes::<T>::iter().count();

			let mut weight = T::DbWeight::get().reads_writes(1, 1);
			let ev = PositionVotes::<T>::clear(existing_votes as u32, None);
			assert!(ev.maybe_cursor.is_none(), "PositionVotes storage is not empty");

			let pv = ProcessedVotes::<T>::clear(processed_votes as u32, None);
			assert!(pv.maybe_cursor.is_none(), "ProcessedVotes storage is not empty");

			weight.saturating_accrue(T::DbWeight::get().reads(existing_votes.saturating_add(processed_votes) as u64));
			weight
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
			// TODO: check if the votes storage is empty
			assert_eq!(
				PositionVotes::<T>::iter().count(),
				0,
				"PositionVotes storage is not empty"
			);
			assert_eq!(
				ProcessedVotes::<T>::iter().count(),
				0,
				"ProcessedVotes storage is not empty"
			);
			Ok(())
		}
	}
}
