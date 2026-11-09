//! Beneficiary-agnostic pool-leak detection: a beneficiary who splits an extraction across fresh
//! signers or across blocks defeats per-account detection, but the pool still bleeds. The
//! invariant-free fingerprint is a drop in the pool's **value-per-share** (vps): legit swaps
//! raise vps (fees), a proportional or single-asset add/remove keeps it flat-or-up (imbalance
//! fee), only over-mint extraction lowers it. So the metric is beneficiary- AND
//! block-count-agnostic.
//!
//! Detection is PRICE-INDEPENDENT — each venue's own invariant. Prices only scale the reported
//! `$` via `dilution = (1 − vps_cur/vps_prev) · usd_value_prev`. The functions here are pure over
//! snapshot values ([`crate::reader`] types); the chain side supplies the snapshots and the prices.

use crate::reader::{OmnipoolAssetSnapshot, StableswapSnapshot, XykSnapshot};
use crate::value::Usd6;
use hydra_dx_math::stableswap::types::AssetReserve;
use hydra_dx_math::stableswap::{calculate_d, MAX_D_ITERATIONS};

/// Pool-leak key of an omnipool asset; stableswap pools use their pool id, XYK pools their share token.
pub const OMNI_KEY_BASE: u32 = 0xF000_0000;

/// Per-block leaks under this magnitude (raw USDT) are fee/rounding noise.
pub const LEAK_RECORD_FLOOR: i128 = 1_000_000; // $1

/// The unexplained-leak assessment for one pool across one block (parent → current).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leak {
	/// Value diluted away from pre-existing shareholders (a vps drop). This is the mark-to-market
	/// loss to existing LPs, which is ≥ any single signer's realized gain.
	pub suspicious: Usd6,
	/// Value-per-share change in basis points (negative = pool poorer per share = leak).
	pub vps_bps: i64,
	/// Net share-issuance change over the block (positive = shares minted).
	pub issuance_delta: i128,
}

/// Combine parent/current value-per-share and the priced reserve value into a leak.
/// `dilution = max(0, 1 − vps_cur/vps_prev) · usd_value_prev`. None if parent vps is unusable.
pub fn finalize(vps_prev: f64, vps_cur: f64, iss_prev: u128, iss_cur: u128, usd_prev: Usd6) -> Option<Leak> {
	// vps_prev must be a usable positive denominator; the explicit form also rejects NaN/±inf
	// (which `<= 0.0` alone would let through).
	if !vps_prev.is_finite() || vps_prev <= 0.0 || !vps_cur.is_finite() {
		return None;
	}
	let ratio = vps_cur / vps_prev;
	let frac = (1.0 - ratio).max(0.0);
	Some(Leak {
		suspicious: Usd6((frac * usd_prev.0 as f64).round() as i128),
		vps_bps: ((ratio - 1.0) * 10_000.0).round() as i64,
		issuance_delta: iss_cur as i128 - iss_prev as i128,
	})
}

/// Stableswap value = D-invariant. D is computed for BOTH reserve snapshots at the CURRENT block's
/// amp+pegs so an amp-ramp / peg-drift re-measurement cancels and only reserve/issuance changes
/// score. `usd_prev` is the pre-block priced reserve value (supplied by the caller, "value what you
/// can"). Returns None if D fails to converge or issuance is zero.
pub fn stableswap_leak(prev: &StableswapSnapshot, cur: &StableswapSnapshot, usd_prev: Usd6) -> Option<Leak> {
	if prev.share_issuance == 0 || cur.share_issuance == 0 {
		return None;
	}
	let res_prev = reserves_at(prev, cur); // prev reserves, but cur decimals ordering guaranteed same
	let res_cur: Vec<AssetReserve> = cur
		.reserves
		.iter()
		.map(|r| AssetReserve::new(r.amount, r.decimals))
		.collect();
	let d_prev = calculate_d::<MAX_D_ITERATIONS>(&res_prev, cur.amplification, &cur.pegs)?;
	let d_cur = calculate_d::<MAX_D_ITERATIONS>(&res_cur, cur.amplification, &cur.pegs)?;
	finalize(
		d_prev as f64 / prev.share_issuance as f64,
		d_cur as f64 / cur.share_issuance as f64,
		prev.share_issuance,
		cur.share_issuance,
		usd_prev,
	)
}

/// The parent reserves valued with the CURRENT snapshot's decimals (assets are ordered identically
/// in both snapshots — same pool, same block window).
fn reserves_at(prev: &StableswapSnapshot, cur: &StableswapSnapshot) -> Vec<AssetReserve> {
	prev.reserves
		.iter()
		.zip(cur.reserves.iter())
		.map(|(p, c)| AssetReserve::new(p.amount, c.decimals))
		.collect()
}

