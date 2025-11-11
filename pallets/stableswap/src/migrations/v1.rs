use crate::*;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::migrations::VersionedMigration;
use frame_support::traits::ConstU32;
use frame_support::traits::UncheckedOnRuntimeUpgrade;
use frame_support::Blake2_128Concat;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

const LOG_TARGET: &str = "runtime::stableswap";

mod v0 {
	use super::*;
	use frame_support::storage_alias;

	#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct PoolInfo<AssetId, BlockNumber> {
		pub assets: BoundedVec<AssetId, ConstU32<MAX_ASSETS_IN_POOL>>,
		pub initial_amplification: NonZeroU16,
		pub final_amplification: NonZeroU16,
		pub initial_block: BlockNumber,
		pub final_block: BlockNumber,
		pub fee: Permill,
	}

	#[storage_alias()]
	pub type Pools<T: crate::Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		<T as crate::Config>::AssetId,
		PoolInfo<<T as crate::Config>::AssetId, BlockNumberFor<T>>,
	>;
}

// Private module to hide migration
mod unversioned {
	pub struct InnerMigrateV0ToV1<T: crate::Config>(core::marker::PhantomData<T>);
}

impl<T: crate::Config> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV0ToV1<T> {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		log::info!(target: LOG_TARGET, "v0->v1 migration started");

		let mut reads: u64 = 0;
		let mut writes: u64 = 0;

		let mut migrated_pools: Vec<(T::AssetId, PoolInfo<T::AssetId, BlockNumberFor<T>>)> = Vec::with_capacity(12);
		let current_block = 100_u32; //T::BlockNumberProvider::current_block_number();

		for (k, pool_v0) in v0::Pools::<T>::iter() {
			reads += 1;

			let mut pool_v1 = PoolInfo {
				assets: pool_v0.assets,
				initial_amplification: pool_v0.initial_amplification,
				final_amplification: pool_v0.final_amplification,
				initial_block: pool_v0.initial_block,
				final_block: pool_v0.final_block,
				fee: pool_v0.fee,
				pegs_info: None,
			};

			if let Some(peg_info) = PoolPegs::<T>::get(k) {
				log::info!(target: LOG_TARGET, "updating pegs for pool_id: {:?}", k);
				reads += pool_v1.assets.len() as u64;
				let target_pegs = match Pallet::<T>::get_target_pegs(&pool_v1.assets, &peg_info.source) {
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

				let (trade_fee, new_pegs) = if let Some(p) = hydra_dx_math::stableswap::recalculate_pegs(
					&peg_info.current,
					current_pegs_updated_at,
					&target_pegs,
					current_block.saturated_into::<u128>(),
					peg_info.max_peg_update,
					pool_v1.fee,
				) {
					p
				} else {
					log::error!(target: LOG_TARGET, "to recalculate pegs, pool_id: {:?}", k);
					continue;
				};

				writes += 1;
				let new_info = peg_info.with_new_pegs(&new_pegs);
				PoolPegs::<T>::insert(k, new_info);

				pool_v1.pegs_info = Some(PegUpateInfo {
					updated_at: current_block.into(),
					updated_fee: trade_fee,
				});
			}

			migrated_pools.push((k, pool_v1));
		}

		writes += migrated_pools.len() as u64;
		for (k, v) in &migrated_pools {
			Pools::<T>::insert(k, v);
		}

		log::info!(target: LOG_TARGET, "migration finished, migrated: {:?} pools", migrated_pools.len());
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
		for k in Pools::<T>::iter_keys() {
			let _ = Pools::<T>::get(k).expect("Pool must be valid");
		}
		Ok(())
	}
}

pub type MigrateV0ToV1<T> =
	VersionedMigration<0, 1, unversioned::InnerMigrateV0ToV1<T>, Pallet<T>, <T as frame_system::Config>::DbWeight>;
