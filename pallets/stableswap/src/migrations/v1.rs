use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::migrations::VersionedMigration;
use frame_support::traits::UncheckedOnRuntimeUpgrade;
use frame_support::Blake2_128Concat;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_runtime::traits::Saturating;
use sp_runtime::FixedU128;
use sp_runtime::Perbill;
use types::BoundedPegSources;

const LOG_TARGET: &str = "runtime::stableswap";

mod v0 {
	use super::*;
	use frame_support::storage_alias;

	#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct PoolPegInfo<AssetId = ()> {
		pub source: BoundedPegSources<AssetId>,
		pub max_peg_update: Perbill,
		pub current: BoundedPegs,
	}

	#[storage_alias()]
	pub type PoolPegs<T: crate::Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as crate::Config>::AssetId,
		PoolPegInfo<<T as crate::Config>::AssetId>,
	>;
}

// Private module to hide migration
mod unversioned {
	pub struct InnerMigrateV0ToV1<T: crate::Config>(core::marker::PhantomData<T>);
}

impl<T: crate::Config<AssetId = u32>> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV0ToV1<T> {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		log::info!(target: LOG_TARGET, "v0->v1 migration started");

		let mut reads: u64 = 0;
		let mut writes: u64 = 0;

		let mut migrated_pegs_info: Vec<(T::AssetId, types::PoolPegInfo<BlockNumberFor<T>, T::AssetId>)> =
			Vec::with_capacity(4);
		let current_block = T::BlockNumberProvider::current_block_number();

		for (k, peg_info_v0) in v0::PoolPegs::<T>::iter() {
			log::info!(target: LOG_TARGET, "updating pegs for pool_id: {:?}", k);
			//NOTE: 1 read for v0::PoolPegs
			reads += 1;

			reads += 1;
			let pool = if let Some(p) = Pools::<T>::get(k) {
				p
			} else {
				log::error!(target: LOG_TARGET, "load pool from storage, pool_id: {:?}", k);
				continue;
			};

			reads += pool.assets.len() as u64;
			let target_pegs = match Pallet::<T>::get_target_pegs(&pool.assets, &peg_info_v0.source) {
				Ok(p) => p,
				Err(e) => {
					log::error!(target: LOG_TARGET, "to get target pegs, pool_id: {:?}, err: {:?}", k, e);
					continue;
				}
			};

			let current_pegs_updated_at = target_pegs
				.iter()
				.map(|e| e.1)
				.min()
				.unwrap_or(current_block.saturated_into());

			let max_peg_update = if k == 690 {
				//GDOT
				peg_info_v0.max_peg_update.int_mul(60)
			} else {
				peg_info_v0.max_peg_update.int_mul(20)
			};

			let (trade_fee, new_pegs) = if let Some(p) = hydra_dx_math::stableswap::recalculate_pegs(
				&peg_info_v0.current,
				current_pegs_updated_at,
				&target_pegs,
				current_block.saturated_into::<u128>(),
				max_peg_update,
				pool.fee,
			) {
				p
			} else {
				log::error!(target: LOG_TARGET, "to recalculate pegs, pool_id: {:?}", k);
				continue;
			};

			writes += 1;
			BlockFee::<T>::insert(k, trade_fee);

			migrated_pegs_info.push((
				k,
				PoolPegInfo {
					source: peg_info_v0.source,
					max_peg_update: peg_info_v0.max_peg_update,
					updated_at: current_block,
					current: BoundedPegs::truncate_from(new_pegs),
				},
			));
		}

		writes += migrated_pegs_info.len() as u64;
		for (k, v) in &migrated_pegs_info {
			PoolPegs::<T>::insert(k, v);
		}

		log::info!(target: LOG_TARGET, "migration finished, migrated: {:?} pools", migrated_pegs_info.len());
		T::DbWeight::get().reads_writes(reads, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		use crate::migrations::StorageVersion;
		ensure!(
			StorageVersion::get::<Pallet<T>>() == 0,
			"can only upgrade from version 0"
		);

		Ok(Vec::new())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		for k in PoolPegs::<T>::iter_keys() {
			let _ = PoolPegs::<T>::get(k).expect("PoolPegInfo must be valid");
		}
		Ok(())
	}
}

pub type MigrateV0ToV1<T> =
	VersionedMigration<0, 1, unversioned::InnerMigrateV0ToV1<T>, Pallet<T>, <T as frame_system::Config>::DbWeight>;