/// XYK value = `sqrt(reserve_a · reserve_b)`, shares = `TotalLiquidity`.
pub fn xyk_leak(prev: &XykSnapshot, cur: &XykSnapshot, usd_prev: Usd6) -> Option<Leak> {
	if prev.share_issuance == 0 || cur.share_issuance == 0 {
		return None;
	}
	let inv_prev = (prev.reserves.0 as f64 * prev.reserves.1 as f64).sqrt();
	let inv_cur = (cur.reserves.0 as f64 * cur.reserves.1 as f64).sqrt();
	finalize(
		inv_prev / prev.share_issuance as f64,
		inv_cur / cur.share_issuance as f64,
		prev.share_issuance,
		cur.share_issuance,
		usd_prev,
	)
}

/// Omnipool per-asset value = `sqrt(reserve · hub_reserve)` (each asset↔LRNA leg is
/// constant-product), shares = the asset's LP shares.
pub fn omnipool_asset_leak(prev: &OmnipoolAssetSnapshot, cur: &OmnipoolAssetSnapshot, usd_prev: Usd6) -> Option<Leak> {
	if prev.shares == 0 || cur.shares == 0 {
		return None;
	}
	let inv_prev = (prev.reserve as f64 * prev.hub_reserve as f64).sqrt();
	let inv_cur = (cur.reserve as f64 * cur.hub_reserve as f64).sqrt();
	finalize(
		inv_prev / prev.shares as f64,
		inv_cur / cur.shares as f64,
		prev.shares,
		cur.shares,
		usd_prev,
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reader::AssetReserve as SnapReserve;

	fn stable(reserves: &[(u128, u8)], amp: u128, issuance: u128) -> StableswapSnapshot {
		StableswapSnapshot {
			assets: (0..reserves.len() as u32).collect(),
			reserves: reserves
				.iter()
				.map(|&(amount, decimals)| SnapReserve { amount, decimals })
				.collect(),
			amplification: amp,
			pegs: vec![(1, 1); reserves.len()],
			share_issuance: issuance,
		}
	}

	// A proportional/single-asset legit op keeps D/issuance flat-or-up → no dilution.
	#[test]
	fn stableswap_leak_should_be_zero_when_op_is_value_conserving() {
		// Balanced pool; add the same value proportionally → vps unchanged.
		let prev = stable(
			&[(1_000_000_000_000_000_000, 18), (1_000_000_000_000_000_000, 18)],
			100,
			2_000_000,
		);
		let cur = stable(
			&[(1_100_000_000_000_000_000, 18), (1_100_000_000_000_000_000, 18)],
			100,
			2_200_000,
		);
		let leak = stableswap_leak(&prev, &cur, Usd6::from_dollars(2.0)).unwrap();
		assert_eq!(
			leak.suspicious,
			Usd6::ZERO,
			"proportional add must not dilute, vps_bps={}",
			leak.vps_bps
		);
		assert!(leak.vps_bps >= 0);
	}

	// Over-mint: shares grow but reserves don't → vps drops → the full pre-block value is scored.
	#[test]
	fn stableswap_leak_should_fire_when_shares_minted_without_reserves() {
		let prev = stable(
			&[(1_000_000_000_000_000_000, 18), (1_000_000_000_000_000_000, 18)],
			100,
			2_000_000,
		);
		// issuance doubles, reserves unchanged → vps halves.
		let cur = stable(
			&[(1_000_000_000_000_000_000, 18), (1_000_000_000_000_000_000, 18)],
			100,
			4_000_000,
		);
		let leak = stableswap_leak(&prev, &cur, Usd6::from_dollars(2.0)).unwrap();
		assert_eq!(leak.vps_bps, -5000, "vps should halve (−5000 bps)");
		assert_eq!(leak.suspicious, Usd6(1_000_000), "half of $2 pre-value diluted");
	}

	#[test]
	fn xyk_leak_should_score_half_when_liquidity_doubles_without_reserves() {
		let prev = XykSnapshot {
			assets: (1, 2),
			reserves: (1_000, 1_000),
			share_token: 100,
			share_issuance: 1_000,
		};
		let cur = XykSnapshot {
			assets: (1, 2),
			reserves: (1_000, 1_000),
			share_token: 100,
			share_issuance: 2_000,
		};
		let leak = xyk_leak(&prev, &cur, Usd6::from_dollars(4.0)).unwrap();
		assert_eq!(leak.vps_bps, -5000);
		assert_eq!(leak.suspicious, Usd6(2_000_000));
	}

	#[test]
	fn omnipool_leak_should_score_half_when_shares_double_without_reserves() {
		let prev = OmnipoolAssetSnapshot {
			reserve: 1_000,
			hub_reserve: 1_000,
			shares: 1_000,
		};
		let cur = OmnipoolAssetSnapshot {
			reserve: 1_000,
			hub_reserve: 1_000,
			shares: 2_000,
		};
		let leak = omnipool_asset_leak(&prev, &cur, Usd6::from_dollars(10.0)).unwrap();
		assert_eq!(leak.vps_bps, -5000);
		assert_eq!(leak.suspicious, Usd6(5_000_000));
	}

	#[test]
	fn finalize_should_return_none_when_parent_vps_unusable() {
		assert!(finalize(0.0, 1.0, 1, 1, Usd6::ONE).is_none());
		assert!(finalize(f64::NAN, 1.0, 1, 1, Usd6::ONE).is_none());
	}
}
