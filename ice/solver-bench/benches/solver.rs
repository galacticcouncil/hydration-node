use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use amm_simulator::HydrationSimulator;
use ice_solver::v4::Solver as IceSolver;
use ice_solver::{IceSolver as _, MinOuts, SolverOptions, SplitConfig};
use ice_solver_bench::{
	clear_intent_storage, create_two_venue_pools, generate_cross_pair_spam_intents,
	generate_heterogeneous_unresolvable_intents, generate_mixed_heterogeneous_intents, generate_mixed_intents,
	generate_mixed_partial_intents, generate_partial_intents, generate_resolvable_intents,
	generate_unresolvable_intents, get_initial_state, load_snapshot, populate_intent_storage, SolverIntent,
	TWO_VENUE_ASSETS,
};
use ice_support::{IntentData, Partial, SwapData};
use pallet_omnipool::types::SlipFeeConfig;
use sp_runtime::Permill;

type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

const SNAPSHOT_PATH: &str = "../../integration-tests/snapshots/ice/mainnet_apr";

fn enable_slip_fees() {
	frame_support::assert_ok!(pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::set_slip_fee(
		hydradx_runtime::RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

fn bench_initial_state(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);

	c.bench_function("simulator_initial_state", |b| {
		b.iter(|| {
			ext.execute_with(|| {
				black_box(get_initial_state());
			})
		})
	});
}

fn bench_resolvable(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_resolvable");
	for n in [10, 50, 100, 200] {
		let intents = generate_resolvable_intents(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_unresolvable(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_unresolvable");
	for n in [10, 50, 100, 500, 1000, 5000] {
		let intents = generate_unresolvable_intents(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_heterogeneous_spam(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_heterogeneous_spam");
	for n in [10, 50, 100, 500, 1000, 5000] {
		let intents = generate_heterogeneous_unresolvable_intents(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_cross_pair_spam(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_cross_pair_spam");
	// Potentially expensive per iteration (exhaustive BFS per distinct unroutable pair).
	group.sample_size(10);
	for n in [10, 50, 100, 500, 1400, 5000] {
		let intents = generate_cross_pair_spam_intents(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_mixed_heterogeneous(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_mixed_heterogeneous");
	for (good, bad) in [(50, 500), (50, 5000), (100, 5000)] {
		let intents = generate_mixed_heterogeneous_intents(good, bad);
		let label = format!("{good}good_{bad}bad");
		group.bench_with_input(BenchmarkId::new("intents", &label), &label, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_mixed(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_mixed");
	for (good, bad) in [(50, 50), (50, 500), (50, 5000), (100, 5000)] {
		let intents = generate_mixed_intents(good, bad);
		let label = format!("{good}good_{bad}bad");
		group.bench_with_input(BenchmarkId::new("intents", &label), &label, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_get_valid_intents(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);

	let mut group = c.benchmark_group("get_valid_intents");
	for n in [10, 50, 100, 500, 1000, 5000] {
		// Populate storage with n intents, then benchmark the read
		ext.execute_with(|| {
			clear_intent_storage();
			populate_intent_storage(n);
		});

		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					black_box(pallet_intent::Pallet::<hydradx_runtime::Runtime>::get_valid_intents());
				})
			})
		});
	}
	// Clean up
	ext.execute_with(clear_intent_storage);
	group.finish();
}

fn bench_partial(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_partial");
	for n in [1, 2, 5, 10, 20] {
		let intents = generate_partial_intents(n);
		group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

fn bench_mixed_partial(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(enable_slip_fees);
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_mixed_partial");
	// (non-partial, partial)
	for (np, p) in [(10, 1), (10, 5), (10, 10), (50, 10), (50, 50), (100, 20)] {
		let intents = generate_mixed_partial_intents(np, p);
		let label = format!("{np}np_{p}p");
		group.bench_with_input(BenchmarkId::new("intents", &label), &label, |b, _| {
			b.iter(|| {
				ext.execute_with(|| {
					Solver::solve(
						black_box(intents.clone()),
						black_box(state.clone()),
						black_box(Permill::zero()),
					)
				})
			})
		});
	}
	group.finish();
}

/// `count` sellers sharing 4 ETH of the two-venue pair — a total the split
/// fires at. Every other one is a partial whose 0.985 limit sits between the
/// single route's rate (0.980) and the split's (0.992), so without splitting
/// the crossing trims it through the fitting quotes.
fn two_venue_intents(count: usize) -> Vec<SolverIntent> {
	let (weth, eth) = TWO_VENUE_ASSETS;
	let amount_in = 4_000_000_000_000_000_000 / count as u128;
	(0..count)
		.map(|i| {
			let partial = i % 2 == 1;
			SolverIntent {
				id: i as u128 + 1,
				data: IntentData::Swap(SwapData {
					asset_in: eth,
					asset_out: weth,
					amount_in,
					amount_out: if partial { amount_in / 1_000 * 985 } else { 1 },
					partial: if partial { Partial::Yes(0) } else { Partial::No },
				}),
			}
		})
		.collect()
}

/// Transfers the split search actually works on — the existing groups trade
/// HDX/BNC, which has one venue, so they only ever take its early exit.
fn bench_split(c: &mut Criterion) {
	let mut ext = load_snapshot(SNAPSHOT_PATH);
	ext.execute_with(|| {
		enable_slip_fees();
		create_two_venue_pools();
	});
	let state = ext.execute_with(get_initial_state);

	let mut group = c.benchmark_group("solver_split");
	for (label, split) in [
		("omnipool_stableswap", SplitConfig::default()),
		("omnipool_stableswap_off", SplitConfig::disabled()),
	] {
		for n in [1, 10, 50] {
			let intents = two_venue_intents(n);
			group.bench_with_input(BenchmarkId::new(label, n), &n, |b, _| {
				b.iter(|| {
					ext.execute_with(|| {
						Solver::solve_with_options(
							black_box(intents.clone()),
							MinOuts::new(),
							black_box(state.clone()),
							black_box(Permill::zero()),
							&SolverOptions {
								split,
								..SolverOptions::default()
							},
						)
					})
				})
			});
		}
	}
	group.finish();
}

criterion_group!(
	benches,
	bench_initial_state,
	bench_get_valid_intents,
	bench_resolvable,
	bench_unresolvable,
	bench_heterogeneous_spam,
	bench_cross_pair_spam,
	bench_mixed,
	bench_mixed_heterogeneous,
	bench_partial,
	bench_mixed_partial,
	bench_split
);
criterion_main!(benches);
