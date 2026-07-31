use crate::*;
use frame_support::migrations::VersionedMigration;
use frame_support::traits::UncheckedOnRuntimeUpgrade;

const LOG_TARGET: &str = "runtime::stableswap";

// Private module to hide migration
mod unversioned {
	pub struct InnerMigrateV1ToV2<T: crate::Config>(core::marker::PhantomData<T>);
}

impl<T: crate::Config> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV1ToV2<T> {
	fn on_runtime_upgrade() -> frame_support::weights::Weight {
		log::info!(target: LOG_TARGET, "v1->v2 migration started");

		let mut pools: u64 = 0;
		for pool_id in Pools::<T>::iter_keys() {
			ShareIssuance::<T>::insert(pool_id, T::Currency::total_issuance(pool_id));
			pools += 1;
		}

		log::info!(target: LOG_TARGET, "migration finished, seeded share issuance for {pools:?} pools");
		T::DbWeight::get().reads_writes(2 * pools, pools)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		use crate::migrations::StorageVersion;
		use codec::Encode;
		ensure!(
			StorageVersion::get::<Pallet<T>>() == 1,
			"can only upgrade from version 1"
		);

		Ok((Pools::<T>::iter_keys().count() as u32).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		use codec::Decode;
		let pre_pool_count =
			u32::decode(&mut state.as_slice()).map_err(|_| "failed to decode pre-upgrade pool count")?;

		let mut pools = 0u32;
		for pool_id in Pools::<T>::iter_keys() {
			ensure!(
				ShareIssuance::<T>::get(pool_id) == T::Currency::total_issuance(pool_id),
				"share issuance must match total issuance"
			);
			pools += 1;
		}
		ensure!(pools == pre_pool_count, "pool count must not change");
		Ok(())
	}
}

pub type MigrateV1ToV2<T> =
	VersionedMigration<1, 2, unversioned::InnerMigrateV1ToV2<T>, Pallet<T>, <T as frame_system::Config>::DbWeight>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::mock::*;
	use crate::tests::to_bounded_asset_vec;
	use frame_support::assert_ok;
	use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};
	use hydradx_traits::stableswap::AssetAmount;
	use sp_runtime::Permill;

	#[test]
	fn migration_should_seed_share_issuance_from_total_issuance_when_pool_exists() {
		let pool_id: AssetId = 100u32;
		ExtBuilder::default()
			.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (BOB, 2, 200 * ONE)])
			.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
			.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
			.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
			.build()
			.execute_with(|| {
				assert_ok!(Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					to_bounded_asset_vec(vec![1, 2]),
					100u16,
					Permill::from_percent(0),
				));
				assert_ok!(Stableswap::add_assets_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![AssetAmount::new(1, 100 * ONE), AssetAmount::new(2, 100 * ONE)]),
					0u128,
				));

				// Simulate pre-v2 chain state: no tracked issuance, on-chain version 1.
				ShareIssuance::<Test>::remove(pool_id);
				StorageVersion::new(1).put::<Pallet<Test>>();

				MigrateV1ToV2::<Test>::on_runtime_upgrade();

				assert_eq!(
					ShareIssuance::<Test>::get(pool_id),
					Tokens::total_issuance(pool_id),
					"tracked issuance must be seeded from total issuance"
				);
				assert_eq!(ShareIssuance::<Test>::get(pool_id), 200 * ONE * 1_000_000);
				assert_eq!(StorageVersion::get::<Pallet<Test>>(), 2);
			});
	}
}
