use crate::{Balance, Config, Liquidity, OracleEntry, Oracles, Pallet, Price, Volume, LOG_TARGET};

pub mod v1 {
	use super::*;
	use codec::MaxEncodedLen;
	use frame_support::pallet_prelude::*;
	use frame_support::{traits::StorageVersion, weights::Weight};
	use sp_core::RuntimeDebug;

	/// A type representing data produced by a trade or liquidity event. Timestamped to the block where
	/// it was created.
	#[derive(RuntimeDebug, Encode, Decode, Clone, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
	pub struct OldOracleEntry<BlockNumber> {
		pub price: Price,
		pub volume: Volume<Balance>,
		pub liquidity: Liquidity<Balance>,
		pub timestamp: BlockNumber,
	}

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "Storage version too high.");

		log::info!(target: LOG_TARGET, "EMA v1 migration: PRE checks successful!");
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(target: LOG_TARGET, "Running migration to v1 for EMA");

		let mut i = 0;
		Oracles::<T>::translate(
			|_,
			 (
				OldOracleEntry {
					price,
					volume,
					liquidity,
					timestamp,
				},
				init,
			)| {
				i += 1;
				Some((
					OracleEntry {
						price,
						volume,
						liquidity,
						updated_at: timestamp,
						inverted_price: price.inverted(),
					},
					init,
				))
			},
		);

		StorageVersion::new(1).put::<Pallet<T>>();

		T::DbWeight::get().reads_writes(i, i)
	}

	pub fn post_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Unexpected storage version.");

		log::info!(target: LOG_TARGET, "EMA v1 migration: POST checks successful!");
	}
}
