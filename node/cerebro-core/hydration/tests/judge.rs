//! `guard::judge` against the monitor's recorded cassettes: the parent block is `PRE`, the block
//! itself `POST`. The signer's balances are then exact only when no other extrinsic of the block
//! touches the signer, so only such candidates are judged. Pools other extrinsics also touched are
//! assessed over the whole block and the block's trades are passed as committed trades, as the
//! monitor sees them, so the judged nets can be compared with the monitor's ledger.

use cerebro_core::model::{is_hook_ext, ChainEvent, Phase, Trade};
use cerebro_core::value::Usd6;
use cerebro_core::{reduce, BlockHash};
use cerebro_hydration::guard::{
	candidate_of, is_candidate, judge, plan, read_point, Accumulators, GuardCtx, Judgement, Thresholds, Verdict, POST,
	PRE,
};
use cerebro_hydration::pricing::Prices;
use cerebro_hydration::{accounts, balance, map_block, permit, xcm, RemoteApi, Transport};
use sp_core::H256;
use std::collections::{BTreeMap, HashMap};

/// Serves `PRE` from the parent block and `POST` from the block.
struct AtBlock<'a> {
	api: &'a RemoteApi,
	parent: H256,
	block: H256,
}

impl AtBlock<'_> {
	fn map(&self, at: BlockHash) -> BlockHash {
		match at {
			PRE => self.parent.0,
			POST => self.block.0,
			other => other,
		}
	}
}

impl Transport for AtBlock<'_> {
	fn storage_many(&self, keys: &[Vec<u8>], at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
		self.api.storage_many(keys, self.map(at))
	}
	fn evm_view(&self, to: [u8; 20], data: &[u8], at: BlockHash) -> Result<Vec<u8>, String> {
		self.api.evm_view(to, data, self.map(at))
	}
	fn storage_keys(&self, prefix: &[u8], at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
		Transport::storage_keys(self.api, prefix, self.map(at))
	}
	fn block_number(&self, at: BlockHash) -> Result<u32, String> {
		self.api.block_number(self.map(at))
	}
}

fn thresholds() -> Thresholds {
	Thresholds {
		min_usd: Usd6::from_dollars(50.0),
		min_pool_leak: Usd6::from_dollars(100.0),
		min_block_usd: Usd6::from_dollars(200.0),
		min_block_pool_leak: Usd6::from_dollars(500.0),
		pool_leak_epsilon_bps: 1,
	}
}

/// What the node runs with: any pool leak holds, signer gain never does.
fn node_thresholds() -> Thresholds {
	Thresholds {
		min_usd: Usd6(i128::MAX),
		min_pool_leak: Usd6::ZERO,
		min_block_usd: Usd6(i128::MAX),
		min_block_pool_leak: Usd6::ZERO,
		pool_leak_epsilon_bps: 0,
	}
}

fn replay(name: &str) -> RemoteApi {
	let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}.json"));
	let fixture = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
	RemoteApi::replaying(&fixture).expect("replay api")
}

struct Judged {
	number: u32,
	ext: u32,
	judgement: Judgement,
}

/// Which trades of the block count as committed before the candidate.
#[derive(Clone, Copy, Debug)]
enum Position {
	/// First in the block: only its own trades.
	First,
	/// Where it really was: the trades before it.
	Actual,
	/// Last in the block: every trade of the block, as the monitor prices.
	Last,
}

/// Judge every candidate in `from..=to` whose signer no other extrinsic of the block touches.
fn judge_range(name: &str, from: u32, to: u32, position: Position) -> Vec<Judged> {
	judge_range_with(name, from, to, position, thresholds())
}

fn judge_range_with(name: &str, from: u32, to: u32, position: Position, th: Thresholds) -> Vec<Judged> {
	let api = replay(name);
	let (best, _) = api.best_head().expect("best head");
	let protocol = accounts::fetch(&api, best).expect("protocol");
	let hashes: Vec<H256> = (from - 1..=to)
		.filter_map(|n| api.block_hash(n).ok().flatten())
		.collect();
	let mut out = Vec::new();
	for number in from..=to {
		let hash = api.block_hash(number).unwrap().expect("block hash");
		let parent = api.block_hash(number - 1).unwrap().expect("parent hash");
		let events = map_block(&api, hash).expect("events");
		let block = reduce::reduce(&events, &protocol);
		let xts = api.block_extrinsics(hash).expect("extrinsics");
		let permits = permit::permit_actors(&api, hash, &xts);
		for ext in block.per_ext.keys().copied().filter(|i| !is_hook_ext(*i)) {
			let Some(j) = judge_one(
				&api,
				&protocol,
				&events,
				&block,
				&xts,
				&permits,
				(parent, hash),
				&hashes,
				ext,
				position,
				th,
			) else {
				continue;
			};
			out.push(Judged {
				number,
				ext,
				judgement: j,
			});
		}
	}
	out
}

