// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

use codec::Encode;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_dynamic_fees::{pallet, types::AssetFeeConfig, AssetFeeConfiguration};
use sp_core::Get;
use sp_runtime::{FixedU128, Saturating};
use sp_std::{marker::PhantomData, vec::Vec};

const MIGRATION_DONE_KEY: &[u8] = b"HydrationDynamicFees2sMigrationDone";
// A defensive bound for custom fee configurations checked by try-runtime before release.
const MAX_ASSET_FEE_CONFIGURATIONS: u64 = 100;

/// Scales custom per-block fee decay to preserve wall-clock behavior across the 6s to 2s transition.
///
/// Fee timestamps remain unchanged because migrating them independently from EMA timestamps would break their pairing.
pub struct MigrateDynamicFeeDecayTo2sBlocks<T: pallet::Config>(PhantomData<T>);

impl<T: pallet::Config> MigrateDynamicFeeDecayTo2sBlocks<T> {
	fn is_done() -> bool {
		sp_io::storage::get(MIGRATION_DONE_KEY).is_some()
	}

	fn mark_done() {
		sp_io::storage::set(MIGRATION_DONE_KEY, &true.encode());
	}

	fn scale_decay(decay: FixedU128) -> FixedU128 {
		let inner = decay.into_inner();
		// A configured non-zero decay must not become permanently non-decaying due to integer truncation.
		FixedU128::from_inner(if inner == 0 { 0 } else { (inner / 3).max(1) })
	}
}

impl<T: pallet::Config> OnRuntimeUpgrade for MigrateDynamicFeeDecayTo2sBlocks<T> {
	fn on_runtime_upgrade() -> Weight {
		if Self::is_done() {
			log::warn!("MigrateDynamicFeeDecayTo2sBlocks already executed");
			return T::DbWeight::get().reads(1);
		}

		let configurations: Vec<_> = AssetFeeConfiguration::<T>::iter().collect();
		let configurations_len = configurations.len() as u64;
		let reads = 1u64.saturating_add(configurations_len);

		log::info!(
			"MigrateDynamicFeeDecayTo2sBlocks found asset fee configurations: {configurations_len:?}, cap: {MAX_ASSET_FEE_CONFIGURATIONS:?}",
		);

		if configurations_len > MAX_ASSET_FEE_CONFIGURATIONS {
			log::error!(
				"MigrateDynamicFeeDecayTo2sBlocks skipped because AssetFeeConfiguration has {configurations_len:?} entries, cap: {MAX_ASSET_FEE_CONFIGURATIONS:?}",
			);
			return T::DbWeight::get().reads(reads);
		}

		let mut migrated = 0u64;
		let mut writes = 0u64;

		for (asset, configuration) in configurations {
			let AssetFeeConfig::Dynamic {
				mut asset_fee_params,
				mut protocol_fee_params,
			} = configuration
			else {
				continue;
			};

			asset_fee_params.decay = Self::scale_decay(asset_fee_params.decay);
			protocol_fee_params.decay = Self::scale_decay(protocol_fee_params.decay);
			AssetFeeConfiguration::<T>::insert(
				asset,
				AssetFeeConfig::Dynamic {
					asset_fee_params,
					protocol_fee_params,
				},
			);
			writes.saturating_inc();
			migrated.saturating_inc();
		}

		Self::mark_done();
		writes.saturating_inc();

		log::info!(
			"MigrateDynamicFeeDecayTo2sBlocks checked configurations: {configurations_len:?}, migrated dynamic configurations: {migrated:?}",
		);

		T::DbWeight::get().reads_writes(reads, writes)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{AssetId, Runtime};
	use pallet_dynamic_fees::types::FeeParams;
	use sp_runtime::Permill;

	fn fee_params(decay: FixedU128) -> FeeParams<Permill> {
		FeeParams {
			min_fee: Permill::from_perthousand(1),
			max_fee: Permill::from_percent(1),
			decay,
			amplification: FixedU128::from(2),
		}
	}

	#[test]
	fn migration_should_scale_only_dynamic_decay_when_moving_to_2s_blocks() {
		let mut ext = sp_io::TestExternalities::new_empty();

		ext.execute_with(|| {
			let dynamic_asset: AssetId = 1;
			let fixed_asset: AssetId = 2;
			let dynamic_config = AssetFeeConfig::Dynamic {
				asset_fee_params: fee_params(FixedU128::from_rational(3, 10_000)),
				protocol_fee_params: fee_params(FixedU128::from_rational(3, 20_000)),
			};
			let fixed_config = AssetFeeConfig::Fixed {
				asset_fee: Permill::from_percent(1),
				protocol_fee: Permill::from_perthousand(1),
			};

			AssetFeeConfiguration::<Runtime>::insert(dynamic_asset, dynamic_config);
			AssetFeeConfiguration::<Runtime>::insert(fixed_asset, fixed_config);

			MigrateDynamicFeeDecayTo2sBlocks::<Runtime>::on_runtime_upgrade();

			let Some(AssetFeeConfig::Dynamic {
				asset_fee_params,
				protocol_fee_params,
			}) = AssetFeeConfiguration::<Runtime>::get(dynamic_asset)
			else {
				panic!("dynamic configuration must remain dynamic");
			};
			assert_eq!(asset_fee_params.decay, FixedU128::from_rational(1, 10_000));
			assert_eq!(protocol_fee_params.decay, FixedU128::from_rational(1, 20_000));
			assert_eq!(AssetFeeConfiguration::<Runtime>::get(fixed_asset), Some(fixed_config));

			MigrateDynamicFeeDecayTo2sBlocks::<Runtime>::on_runtime_upgrade();

			let Some(AssetFeeConfig::Dynamic { asset_fee_params, .. }) =
				AssetFeeConfiguration::<Runtime>::get(dynamic_asset)
			else {
				panic!("dynamic configuration must remain dynamic");
			};
			assert_eq!(asset_fee_params.decay, FixedU128::from_rational(1, 10_000));
		});
	}
}
