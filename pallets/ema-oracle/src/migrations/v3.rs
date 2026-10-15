use crate::*;
use frame_support::migrations::VersionedMigration;
use frame_support::traits::UncheckedOnRuntimeUpgrade;
use frame_support::weights::Weight;
use hydra_dx_math::ema::{arithmetic_price_weighted_average, exp_smoothing};
use sp_arithmetic::traits::SaturatedConversion;
use sp_std::collections::btree_map::BTreeMap;

type OracleKey = (Source, (AssetId, AssetId), OraclePeriod);
type StoredOracle<BlockNumber> = (OracleEntry<BlockNumber>, BlockNumber);

mod unversioned {
	pub struct InnerMigrateV2ToV3<T: crate::Config>(core::marker::PhantomData<T>);
}

fn parent_block<T: crate::Config>() -> BlockNumberFor<T> {
	T::BlockNumberProvider::current_block_number().saturating_sub(One::one())
}

fn settle_oracles<T: crate::Config>(
	parent: BlockNumberFor<T>,
	oracles: Vec<(OracleKey, StoredOracle<BlockNumberFor<T>>)>,
) -> Vec<(OracleKey, StoredOracle<BlockNumberFor<T>>)> {
	let last_blocks: BTreeMap<_, _> = oracles
		.iter()
		.filter_map(|((source, assets, period), (entry, _))| {
			if *period != OraclePeriod::LastBlock {
				return None;
			}
			let mut entry = entry.clone();
			if entry.updated_at < parent {
				entry.fast_forward_to(parent);
			}
			Some(((*source, *assets), entry))
		})
		.collect();

	oracles
		.into_iter()
		.map(|(key @ (source, assets, period), (mut entry, initialized))| {
			if period == OraclePeriod::LastBlock {
				if entry.updated_at < parent {
					entry.fast_forward_to(parent);
				}
				return (key, (entry, initialized));
			}

			if entry.updated_at < parent {
				if let Some(last_block) = last_blocks.get(&(source, assets)) {
					let iterations = parent.saturating_sub(entry.updated_at).saturated_into();
					if let Some(mut settled) = entry.calculate_current_from_outdated(period, last_block) {
						let smoothing = exp_smoothing(into_smoothing(period), iterations);
						settled.price = arithmetic_price_weighted_average(entry.price, last_block.price, smoothing);
						entry = settled;
					}
				} else {
					log::warn!(
						target: LOG_TARGET,
						"Cannot settle EMA oracle ({source:?}, {assets:?}, {period:?}) without LastBlock entry"
					);
				}
			}

			(key, (entry, initialized))
		})
		.collect()
}

impl<T: crate::Config> UncheckedOnRuntimeUpgrade for unversioned::InnerMigrateV2ToV3<T> {
	fn on_runtime_upgrade() -> Weight {
		let parent = parent_block::<T>();
		let previous: Vec<_> = Oracles::<T>::iter().collect();
		let reads = previous.len() as u64;
		let settled = settle_oracles::<T>(parent, previous.clone());
		let mut writes = 0u64;

		for ((previous_key, previous_value), (key, value)) in previous.into_iter().zip(settled) {
			debug_assert_eq!(previous_key, key);
			if previous_value != value {
				Oracles::<T>::insert(key, value);
				writes = writes.saturating_add(1);
			}
		}

		log::info!(target: LOG_TARGET, "v2->v3 migration settled {writes:?} of {reads:?} EMA entries");
		T::DbWeight::get().reads_writes(reads, writes)
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		use codec::Encode;
		use frame_support::traits::StorageVersion;

		ensure!(
			StorageVersion::get::<Pallet<T>>() == 2,
			"can only upgrade from version 2"
		);
		let expected = settle_oracles::<T>(parent_block::<T>(), Oracles::<T>::iter().collect());
		Ok((expected.len() as u64, sp_core::hashing::blake2_256(&expected.encode())).encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		use codec::{Decode, Encode};
		use frame_support::traits::StorageVersion;

		let (expected_count, expected_hash) =
			<(u64, [u8; 32])>::decode(&mut state.as_slice()).map_err(|_| "failed to decode pre-upgrade EMA state")?;
		let actual: Vec<_> = Oracles::<T>::iter().collect();
		ensure!(StorageVersion::get::<Pallet<T>>() == 3, "storage version must be 3");
		ensure!(actual.len() as u64 == expected_count, "oracle count must not change");
		ensure!(
			sp_core::hashing::blake2_256(&actual.encode()) == expected_hash,
			"oracles must be settled with the legacy price average"
		);
		Ok(())
	}
}

pub type MigrateV2ToV3<T> =
	VersionedMigration<2, 3, unversioned::InnerMigrateV2ToV3<T>, Pallet<T>, <T as frame_system::Config>::DbWeight>;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::tests::{new_test_ext, EmaOracle, System, Test, SOURCE};
	use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};
	use hydra_dx_math::ema::{arithmetic_price_weighted_average, price_weighted_average};

	#[test]
	fn migration_should_preserve_legacy_projection_when_oracle_is_stale() {
		new_test_ext().execute_with(|| {
			System::set_block_number(11);
			let previous = OracleEntry::new(
				Price::one(),
				Volume::from_a_in_b_out(10, 20),
				Liquidity::new(100, 200),
				Some(300),
				9,
			);
			let last_block = OracleEntry::new(
				Price::new(2, 1),
				Volume::from_a_in_b_out(30, 40),
				Liquidity::new(150, 250),
				Some(350),
				9,
			);
			let smoothing = into_smoothing(OraclePeriod::TenMinutes);
			let legacy_price = arithmetic_price_weighted_average(previous.price, last_block.price, smoothing);
			let symmetric_price = price_weighted_average(previous.price, last_block.price, smoothing);
			assert_ne!(legacy_price, symmetric_price);

			Oracles::<Test>::insert((SOURCE, (1, 2), OraclePeriod::LastBlock), (last_block.clone(), 3));
			Oracles::<Test>::insert((SOURCE, (1, 2), OraclePeriod::TenMinutes), (previous, 3));
			StorageVersion::new(2).put::<Pallet<Test>>();

			MigrateV2ToV3::<Test>::on_runtime_upgrade();

			let (migrated, initialized) = EmaOracle::oracle((SOURCE, (1, 2), OraclePeriod::TenMinutes)).unwrap();
			assert_eq!(migrated.price, legacy_price);
			assert_eq!(migrated.updated_at, 10);
			assert_eq!(initialized, 3);

			let (migrated_last_block, _) = EmaOracle::oracle((SOURCE, (1, 2), OraclePeriod::LastBlock)).unwrap();
			assert_eq!(migrated_last_block.updated_at, 10);
			assert_eq!(migrated_last_block.volume, Volume::default());
			assert_eq!(StorageVersion::get::<Pallet<Test>>(), 3);
		});
	}

	#[test]
	fn migration_should_preserve_last_block_volume_when_entry_is_current() {
		new_test_ext().execute_with(|| {
			System::set_block_number(11);
			let last_block = OracleEntry::new(
				Price::new(2, 1),
				Volume::from_a_in_b_out(30, 40),
				Liquidity::new(150, 250),
				Some(350),
				10,
			);
			Oracles::<Test>::insert((SOURCE, (1, 2), OraclePeriod::LastBlock), (last_block.clone(), 3));
			StorageVersion::new(2).put::<Pallet<Test>>();

			MigrateV2ToV3::<Test>::on_runtime_upgrade();

			assert_eq!(
				EmaOracle::oracle((SOURCE, (1, 2), OraclePeriod::LastBlock)),
				Some((last_block, 3))
			);
		});
	}
}
