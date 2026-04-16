use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

use amm_simulator::HydrationSimulator;
use ice_solver::v2::Solver as IceSolver;
use ice_solver_bench::{
	clear_intent_storage, generate_mixed_intents, generate_mixed_partial_intents, generate_partial_intents,
	generate_resolvable_intents, generate_unresolvable_intents, get_initial_state, load_snapshot,
	populate_intent_storage,
};
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
				ext.execute_with(|| Solver::solve(black_box(intents.clone()), black_box(state.clone())))
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
				ext.execute_with(|| Solver::solve(black_box(intents.clone()), black_box(state.clone())))
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
		let label = format!("{}good_{}bad", good, bad);
		group.bench_with_input(BenchmarkId::new("intents", &label), &label, |b, _| {
			b.iter(|| {
				ext.execute_with(|| Solver::solve(black_box(intents.clone()), black_box(state.clone())))
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
				ext.execute_with(|| Solver::solve(black_box(intents.clone()), black_box(state.clone())))
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
		let label = format!("{}np_{}p", np, p);
		group.bench_with_input(BenchmarkId::new("intents", &label), &label, |b, _| {
			b.iter(|| {
				ext.execute_with(|| Solver::solve(black_box(intents.clone()), black_box(state.clone())))
			})
		});
	}
	group.finish();
}

criterion_group!(
	benches,
	bench_initial_state,
	bench_get_valid_intents,
	bench_resolvable,
	bench_unresolvable,
	bench_mixed,
	bench_partial,
	bench_mixed_partial
);
criterion_main!(benches);
