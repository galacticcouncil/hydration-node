use frame_support::{traits::Get, weights::Weight};

use crate::*;

const TARGET: &str = "runtime::staking::migration::v2";

pub fn migrate_to_v2<T: Config>() -> Weight {
	let on_chain_storage_version = StorageVersion::get::<Pallet<T>>();
	let mut weight: Weight = T::DbWeight::get().reads(1);

	if on_chain_storage_version < 2 {
		log::info!(
			target: TARGET,
			"Running migration storage v2 for pallet-staking with storage version {:?}",
			on_chain_storage_version,
		);

		let existing_votes = PositionVotes::<T>::iter().count();
		let processed_votes = ProcessedVotes::<T>::iter().count();

		weight.saturating_accrue(T::DbWeight::get().reads(2));
		log::info!(
			target: TARGET,
			"Clearing '{}' existing votes and '{}' processed votes.",
			existing_votes,
			processed_votes

		);
		if existing_votes.saturating_add(processed_votes) > 500 {
			log::warn!(
				target: TARGET,
				"Migration to v2 will take a long time due to the large number of votes. Ignoring."
			);
		} else {
			let _ = PositionVotes::<T>::clear(existing_votes as u32, None);
			let _ = ProcessedVotes::<T>::clear(processed_votes as u32, None);
			weight.saturating_accrue(T::DbWeight::get().reads(existing_votes.saturating_add(processed_votes) as u64));
		}
		StorageVersion::new(2).put::<Pallet<T>>();
		weight.saturating_accrue(T::DbWeight::get().writes(1));
	} else {
		log::warn!(
			target: TARGET,
			"Attempted to apply migration to v2 but failed because storage version is {:?}",
			on_chain_storage_version,
		);
	}

	weight
}