#[allow(clippy::too_many_arguments)]
fn judge_one(
	api: &RemoteApi,
	protocol: &cerebro_core::accounts::ProtocolAccounts,
	events: &[(Phase, ChainEvent)],
	block: &reduce::BlockData,
	xts: &[Vec<u8>],
	permits: &HashMap<u32, cerebro_core::AccountId>,
	(parent, hash): (H256, H256),
	recorded: &[H256],
	ext: u32,
	position: Position,
	th: Thresholds,
) -> Option<Judgement> {
	{
		let own = events
			.iter()
			.filter(|(p, _)| *p == Phase::ApplyExtrinsic(ext))
			.map(|(p, e)| (*p, e.clone(), None))
			.collect();
		let permit_actor = permits.get(&ext).copied();
		let cand = candidate_of(
			own,
			ext,
			protocol,
			&HashMap::new(),
			permit_actor,
			xcm::is_xcm_transfer(&xts[ext as usize]),
		);
		if !is_candidate(&cand) {
			return None;
		}
		let signer = cand.flows.signer?;
		if block
			.per_ext
			.iter()
			.any(|(i, f)| *i != ext && (f.signer == Some(signer) || f.deltas.contains_key(&signer)))
		{
			return None;
		}
		// `Last` keeps the candidate's own trades at their event position, as the monitor sees them;
		// `judge` appending them again changes no price.
		let first_own = events
			.iter()
			.position(|(p, _)| *p == Phase::ApplyExtrinsic(ext))
			.unwrap_or(events.len());
		let committed_events = match position {
			Position::First => &events[..0],
			Position::Actual => &events[..first_own],
			Position::Last => events,
		};
		let committed: Vec<Trade> = committed_events
			.iter()
			.filter_map(|(_, e)| match e {
				ChainEvent::Trade(t) => Some(t.clone()),
				_ => None,
			})
			.collect();
		let atoken_of = if events
			.iter()
			.any(|(_, e)| matches!(e, ChainEvent::AavePosition(d) if d.collateral))
		{
			balance::underlying_to_atoken(api, hash)
		} else {
			BTreeMap::new()
		};
		let t = AtBlock {
			api,
			parent,
			block: hash,
		};
		let ctx = |contracts| GuardCtx {
			protocol,
			contracts,
			atoken_of: &atoken_of,
			thresholds: th,
			committed_trades: &committed,
		};
		// The guard's contract map comes from its cache. Here, resolve the swept assets the way the
		// monitor did: at whichever block of the recording first needed them.
		let no_contracts = HashMap::new();
		let contracts: HashMap<_, _> = plan(&t, POST, &cand, &ctx(&no_contracts))
			.balances
			.iter()
			.filter_map(|(_, a)| {
				recorded
					.iter()
					.find_map(|h| balance::atoken_contract(api, *a, *h))
					.map(|c| (c, *a))
			})
			.collect();
		let cx = ctx(&contracts);
		let plan = plan(&t, POST, &cand, &cx);
		let pre = read_point(&t, PRE, &plan);
		let post = read_point(&t, POST, &plan);
		let oracle = Prices::fetch(api, hash, plan.oracle_assets.iter().copied());
		Some(judge(&cand, &plan, &pre, &post, &oracle, &cx, &Accumulators::default()))
	}
}

/// `(block, ext) → net` of every priced action the monitor's golden ledger recorded.
fn golden_nets(name: &str) -> BTreeMap<(u32, u32), i128> {
	let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}.summary.txt"));
	let golden = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
	golden
		.split("## actions\n")
		.nth(1)
		.and_then(|rest| rest.split("## ").next())
		.unwrap_or_default()
		.lines()
		.filter_map(|line| {
			let f: Vec<&str> = line.split('|').collect();
			if f.get(5) != Some(&"1") {
				return None;
			}
			Some(((f[1].parse().ok()?, f[2].parse().ok()?), f[3].parse().ok()?))
		})
		.collect()
}

