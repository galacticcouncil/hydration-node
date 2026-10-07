//! Replays captured `IceSolverApi_solver_input` results through the node's solve path and
//! reports what the solver did with one intent. Run manually, not in CI.
//!
//!   ice-replay <dir> <intent id> [<real fills>] [--counterfactual] [--min-partial-fill-ppm N]
//!
//! `<dir>` holds one `<block>.bin` per block: the raw SCALE `Option<SolverInput>` a
//! `state_call("IceSolverApi_solver_input", "0x", <block hash>)` returns. The solution built
//! from block N's input executes in block N + 1, so `<real fills>` — lines of
//! `<block> <amount_in>` taken from the intent's `IntentResovedPartially` / `IntentResolved`
//! events — is compared against the replay shifted by one block.
//!
//! `--counterfactual` ignores the intent's recorded progress and carries forward the fills the
//! replay itself made, re-adding the intent after its real completion while it still has a
//! remainder. Pool state stays the real one, so the impact of fills that differ from history on
//! later blocks is not modelled.

use amm_simulator::HydrationSimulator;
use codec::Decode;
use hydradx_runtime::{HydrationSimulators, SimulatorPriceDenom, SmartRouteFinder};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::{IceSolver, SolverOptions};
use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Partial};
use pallet_ice_runtime_api::SolverInput;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BTreeMap;

thread_local! {
	static ED: RefCell<BTreeMap<AssetId, Balance>> = const { RefCell::new(BTreeMap::new()) };
}

/// The node's `NodeSimulatorConfig`: existential deposits come from the captured input.
struct ReplayConfig;
impl SimulatorConfig for ReplayConfig {
	type Simulators = HydrationSimulators;
	type RouteDiscovery = SmartRouteFinder<HydrationSimulators>;
	type PriceDenominator = SimulatorPriceDenom;

	fn existential_deposit(asset_id: AssetId) -> Balance {
		ED.with(|m| m.borrow().get(&asset_id).copied().unwrap_or(0))
	}
}

type Solver = ice_solver::v4::Solver<HydrationSimulator<ReplayConfig>>;

fn with_filled(intent: &Intent, filled: Balance) -> Intent {
	let mut intent = intent.clone();
	if let IntentData::Swap(swap) = &mut intent.data {
		swap.partial = Partial::Yes(filled);
	}
	intent
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let positional: Vec<&String> = args
		.iter()
		.enumerate()
		.skip(1)
		.filter(|(i, a)| !a.starts_with("--") && args[i - 1] != "--min-partial-fill-ppm")
		.map(|(_, a)| a)
		.collect();
	let flag_value = |name: &str| {
		args.iter()
			.position(|a| a == name)
			.and_then(|i| args.get(i + 1))
			.map(|v| v.parse::<u32>().expect("numeric flag value"))
	};
	let dir = positional
		.first()
		.expect("usage: ice-replay <dir> <intent id> [<real fills>] [flags]");
	let id: IntentId = positional.get(1).expect("intent id").parse().expect("intent id");
	let counterfactual = args.iter().any(|a| a == "--counterfactual");
	let mut options = SolverOptions::default();
	if let Some(ppm) = flag_value("--min-partial-fill-ppm") {
		options.min_partial_fill = sp_runtime::Permill::from_parts(ppm);
	}
	let real: BTreeMap<u32, Balance> = positional
		.get(2)
		.map(|path| {
			std::fs::read_to_string(path)
				.expect("real fills file")
				.lines()
				.filter_map(|l| {
					let mut it = l.split_whitespace();
					Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
				})
				.collect()
		})
		.unwrap_or_default();

	let mut blocks: Vec<u32> = std::fs::read_dir(dir)
		.expect("input dir")
		.filter_map(|e| e.ok()?.path().file_stem()?.to_str()?.parse().ok())
		.collect();
	blocks.sort_unstable();

	let mut template: Option<Intent> = None;
	let mut filled: Balance = 0;
	let mut fills: Vec<(u32, Balance, Balance, usize)> = Vec::new();
	let (mut same, mut differ, mut replay_only, mut real_only) = (0u32, 0u32, 0u32, 0u32);

	for block in blocks {
		let bytes = std::fs::read(format!("{dir}/{block}.bin")).expect("input file");
		let Some(mut input) = Option::<SolverInput>::decode(&mut &bytes[..]).expect("SolverInput decodes") else {
			continue;
		};
		match input.intents.iter().position(|i| i.id == id) {
			Some(pos) => {
				if template.is_none() {
					template = Some(input.intents[pos].clone());
				}
				if counterfactual {
					input.intents[pos] = with_filled(&input.intents[pos], filled);
				}
			}
			None if counterfactual => {
				if let Some(t) = &template {
					input.intents.push(with_filled(t, filled));
					input.intents.sort_by_key(|i| Reverse(i.id));
				}
			}
			None => {}
		}
		let Some(total) = template.as_ref().map(|t| t.data.amount_in()) else {
			continue;
		};
		if counterfactual && filled >= total {
			break;
		}

		ED.with(|m| {
			let mut m = m.borrow_mut();
			m.clear();
			m.extend(input.existential_deposits.iter().copied());
		});
		let state = <HydrationSimulators as SimulatorSet>::State::decode(&mut &input.state[..]).expect("state decodes");
		let min_outs = input.min_amount_out.iter().copied().collect();
		let fill = Solver::solve_with_options(input.intents, min_outs, state, input.fee, &options)
			.ok()
			.and_then(|s| {
				let trades = s.trades.len();
				s.resolved_intents
					.iter()
					.find(|r| r.id == id)
					.map(|r| (r.data.amount_in(), r.data.amount_out(), trades))
			});

		let executed_in = block + 1;
		if let Some((amount_in, amount_out, trades)) = fill {
			fills.push((executed_in, amount_in, amount_out, trades));
			if counterfactual {
				filled = filled.saturating_add(amount_in);
			}
		}
		if !counterfactual && !real.is_empty() {
			match (fill.map(|f| f.0), real.get(&executed_in).copied()) {
				(Some(a), Some(b)) if a == b => same += 1,
				(Some(_), Some(_)) => differ += 1,
				(Some(_), None) => replay_only += 1,
				(None, Some(_)) => real_only += 1,
				(None, None) => {}
			}
		}
	}

	for (block, amount_in, amount_out, trades) in &fills {
		println!("fill in #{block}: {amount_in} in -> {amount_out} out ({trades} trades)");
	}
	let total_in: Balance = fills.iter().map(|f| f.1).sum();
	let total = template.map(|t| t.data.amount_in()).unwrap_or_default();
	println!(
		"{} fills, {} of {} filled ({:.2} %), first #{}, last #{}",
		fills.len(),
		total_in,
		total,
		total_in as f64 * 100.0 / total.max(1) as f64,
		fills.first().map(|f| f.0).unwrap_or_default(),
		fills.last().map(|f| f.0).unwrap_or_default(),
	);
	if !counterfactual && !real.is_empty() {
		println!(
			"against the chain: {same} identical, {differ} different amount, {replay_only} replay-only, {real_only} chain-only (of {} real fills)",
			real.len()
		);
	}
}
