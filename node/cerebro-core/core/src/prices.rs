//! Per-block asset valuation from the block's OWN executed trades — the realized spot,
//! no oracle involved (the EMA oracle is stale/wrong for the bridged asset class).
//!
//! A trade `amount_in of A → amount_out of B` means the pool valued those equal, so
//! `price(B) = price(A) · amount_in/amount_out`. Anchor USDT at 1, relax over every trade
//! edge, and a fair swap nets exactly 0 (value in == value out); only value taken *beyond*
//! the executed rate nets positive.
//!
//! f64 mirrors the reference implementation exactly (including `.round()`) so parity holds
//! to the raw unit — do not "improve" to fixed-point without re-baselining the goldens.

use crate::model::Trade;
use crate::value::Usd6;
use crate::AssetId;
use std::collections::HashMap;

/// The numeraire: asset the whole tool is denominated in (mainnet USDT). Anchored at raw 1.
pub const USDT: AssetId = 10;

/// Raw-USDT price of one raw unit of a $1 asset: `1e6 / 10^dec`.
pub fn dollar_raw_price(dec: u32) -> f64 {
	1e6 / 10f64.powi(dec as i32)
}

pub struct ExecPrices {
	/// asset -> raw-USDT (6 dec) per one raw unit, from THIS block's trades only.
	price: HashMap<AssetId, f64>,
}

impl ExecPrices {
	/// Two anchor tiers, both raw-USDT per raw unit:
	/// - `confirmed`: oracle-≈$1 stables (USDT/HOLLAR/USDC), seeded BEFORE relaxation so the
	///   graph can price a premium stable off them (sUSDe ≈ 1.24·HOLLAR).
	/// - `par`: pool-vouched members with no oracle confirmation (wormhole stables), seeded only
	///   AFTER relaxation as a last resort for assets the executed rates couldn't reach.
	///
	/// The ordering is load-bearing: par-anchoring a yield-bearing stable at flat $1 before it can
	/// be priced from its own trade edge under-values its cost and invents phantom profit
	/// (the apyUSD / sUSDe false positives).
	pub fn build(trades: &[Trade], confirmed: &HashMap<AssetId, f64>, par: &HashMap<AssetId, f64>) -> Self {
		let mut price: HashMap<AssetId, f64> = HashMap::new();
		price.insert(USDT, 1.0);
		for (&asset, &p) in confirmed {
			price.insert(asset, p);
		}
		Self::relax(&mut price, trades);
		for (&asset, &p) in par {
			price.entry(asset).or_insert(p);
		}
		Self::relax(&mut price, trades);
		Self { price }
	}

	fn relax(price: &mut HashMap<AssetId, f64>, trades: &[Trade]) {
		loop {
			let mut changed = false;
			for t in trades {
				if t.amount_in == 0 || t.amount_out == 0 {
					continue;
				}
				let (amt_in, amt_out) = (t.amount_in as f64, t.amount_out as f64);
				match (price.get(&t.asset_in).copied(), price.get(&t.asset_out).copied()) {
					(Some(p_in), None) => {
						price.insert(t.asset_out, p_in * amt_in / amt_out);
						changed = true;
					}
					(None, Some(p_out)) => {
						price.insert(t.asset_in, p_out * amt_out / amt_in);
						changed = true;
					}
					_ => {}
				}
			}
			if !changed {
				break;
			}
		}
	}

	/// Value `delta` (raw units, signed) of `asset`. None if the asset isn't reachable from
	/// USDT via this block's trades — an unpriceable leg is never silently valued at zero.
	pub fn value(&self, asset: AssetId, delta: i128) -> Option<Usd6> {
		let p = self.price.get(&asset)?;
		Some(Usd6((delta as f64 * p).round() as i128))
	}

	/// Raw-USDT price of one raw unit of `asset` if reachable (diagnostics).
	pub fn price_of(&self, asset: AssetId) -> Option<f64> {
		self.price.get(&asset).copied()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn trade(asset_in: AssetId, amount_in: u128, asset_out: AssetId, amount_out: u128) -> Trade {
		Trade {
			asset_in,
			amount_in,
			asset_out,
			amount_out,
			router: true,
		}
	}

	// DOT = asset 5. A fair USDT->DOT swap: 86.18 USDT -> 101.85 DOT (block 13130910's rate).
	#[test]
	fn fair_trade_should_net_zero_when_valued_at_its_own_executed_rate() {
		let t = trade(USDT, 86_180_000, 5, 101_850_000_000);
		let px = ExecPrices::build(&[t], &HashMap::new(), &HashMap::new());
		// Value the exact legs of that trade: -in + out == 0 to the raw unit.
		let inn = px.value(USDT, -86_180_000).unwrap();
		let out = px.value(5, 101_850_000_000).unwrap();
		assert_eq!(inn.saturating_add(out), Usd6::ZERO);
	}

	#[test]
	fn unreachable_asset_should_be_unpriced_when_no_trade_path_to_usdt() {
		let t = trade(5, 100, 9999, 200); // 9999 only reachable if 5 is priced; 5 has no USDT path
		let px = ExecPrices::build(&[t], &HashMap::new(), &HashMap::new());
		assert_eq!(px.value(9999, 1), None);
		assert_eq!(px.value(5, 1), None);
	}

	// sUSDe (premium stable ~$1.24) shares a pool with HOLLAR and trades to it, but has NO
	// direct USDT edge. Confirmed-anchoring HOLLAR lets the graph price sUSDe at its real 1.24,
	// and the later par anchor must NOT flatten it to $1. (apyUSD/sUSDe FP class.)
	#[test]
	fn premium_stable_should_keep_executed_rate_when_par_anchor_would_flatten_it() {
		const HOLLAR: AssetId = 222;
		const SUSDE: AssetId = 1_000_625;
		// sUSDe -> HOLLAR at 1.2435 (1 sUSDe = 1.2435 HOLLAR); both 18-dec.
		let t = trade(SUSDE, 1_000_000_000_000_000_000, HOLLAR, 1_243_500_000_000_000_000);
		let confirmed = HashMap::from([(HOLLAR, 1e-12)]); // HOLLAR $1 at 18 dec => raw price 1e-12 * ...
													// Use realistic raw price: 1 raw HOLLAR (1e-18 whole) = 1e-18 * $1 in USDT-6 = 1e-12 raw-USDT.
		let par = HashMap::from([(SUSDE, 1e-12)]); // par would say $1 — must lose to the executed rate
		let px = ExecPrices::build(&[t], &confirmed, &par);
		let p_susde = px.price_of(SUSDE).unwrap();
		let p_hollar = px.price_of(HOLLAR).unwrap();
		// sUSDe priced off the trade at 1.2435x HOLLAR, NOT flattened to HOLLAR's $1 par.
		assert!(
			(p_susde / p_hollar - 1.2435).abs() < 1e-6,
			"sUSDe should be 1.2435x HOLLAR, got {}",
			p_susde / p_hollar
		);
	}

	#[test]
	fn par_anchor_should_apply_when_asset_has_no_trade_edge() {
		const USDT_MRL: AssetId = 23; // wormhole stable, no oracle feed, no in-block trade
		let par = HashMap::from([(USDT_MRL, 1.0)]);
		let px = ExecPrices::build(&[], &HashMap::new(), &par);
		assert_eq!(px.value(USDT_MRL, 1_000_000), Some(Usd6(1_000_000)));
	}
}