/// No candidate of a false-positive fixture is held, and every one the monitor priced is judged
/// at the monitor's confirmed net.
fn assert_allowed_at_monitor_net(name: &str, from: u32, to: u32) {
	let judged = judge_range(name, from, to, Position::Last);
	let golden = golden_nets(name);
	let mut compared = 0;
	for j in &judged {
		assert_ne!(
			j.judgement.verdict,
			Verdict::Hold,
			"{name}: #{} ext {} held: {:?}",
			j.number,
			j.ext,
			j.judgement
		);
		if let Some(net) = golden.get(&(j.number, j.ext)) {
			assert_eq!(j.judgement.net, Usd6(*net), "{name}: #{} ext {}", j.number, j.ext);
			compared += 1;
		}
	}
	assert!(compared > 0, "{name}: no judged candidate has a monitor net to compare");
}

#[test]
fn judge_should_hold_when_arb_true_positive_fixture_is_applied() {
	let judged = judge_range("arb-true-positive", 13366416, 13366418, Position::Last);

	let tp = judged
		.iter()
		.find(|j| (j.number, j.ext) == (13366417, 2))
		.expect("the arb extrinsic is judged");
	assert_eq!(tp.judgement.verdict, Verdict::Hold);
	assert_eq!(tp.judgement.net, Usd6(50_714_638));
}

#[test]
fn judge_should_hold_at_every_position_when_extraction_fixture_is_moved() {
	// Fewer earlier trades leave the native fee leg unpriced, so the net moves with the position;
	// the verdict must not.
	for (position, net) in [
		(Position::First, Usd6(50_722_400)),
		(Position::Actual, Usd6(50_722_400)),
		(Position::Last, Usd6(50_714_638)),
	] {
		let judged = judge_range("arb-true-positive", 13366416, 13366418, position);

		let tp = judged
			.iter()
			.find(|j| (j.number, j.ext) == (13366417, 2))
			.expect("the arb extrinsic is judged");
		assert_eq!(tp.judgement.verdict, Verdict::Hold, "{position:?}");
		assert_eq!(tp.judgement.net, net, "{position:?}");
	}
}

#[test]
fn judge_should_allow_when_aave_borrow_fixture_is_applied() {
	assert_allowed_at_monitor_net("aave-borrow", 13172990, 13172996);
}

#[test]
fn judge_should_allow_when_aave_supply_fixture_is_applied() {
	assert_allowed_at_monitor_net("aave-supply", 13358735, 13358737);
}

#[test]
fn judge_should_allow_when_dca_sibling_fixture_is_applied() {
	assert_allowed_at_monitor_net("dca-sibling", 13364671, 13364673);
}

#[test]
fn judge_should_allow_when_flash_funded_repay_fixture_is_applied() {
	assert_allowed_at_monitor_net("flash-funded-repay", 13348492, 13348494);
}

#[test]
fn judge_should_allow_when_lm_reward_fixture_is_applied() {
	assert_allowed_at_monitor_net("lm-reward", 13342000, 13342011);
}

#[test]
fn judge_should_allow_when_lp_withdrawal_fixture_is_applied() {
	assert_allowed_at_monitor_net("lp-withdrawal", 13240732, 13240734);
}

#[test]
fn judge_should_allow_when_staking_reward_fixture_is_applied() {
	assert_allowed_at_monitor_net("staking-reward", 13215038, 13215044);
}

#[test]
fn judge_should_hold_nothing_when_node_thresholds_apply_to_fixtures() {
	let held: Vec<_> = [
		("arb-true-positive", 13366416, 13366418),
		("aave-borrow", 13172990, 13172996),
		("aave-supply", 13358735, 13358737),
		("dca-sibling", 13364671, 13364673),
		("flash-funded-repay", 13348492, 13348494),
		("lm-reward", 13342000, 13342011),
		("lp-withdrawal", 13240732, 13240734),
		("staking-reward", 13215038, 13215044),
	]
	.into_iter()
	.flat_map(|(name, from, to)| {
		judge_range_with(name, from, to, Position::Actual, node_thresholds())
			.into_iter()
			.filter(|j| j.judgement.verdict.blocks())
			.map(move |j| (name, j.number, j.ext, j.judgement.pools))
	})
	.collect();

	assert_eq!(held, vec![]);
}
