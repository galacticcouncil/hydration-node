use frame_support::traits::OnRuntimeUpgrade;
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_support::{IntentData, Partial, SwapData};

// Re-export both Intent types under distinct names
pub use ice_support::Intent as SolverIntent;
pub use pallet_intent::types::Intent as StorageIntent;

pub type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

pub fn load_snapshot(
	path: &str,
) -> frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block> {
	tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()
		.unwrap()
		.block_on(async {
			use frame_remote_externalities::*;

			let snapshot_config = SnapshotConfig::from(String::from(path));
			let offline_config = OfflineConfig {
				state_snapshot: snapshot_config,
			};
			let mode = Mode::Offline(offline_config);
			let builder = Builder::<hydradx_runtime::Block>::new().mode(mode);

			let mut p = builder.build().await.unwrap();
			p.execute_with(|| {
				pallet_ema_oracle::migrations::v1::MigrateV0ToV1::<hydradx_runtime::Runtime>::on_runtime_upgrade();
			});
			p
		})
}

/// Must be called inside `execute_with` — reads pool state from storage.
pub fn get_initial_state() -> CombinedSimulatorState {
	<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators::initial_state()
}

/// Generate `count` resolvable intents (alternating HDX→BNC and BNC→HDX).
pub fn generate_resolvable_intents(count: usize) -> Vec<SolverIntent> {
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	(0..count)
		.map(|i| {
			let (asset_in, asset_out, amount_in, amount_out) = if i % 2 == 0 {
				(hdx, bnc, 500 * hdx_unit, bnc_unit)
			} else {
				(bnc, hdx, 30 * bnc_unit, hdx_unit)
			};
			SolverIntent {
				id: i as u128 + 1,
				data: IntentData::Swap(SwapData {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
					partial: Partial::No,
				}),
			}
		})
		.collect()
}

/// Generate `count` unresolvable intents (absurd min_out that no AMM can satisfy).
pub fn generate_unresolvable_intents(count: usize) -> Vec<SolverIntent> {
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	(0..count)
		.map(|i| {
			// Sell 1 HDX, demand 1_000_000 BNC — impossible
			SolverIntent {
				id: (i + 100_000) as u128,
				data: IntentData::Swap(SwapData {
					asset_in: hdx,
					asset_out: bnc,
					amount_in: hdx_unit,
					amount_out: 1_000_000 * bnc_unit,
					partial: Partial::No,
				}),
			}
		})
		.collect()
}

/// Generate a mixed batch: `resolvable` good intents + `unresolvable` bad intents, interleaved.
pub fn generate_mixed_intents(resolvable: usize, unresolvable: usize) -> Vec<SolverIntent> {
	let good = generate_resolvable_intents(resolvable);
	let bad = generate_unresolvable_intents(unresolvable);

	let mut mixed = Vec::with_capacity(good.len() + bad.len());
	let mut gi = good.into_iter();
	let mut bi = bad.into_iter();
	loop {
		match (gi.next(), bi.next()) {
			(Some(g), Some(b)) => {
				mixed.push(g);
				mixed.push(b);
			}
			(Some(g), None) => mixed.push(g),
			(None, Some(b)) => mixed.push(b),
			(None, None) => break,
		}
	}
	mixed
}

/// Insert `count` swap intents directly into pallet-intent storage.
/// Must be called inside `execute_with`.
pub fn populate_intent_storage(count: usize) {
	use ice_support::SwapData as IceSwapData;

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	for i in 0..count {
		let id = (i + 1) as u128;
		let (asset_in, asset_out, amount_in, amount_out) = if i % 2 == 0 {
			(hdx, bnc, 500 * hdx_unit, bnc_unit)
		} else {
			(bnc, hdx, 30 * bnc_unit, hdx_unit)
		};

		let intent = StorageIntent {
			data: IntentData::Swap(IceSwapData {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				partial: Partial::No,
			}),
			deadline: None,
			on_resolved: None,
		};

		pallet_intent::Intents::<hydradx_runtime::Runtime>::insert(id, intent);
	}
}

/// Remove all intents from storage. Must be called inside `execute_with`.
pub fn clear_intent_storage() {
	let _ = pallet_intent::Intents::<hydradx_runtime::Runtime>::clear(u32::MAX, None);
}

/// Generate `count` partial-fill intents (alternating HDX→BNC and BNC→HDX).
/// Uses large amounts with tight limits to exercise the binary search.
pub fn generate_partial_intents(count: usize) -> Vec<SolverIntent> {
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	(0..count)
		.map(|i| {
			let (asset_in, asset_out, amount_in, amount_out) = if i % 2 == 0 {
				// Large HDX→BNC with tight limit (~0.065 BNC/HDX, spot is ~0.068)
				(hdx, bnc, 500_000 * hdx_unit, 32_500 * bnc_unit)
			} else {
				// Large BNC→HDX with tight limit
				(bnc, hdx, 30_000 * bnc_unit, 400_000 * hdx_unit)
			};
			SolverIntent {
				id: (i + 200_000) as u128,
				data: IntentData::Swap(SwapData {
					asset_in,
					asset_out,
					amount_in,
					amount_out,
					partial: Partial::Yes(0),
				}),
			}
		})
		.collect()
}

/// Generate a batch with `non_partial` non-partial + `partial` partial intents.
pub fn generate_mixed_partial_intents(non_partial: usize, partial: usize) -> Vec<SolverIntent> {
	let mut intents = generate_resolvable_intents(non_partial);
	let mut partials = generate_partial_intents(partial);
	intents.append(&mut partials);
	intents
}
