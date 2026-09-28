use frame_support::traits::OnRuntimeUpgrade;
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_support::{IntentData, Partial, SwapData};

/// Continuous property-soak / fuzzing harness for the ICE solver and on-chain
/// solution submission. Standalone manual utility — see `bin/ice_fuzz.rs`.
pub mod fuzz;

// Re-export both Intent types under distinct names
pub use ice_support::Intent as SolverIntent;
pub use pallet_intent::types::Intent as StorageIntent;

pub type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

pub fn load_snapshot(path: &str) -> frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block> {
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

/// Generate `count` unresolvable intents with a unique `amount_in` each, so every
/// intent misses the solver's `(pair, amount_in)` quote cache and pays its own
/// authoritative route quote. The homogeneous generator above collapses into a
/// single cache entry — this one measures the adversarial worst case.
pub fn generate_heterogeneous_unresolvable_intents(count: usize) -> Vec<SolverIntent> {
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	(0..count)
		.map(|i| SolverIntent {
			id: (i + 300_000) as u128,
			data: IntentData::Swap(SwapData {
				asset_in: hdx,
				asset_out: bnc,
				amount_in: hdx_unit + i as u128,
				amount_out: 1_000_000 * bnc_unit,
				partial: Partial::No,
			}),
		})
		.collect()
}

/// Generate `count` unresolvable intents spread across distinct pairs: `asset_out`
/// rotates through the low registry id space, so the batch mixes a few routable
/// targets with masses of unroutable ones. Each distinct pair pays its own
/// spot-price probe and route discovery (exhaustive BFS for unroutable targets)
/// instead of sharing the per-pair route cache — the cross-pair worst case.
pub fn generate_cross_pair_spam_intents(count: usize) -> Vec<SolverIntent> {
	let hdx = 0u32;
	let hdx_unit = 1_000_000_000_000u128;

	(0..count)
		.map(|i| SolverIntent {
			id: (i + 400_000) as u128,
			data: IntentData::Swap(SwapData {
				asset_in: hdx,
				asset_out: 1 + (i as u32 % 1_400),
				amount_in: hdx_unit,
				amount_out: 1_000_000 * hdx_unit,
				partial: Partial::No,
			}),
		})
		.collect()
}

/// Generate a mixed batch: `resolvable` good intents + `unresolvable` bad intents, interleaved.
pub fn generate_mixed_intents(resolvable: usize, unresolvable: usize) -> Vec<SolverIntent> {
	interleave(
		generate_resolvable_intents(resolvable),
		generate_unresolvable_intents(unresolvable),
	)
}

/// Mixed batch with cache-defeating spam: `resolvable` good intents interleaved with
/// `unresolvable` heterogeneous bad ones.
pub fn generate_mixed_heterogeneous_intents(resolvable: usize, unresolvable: usize) -> Vec<SolverIntent> {
	interleave(
		generate_resolvable_intents(resolvable),
		generate_heterogeneous_unresolvable_intents(unresolvable),
	)
}

fn interleave(good: Vec<SolverIntent>, bad: Vec<SolverIntent>) -> Vec<SolverIntent> {
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

/// WETH / ETH. Their route on `mainnet_apr` runs through Aave, which the
/// offline snapshot externalities leave without reserves, so here the pair has
/// no route until [`create_two_venue_pools`] adds two.
pub const TWO_VENUE_ASSETS: (u32, u32) = (20, 34);

/// Two pool-disjoint ETH -> WETH routes: a balanced 5/5 stableswap pool, and a
/// path through a synthetic token whose ETH-scarce first pool pays more for the
/// first ETH, so a transfer of a few ETH is split between them. Must be called
/// inside `execute_with`.
///
/// A second pool of the same pair would not do: the stableswap simulator looks
/// a pool up by its assets, not its id, so it would price both routes on one
/// curve. And WETH cannot be minted past the snapshot's circuit-breaker deposit
/// limit (~15 WETH on `mainnet_apr`) — the rest lands reserved, not free.
pub fn create_two_venue_pools() {
	use frame_support::storage::{with_transaction, TransactionOutcome};
	use hydradx_runtime::AssetRegistry;
	use hydradx_traits::{AssetKind, Create};

	const UNIT: u128 = 1_000_000_000_000_000_000;
	let (weth, eth) = TWO_VENUE_ASSETS;
	let register = |kind| {
		with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
				None,
				None,
				kind,
				1u128,
				None,
				Some(18),
				None,
				None,
			))
		})
		.expect("asset should register")
	};
	let bridge = register(AssetKind::Token);
	create_pool(register(AssetKind::StableSwap), [(weth, 5 * UNIT), (eth, 5 * UNIT)]);
	create_pool(register(AssetKind::StableSwap), [(eth, UNIT), (bridge, 2 * UNIT)]);
	create_pool(register(AssetKind::StableSwap), [(bridge, 2 * UNIT), (weth, 2 * UNIT)]);
}

fn create_pool(pool: u32, reserves: [(u32, u128); 2]) {
	use frame_support::assert_ok;
	use frame_support::BoundedVec;
	use hydradx_runtime::{Currencies, RuntimeOrigin, Stableswap};
	use hydradx_traits::stableswap::AssetAmount;

	let lp = primitives::AccountId::from([7u8; 32]);
	assert_ok!(Stableswap::create_pool(
		RuntimeOrigin::root(),
		pool,
		BoundedVec::truncate_from(reserves.iter().map(|(asset, _)| *asset).collect()),
		100,
		sp_runtime::Permill::from_rational(1u32, 10_000u32),
	));
	for (asset, amount) in reserves {
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			lp.clone(),
			asset,
			amount as i128,
		));
	}
	assert_ok!(Stableswap::add_assets_liquidity(
		RuntimeOrigin::signed(lp),
		pool,
		BoundedVec::truncate_from(
			reserves
				.iter()
				.map(|(asset, amount)| AssetAmount::new(*asset, *amount))
				.collect()
		),
		0,
	));
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
