use crate::*;
use frame_support::weights::Weight;
use frame_support::{migrations::VersionedMigration, traits::UncheckedOnRuntimeUpgrade};
use hydradx_traits::AMM;
use pallet_xyk::types::AssetPair;
use primitives::constants::chain::XYK_SOURCE;

//This migration migrates ema-oracle's storages to support `shares_issuance` tracking

mod v0 {
	use super::*;
	use frame_support::storage_alias;

	#[derive(RuntimeDebug, Encode, Decode, Clone, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
	pub struct OracleEntry<BlockNumber> {
		pub price: Price,
		pub volume: Volume<Balance>,
		pub liquidity: Liquidity<Balance>,
		pub updated_at: BlockNumber,
	}

	#[storage_alias]
	pub type Oracles<T: crate::Config> = StorageNMap<
		Pallet<T>,
		(
			NMapKey<Twox64Concat, Source>,
			NMapKey<Twox64Concat, (AssetId, AssetId)>,
			NMapKey<Twox64Concat, OraclePeriod>,
		),
		(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>),
		OptionQuery,
	>;

	//NOTE: `Accumulator` is processed `on_finalize` so this storage doesn't need to be migrated
}

// Private module to hide the migration.
mod unversioned {
	pub struct InnerMigrateV0ToV1<T: crate::Config>(core::marker::PhantomData<T>);
}

impl<T: crate::Config + pallet_xyk::Config> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV0ToV1<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!(target: "runtime::ema-oracle", "v0->v1 migration started");

		let mut reads = 0;

		let mut migrated_entries: Vec<(
			(Source, (AssetId, AssetId), OraclePeriod),
			(OracleEntry<BlockNumberFor<T>>, BlockNumberFor<T>),
		)> = Vec::new();

		for (key, val_v0) in v0::Oracles::<T>::iter() {
			reads += 1;

			const DOT: AssetId = 5;
			const EWT: AssetId = 252_525;
			let shares_issuance = if key.0 == XYK_SOURCE && key.1 == (DOT, EWT) {
				//NOTE: we have running liquidity mining fo DOT/EWT pool so we want to set value for
				//this pool in migration. Rest will be populated by normal trading activity.
				reads += 1;

				let pair_account = pallet_xyk::Pallet::<T>::get_pair_id(AssetPair::new(DOT, EWT));
				Some(pallet_xyk::Pallet::<T>::total_liquidity(pair_account))
			} else {
				None
			};

			migrated_entries.push((
				key,
				(
					OracleEntry::new(
						val_v0.0.price,
						val_v0.0.volume,
						val_v0.0.liquidity,
						shares_issuance,
						val_v0.0.updated_at,
					),
					val_v0.1,
				),
			));

			log::info!(target: "runtime::ema-oracle", "entry with key: ({:?}, {:?}, {:?}) migrated", key.0, key.1, key.2);
		}

		for (k, v) in migrated_entries {
			Oracles::<T>::insert(k, v);
		}

		log::info!(target: "runtime::ema-oracle", "ema-oracle oracle entries migration finished, migrated: {:?} entries", reads);
		//NOTE: each read item was also wrote into storage so reads == writes
		T::DbWeight::get().reads_writes(reads, reads)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		for (src, pair, period) in Oracles::<T>::iter_keys() {
			let _ = Oracles::<T>::get((src, pair, period)).expect("Oracle entry must be valid");
		}
		Ok(())
	}
}

pub type MigrateV0ToV1<T> =
	VersionedMigration<0, 1, unversioned::InnerMigrateV0ToV1<T>, Pallet<T>, <T as frame_system::Config>::DbWeight>;
