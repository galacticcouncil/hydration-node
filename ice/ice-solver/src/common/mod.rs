//! Common utilities shared between solver versions.

pub mod flow_graph;
pub mod ring_detection;

use hydra_dx_math::types::Ratio;
use ice_support::{AssetId, Balance, Intent, IntentData};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;

#[derive(Default, Debug, Clone)]
pub struct AssetFlow {
	pub total_in: Balance,
	pub total_out: Balance,
}

/// out = amount_in * (price_in / price_out)
///     = amount_in * price_in.n * price_out.d / (price_in.d * price_out.n)
///
/// Overflow-safe: handles large Ratio values (128-bit n/d) from real AMM spot prices.
/// Tries multiple computation orders to avoid U256 overflow while preserving precision.
pub fn calc_amount_out(amount_in: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
	let pi_n = U256::from(price_in.n);
	let pi_d = U256::from(price_in.d);
	let po_n = U256::from(price_out.n);
	let po_d = U256::from(price_out.d);
	let amt = U256::from(amount_in);

	// Strategy 1: direct — amount_in * (pi.n * po.d) / (pi.d * po.n)
	if let (Some(n), Some(d)) = (pi_n.checked_mul(po_d), pi_d.checked_mul(po_n)) {
		if let Some(result) = amt.checked_mul(n) {
			return result.checked_div(d)?.try_into().ok();
		}
		// amount_in * n overflows — split n/d (only useful when n >= d)
		if n >= d {
			let q = n.checked_div(d)?;
			let r = n.checked_rem(d)?;
			let base = amt.checked_mul(q)?;
			let correction = amt
				.checked_mul(r)
				.and_then(|v| v.checked_div(d))
				.unwrap_or(U256::zero());
			return base.checked_add(correction)?.try_into().ok();
		}
		// n < d: ratio < 1, split loses all precision — fall through to strategy 2/3
	}

	// Strategy 2: cross-cancel — (amount_in * pi.n / po.n) * (po.d / pi.d)
	// This works when pi.n and po.n are similar magnitude (both large) so their ratio is small.
	if let Some(ratio_n) = amt.checked_mul(pi_n) {
		let step1 = ratio_n.checked_div(po_n)?;
		if let Some(v) = step1.checked_mul(po_d) {
			return v.checked_div(pi_d)?.try_into().ok();
		}
	}

	// Strategy 3: (amount_in / pi.d) * pi.n then * po.d / po.n
	// Divide early to keep values small.
	let step1 = mul_div(amt, pi_n, pi_d)?;
	let result = mul_div(step1, po_d, po_n)?;
	result.try_into().ok()
}

/// Compute a * b / c with overflow protection.
pub fn mul_div(a: U256, b: U256, c: U256) -> Option<U256> {
	if c.is_zero() {
		return None;
	}
	if let Some(v) = a.checked_mul(b) {
		return v.checked_div(c);
	}
	// a * b overflows — use: (a / c) * b + (a % c) * b / c
	let q = a.checked_div(c)?;
	let r = a.checked_rem(c)?;
	let base = q.checked_mul(b)?;
	let correction = r.checked_mul(b).and_then(|v| v.checked_div(c)).unwrap_or(U256::zero());
	base.checked_add(correction)
}

/// in = amount_out * (price_out / price_in)
#[allow(dead_code)]
pub fn calc_amount_in(amount_out: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
	let n = U256::from(price_out.n) * U256::from(price_in.d);
	let d = U256::from(price_out.d) * U256::from(price_in.n);
	let result = U256::from(amount_out).checked_mul(n)?.checked_div(d)?;
	result.try_into().ok()
}

pub fn collect_unique_assets(intents: &[Intent]) -> BTreeSet<AssetId> {
	let mut assets: BTreeSet<AssetId> = BTreeSet::new();
	for intent in intents {
		match &intent.data {
			IntentData::Swap(swap) => {
				assets.insert(swap.asset_in);
				assets.insert(swap.asset_out);
			}
		}
	}
	assets
}

pub fn is_satisfiable(intent: &Intent, spot_prices: &BTreeMap<AssetId, Ratio>) -> bool {
	match &intent.data {
		IntentData::Swap(swap) => {
			let Some(price_in) = spot_prices.get(&swap.asset_in) else {
				log::trace!(target: "solver", "intent {}: not satisfiable — no spot price for asset_in {}", intent.id, swap.asset_in);
				return false;
			};
			let Some(price_out) = spot_prices.get(&swap.asset_out) else {
				log::trace!(target: "solver", "intent {}: not satisfiable — no spot price for asset_out {}", intent.id, swap.asset_out);
				return false;
			};

			let Some(calculated_out) = calc_amount_out(swap.amount_in, price_in, price_out) else {
				log::trace!(target: "solver", "intent {}: not satisfiable — calc_amount_out overflow for {} → {}", intent.id, swap.asset_in, swap.asset_out);
				return false;
			};
			if calculated_out < swap.amount_out {
				log::trace!(target: "solver", "intent {}: not satisfiable — spot output {} < min_out {} for {} → {}",
					intent.id, calculated_out, swap.amount_out, swap.asset_in, swap.asset_out);
				return false;
			}
			log::trace!(target: "solver", "intent {}: satisfiable — spot output {} >= min_out {} for {} → {}",
				intent.id, calculated_out, swap.amount_out, swap.asset_in, swap.asset_out);
			true
		}
	}
}

pub fn calculate_flows(intents: &[&Intent], spot_prices: &BTreeMap<AssetId, Ratio>) -> BTreeMap<AssetId, AssetFlow> {
	let mut flows: BTreeMap<AssetId, AssetFlow> = BTreeMap::new();

	for intent in intents {
		match &intent.data {
			IntentData::Swap(swap) => {
				if let (Some(price_in), Some(price_out)) =
					(spot_prices.get(&swap.asset_in), spot_prices.get(&swap.asset_out))
				{
					flows.entry(swap.asset_in).or_default().total_in += swap.amount_in;
					if let Some(amount_out) = calc_amount_out(swap.amount_in, price_in, price_out) {
						flows.entry(swap.asset_out).or_default().total_out += amount_out;
					}
				}
			}
		}
	}

	flows
}
