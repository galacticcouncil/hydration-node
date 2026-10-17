//! Cost of the conditional-offer pass at queue sizes far above the per-account cap.
//! Intents are written to storage directly, so the cap does not apply.

use crate::tests::mock::*;
use crate::*;

fn seed_match_first(count: u128) {
	pallet_timestamp::Now::<Test>::put(0);
	set_oracle_price(Some(EmaPrice { n: 1, d: 1 }));
	for i in 0..count {
		let (asset_in, asset_out) = if i % 2 == 0 { (HDX, DOT) } else { (DOT, HDX) };
		Intents::<Test>::insert(
			i,
			Intent {
				data: IntentData::MatchFirstSwap {
					swap: SwapData {
						asset_in,
						asset_out,
						amount_in: 10_000,
						amount_out: 10_000,
						partial: Partial::No,
					},
					match_until: 3_000,
				},
				deadline: None,
				on_resolved: None,
			},
		);
	}
}

#[test]
#[ignore = "timing probe: cargo test -p pallet-intent conditional_scaling -- --ignored --nocapture"]
fn solver_intents_should_stay_within_block_time_when_queue_holds_10k_match_first_intents() {
	ExtBuilder::default().build().execute_with(|| {
		seed_match_first(10_000);

		let start = std::time::Instant::now();
		let (list, _) = IntentPallet::solver_intents();
		let elapsed = start.elapsed();

		println!("10k match-first intents: {} offered in {elapsed:?}", list.len());
		assert_eq!(list.len(), 10_000);
		assert!(elapsed < std::time::Duration::from_millis(1_000), "took {elapsed:?}");
	});
}

/// The pre-aggregation pass, kept verbatim as the reference the optimised one must match.
fn reference_apply(intents: &mut Vec<SolverEntry>, conditional: &BTreeMap<IntentId, ConditionalOffer>) {
	let mut withheld: BTreeSet<IntentId> = BTreeSet::new();
	let mut converged = false;
	for _ in 0..(2 * conditional.len() + 2) {
		let mut changed = false;
		for (&id, offer) in conditional {
			if withheld.contains(&id) {
				continue;
			}
			let Some(i) = intents.iter().position(|(other, _, _)| *other == id) else {
				continue;
			};
			let IntentData::Swap(w) = &intents[i].1.data else {
				continue;
			};
			let (amount_in, offered_now) = (w.amount_in, w.remaining());
			let coverage = reference_reverse_flow(id, w, offer.reverse_rate, intents, conditional, &withheld);
			match offer.kind {
				Conditional::WholeAmount => {
					if amount_in.saturating_sub(coverage) >= offer.ed_in {
						withheld.insert(id);
						changed = true;
					}
				}
				Conditional::CappedPartial => {
					let cap = coverage.min(offer.stored_remaining);
					if cap < offer.ed_in {
						withheld.insert(id);
						changed = true;
					} else if offered_now.saturating_sub(cap) >= offer.ed_in {
						if let IntentData::Swap(w) = &mut intents[i].1.data {
							w.partial = Partial::Yes(amount_in.saturating_sub(cap));
						}
						changed = true;
					}
				}
			}
		}
		if !changed {
			converged = true;
			break;
		}
	}
	if !converged {
		withheld.extend(conditional.keys().copied());
	}
	intents.retain(|(id, _, _)| !withheld.contains(id));
}

fn reference_reverse_flow(
	id: IntentId,
	w: &SwapData,
	rate: FixedU128,
	intents: &[SolverEntry],
	conditional: &BTreeMap<IntentId, ConditionalOffer>,
	withheld: &BTreeSet<IntentId>,
) -> Balance {
	let mut reverse: Balance = 0;
	let mut ahead: Balance = 0;
	for (other, intent, _) in intents {
		if *other == id || withheld.contains(other) {
			continue;
		}
		let IntentData::Swap(o) = &intent.data else { continue };
		if o.asset_in == w.asset_out && o.asset_out == w.asset_in {
			if rate.checked_mul_int(o.amount_in).is_some_and(|out| out >= o.amount_out) {
				reverse = reverse.saturating_add(rate.checked_mul_int(o.remaining()).unwrap_or(0));
			}
		} else if o.asset_in == w.asset_in && o.asset_out == w.asset_out {
			let queued_ahead = *other < id || !conditional.contains_key(other);
			if queued_ahead
				&& rate
					.checked_mul_int(o.amount_out)
					.is_some_and(|cost| cost <= o.amount_in)
			{
				ahead = ahead.saturating_add(o.remaining());
			}
		}
	}
	reverse.saturating_sub(ahead)
}

struct Lcg(u64);
impl Lcg {
	fn below(&mut self, n: u64) -> u64 {
		self.0 = self
			.0
			.wrapping_mul(6364136223846793005)
			.wrapping_add(1442695040888963407);
		(self.0 >> 33) % n
	}
}

fn random_queue(rng: &mut Lcg) -> (Vec<SolverEntry>, BTreeMap<IntentId, ConditionalOffer>) {
	const ASSETS: [AssetId; 3] = [HDX, DOT, ETH];
	const RATES: [(u128, u128); 3] = [(1, 1), (9, 10), (11, 10)];
	let pair_rate: Vec<usize> = (0..9).map(|_| rng.below(3) as usize).collect();
	let mut intents = Vec::new();
	let mut conditional = BTreeMap::new();
	for id in 0..(2 + rng.below(40)) as IntentId {
		let a = rng.below(3) as usize;
		let b = (a + 1 + rng.below(2) as usize) % 3;
		let amount_in = 1_000 + rng.below(50_000) as Balance;
		let amount_out = amount_in * (80 + rng.below(41) as Balance) / 100;
		let partial = match rng.below(3) {
			0 => Partial::No,
			1 => Partial::Yes(0),
			_ => Partial::Yes(rng.below(amount_in as u64) as Balance),
		};
		let swap = SwapData {
			asset_in: ASSETS[a],
			asset_out: ASSETS[b],
			amount_in,
			amount_out,
			partial,
		};
		if rng.below(3) != 0 {
			// Mostly one rate per pair, sometimes a second, to cover several rates on one pair.
			let (n, d) = RATES[if rng.below(5) == 0 {
				rng.below(3) as usize
			} else {
				pair_rate[a * 3 + b]
			}];
			let kind = if swap.partial.is_partial() && rng.below(2) == 0 {
				Conditional::CappedPartial
			} else {
				Conditional::WholeAmount
			};
			conditional.insert(
				id,
				ConditionalOffer {
					kind,
					stored_remaining: swap.remaining(),
					reverse_rate: FixedU128::from_rational(n, d),
					ed_in: [1, 1_000][rng.below(2) as usize],
				},
			);
		}
		let intent = Intent {
			data: IntentData::Swap(swap),
			deadline: None,
			on_resolved: None,
		};
		intents.push((id, intent, None));
	}
	(intents, conditional)
}

#[test]
fn apply_conditional_offers_should_match_reference_pass_when_queues_are_random() {
	let mut rng = Lcg(7);
	let (mut withheld, mut capped) = (0, 0);
	for _ in 0..5_000 {
		let (queue, conditional) = random_queue(&mut rng);
		let mut expected = queue.clone();
		reference_apply(&mut expected, &conditional);
		let mut actual = queue.clone();
		IntentPallet::apply_conditional_offers(&mut actual, &conditional);
		assert_eq!(actual, expected);

		withheld += queue.len() - expected.len();
		capped += expected.iter().filter(|e| !queue.contains(e)).count();
	}
	// Guards against a generator that never exercises the interesting branches.
	assert_eq!(
		(withheld > 10_000, capped > 1_000),
		(true, true),
		"withheld {withheld}, capped {capped}"
	);
}
