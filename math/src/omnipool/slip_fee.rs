use crate::omnipool::types::SignedBalance;
use crate::types::Balance;
use crate::MathError::Overflow;
use crate::{to_balance, to_u256};
use num_traits::One;
use primitive_types::U256;
use sp_arithmetic::Permill;

/// Permill scale (10^6).
const PERMILL_SCALE: u128 = 1_000_000;

/// Calculate the slip fee *amount* at full U256 precision, avoiding Permill truncation.
///
/// Computes: `min(|cumulative| / (Q₀ + cumulative), max_slip_fee) × base_amount`
///
/// When the rate is below `max_slip_fee`, the amount is computed as:
///   `|cumulative| × base_amount / (Q₀ + cumulative)` — full precision, only ±1 from integer division.
///
/// When the rate hits the cap, falls back to `max_slip_fee.mul_floor(base_amount)`.
pub fn calculate_slip_fee_amount(
	hub_reserve_at_block_start: Balance,
	prior_delta: SignedBalance,
	delta_q: SignedBalance,
	max_slip_fee: Permill,
	base_amount: Balance,
) -> Option<Balance> {
	if hub_reserve_at_block_start == 0 || base_amount == 0 {
		return Some(0);
	}

	let cumulative = prior_delta.checked_add(delta_q)?;

	let denom = cumulative.add_to_unsigned(hub_reserve_at_block_start)?;
	if denom == 0 {
		return None;
	}

	if cumulative.is_zero() {
		return Some(0);
	}

	let abs_cumulative = cumulative.abs();

	if slip_cap_fires_for(abs_cumulative, denom, max_slip_fee)? {
		// Capped: use Permill for the capped amount
		Some(max_slip_fee.mul_floor(base_amount))
	} else {
		// Full precision: |cumulative| * base_amount / denom
		let amount_hp = U256::from(abs_cumulative)
			.checked_mul(U256::from(base_amount))?
			.checked_div(U256::from(denom))?;
		to_balance!(amount_hp).ok()
	}
}

/// Single source of truth for the slip-fee cap predicate.
///
/// Mirrors the check inside `calculate_slip_fee_amount`: returns `true` iff
/// `|cumulative| * 10^6 > max_parts * denom`, the same threshold used to switch
/// the forward path from the proportional formula to the linear-capped formula.
///
/// Returns `Some(false)` for the degenerate cases (zero denom, zero cumulative)
/// so callers can fall through to the uncapped branch without special handling.
fn slip_cap_fires_for(abs_cumulative: Balance, denom: Balance, max_slip_fee: Permill) -> Option<bool> {
	if denom == 0 || abs_cumulative == 0 {
		return Some(false);
	}
	let max_parts = max_slip_fee.deconstruct() as u128;
	let lhs = U256::from(abs_cumulative).checked_mul(U256::from(PERMILL_SCALE))?;
	let rhs = U256::from(max_parts).checked_mul(U256::from(denom))?;
	Some(lhs > rhs)
}

/// Returns `true` iff the forward slip-fee path would cap when called with
/// `(hub_reserve_at_block_start, prior_delta, delta_q)`.
fn slip_cap_fires(
	hub_reserve_at_block_start: Balance,
	prior_delta: SignedBalance,
	delta_q: SignedBalance,
	max_slip_fee: Permill,
) -> Option<bool> {
	let cumulative = prior_delta.checked_add(delta_q)?;
	let denom = cumulative.add_to_unsigned(hub_reserve_at_block_start)?;
	slip_cap_fires_for(cumulative.abs(), denom, max_slip_fee)
}

/// Invert buy-side slip fee: given `D_net` (hub asset actually entering buy pool),
/// find `D_gross` (hub asset before buy-side slip deduction).
///
/// Forward formula: `D_net = D_gross - slip(D_gross)`, where
/// `slip = min(|cum|/(L+cum), max_slip_fee) * D_gross` and `cum = C + D_gross`.
///
/// - `d_net` — hub asset entering the buy pool after slip deduction
/// - `l` — hub reserve at block start (Q₀)
/// - `c` — cumulative signed hub asset delta before this trade
/// - `max_slip_fee` — slip-fee cap; when the cap fires at the resulting `D_gross`,
///   the inverse switches to the linear closed form.
///
/// Uncapped regime — two cases by sign of cumulative after the trade:
/// - Case 1 (`cum >= 0`): `D_gross = D_net * (L+C) / (L - D_net)` (linear)
/// - Case 2 (`C < 0, D_gross < |C|`): quadratic inversion
///
/// Capped regime — linear closed form: `D_gross = floor(D_net * 10^6 / (10^6 - max_parts)) + 1`.
/// The `+ 1` matches the ceiling discipline of the uncapped path so that
/// `forward(invert(d_net)) >= d_net` always holds.
pub(crate) fn invert_buy_side_slip(
	d_net: Balance,
	l: Balance,
	c: SignedBalance,
	max_slip_fee: Permill,
) -> Option<Balance> {
	// Try the uncapped (quadratic / linear) inverse first. It can fail to find
	// a real root when the trade is so large that no uncapped d_gross exists —
	// in that regime the cap MUST be binding, so we fall through to the linear
	// capped formula below.
	if let Some(d_gross_uncapped) = invert_buy_side_slip_uncapped(d_net, l, c) {
		// Same predicate as `calculate_slip_fee_amount`: if forward wouldn't cap
		// at this d_gross, the uncapped result is what the forward path uses.
		if !slip_cap_fires(l, c, SignedBalance::Positive(d_gross_uncapped), max_slip_fee)? {
			return Some(d_gross_uncapped);
		}
	}

	// Capped regime — forward slip is `max.mul_floor(D_gross)`, so
	//   D_net = D_gross - floor(max_parts * D_gross / 10^6)
	// The closed form `D_gross = floor(D_net * 10^6 / (10^6 - max_parts))` is
	// exact: it can be shown that `forward(D_gross) == D_net` for the floor result,
	// so no `+1` ceiling adjustment is needed (unlike the uncapped path which uses
	// quadratic/linear formulas that lose precision via integer division).
	let max_parts = max_slip_fee.deconstruct() as u128;
	let one_minus_max = PERMILL_SCALE.checked_sub(max_parts)?;
	if one_minus_max == 0 {
		return None;
	}
	let d_gross_hp = U256::from(d_net)
		.checked_mul(U256::from(PERMILL_SCALE))?
		.checked_div(U256::from(one_minus_max))?;
	to_balance!(d_gross_hp).ok()
}

/// Uncapped buy-side inverse (closed-form linear + quadratic fallback).
///
/// Kept private — production callers go through [`invert_buy_side_slip`].
fn invert_buy_side_slip_uncapped(d_net: Balance, l: Balance, c: SignedBalance) -> Option<Balance> {
	let s_buy = c.add_to_unsigned(l)?; // L + C
	if s_buy == 0 {
		return None;
	}

	if d_net >= l {
		return None;
	}

	// Try Case 1 (linear): D_gross = D_net * S_buy / (L - D_net)
	let l_minus_d = l.checked_sub(d_net)?;
	let (d_net_hp, s_buy_hp, l_minus_d_hp) = to_u256!(d_net, s_buy, l_minus_d);
	let d_gross_case1_hp = d_net_hp
		.checked_mul(s_buy_hp)?
		.checked_div(l_minus_d_hp)?
		.checked_add(U256::one())?;
	let d_gross_case1 = to_balance!(d_gross_case1_hp).ok()?;

	let abs_c = c.abs();
	if c.is_positive() || d_gross_case1 >= abs_c {
		// Case 1 valid: cum = C + D_gross >= 0
		return Some(d_gross_case1);
	}

	// Case 2: C < 0, D_gross < |C|, cum stays negative
	// 2*D_gross² + (L + 2C - D_net)*D_gross - D_net*S_buy = 0
	let two_c = abs_c.checked_mul(2)?;

	// b = L + 2C - D_net. Since C < 0: b = L - 2|C| - D_net
	let l_minus_2c = l.checked_sub(two_c)?;
	let b_positive = l_minus_2c >= d_net;
	let b_abs = if b_positive {
		l_minus_2c.checked_sub(d_net)?
	} else {
		d_net.checked_sub(l_minus_2c)?
	};

	// disc = b² + 8*D_net*S_buy
	let b_abs_hp = U256::from(b_abs);
	let b_sq = b_abs_hp.checked_mul(b_abs_hp)?;
	let eight_ds = U256::from(8u64).checked_mul(d_net_hp)?.checked_mul(s_buy_hp)?;
	// Safe: both terms are products of small factors, sum fits in U256
	let disc = b_sq.saturating_add(eight_ds);
	let sqrt_disc = disc.integer_sqrt();

	// D_gross = (-b + sqrt(disc)) / 4
	// When b > 0: use numerically stable form D_gross = 2*D_net*S_buy / (b + sqrt(disc))
	let d_gross_hp = if b_positive {
		// Safe: both are <= disc, sum fits in U256
		let denom = b_abs_hp.saturating_add(sqrt_disc);
		U256::from(2u64)
			.checked_mul(d_net_hp)?
			.checked_mul(s_buy_hp)?
			.checked_div(denom)?
	} else {
		// Safe: both are <= disc, sum fits in U256
		(b_abs_hp.saturating_add(sqrt_disc)).checked_div(U256::from(4u64))?
	};
	let d_gross = to_balance!(d_gross_hp).ok()?;
	d_gross.checked_add(Balance::one())
}

/// Invert sell-side fees (protocol_fee + sell-side slip) to find `delta_hub_reserve_in`
/// given `D_gross` (hub asset after sell-side deductions).
///
/// Forward formula: `D_gross = u*(1 - pf) - slip(u)`, where `u = delta_hub_reserve_in`,
/// `pf = protocol_fee`, and `slip(u) = min(|C-u|/(L+C-u), max_slip_fee) * u`.
///
/// - `d_gross` — hub asset remaining after protocol fee and sell-side slip
/// - `protocol_fee` — protocol fee rate
/// - `l` — hub reserve at block start (Q₀) for the sell pool
/// - `c` — cumulative signed hub asset delta before this trade (for the sell pool)
/// - `max_slip_fee` — slip-fee cap; when the cap fires at the resulting `u`,
///   the inverse switches to the linear closed form.
///
/// Uncapped regime — two cases by sign of `cum = C - u`:
/// - Case A (`u > C`, cum < 0): `(k+1)*u² - (kS + C + D)*u + DS = 0`
/// - Case B (`u <= C`, `C > 0`, opposing flow): `pf*u² + (D + kS - C)*u - DS = 0`
///
/// Capped regime — linear closed form: `u = floor(D_gross * 10^6 / (10^6 - pf_parts - max_parts)) + 1`.
pub(crate) fn invert_sell_side_fees(
	d_gross: Balance,
	protocol_fee: Permill,
	l: Balance,
	c: SignedBalance,
	max_slip_fee: Permill,
) -> Option<Balance> {
	// Try the uncapped quadratic first; it may fail (no real root) when the
	// trade is too large for the uncapped regime — in which case the cap is
	// necessarily binding and we fall through to the linear capped formula.
	if let Some(u_uncapped) = invert_sell_side_fees_uncapped(d_gross, protocol_fee, l, c) {
		// For sell side the cumulative after the trade is `C - u` (delta_q = -u).
		if !slip_cap_fires(l, c, SignedBalance::Negative(u_uncapped), max_slip_fee)? {
			return Some(u_uncapped);
		}
	}

	// Capped regime — forward: D_gross = u - floor(pf*u/10^6) - floor(max*u/10^6).
	// Continuous approximation: D_gross = u * (10^6 - pf_parts - max_parts) / 10^6.
	// The closed form `u = floor(D_gross * 10^6 / (10^6 - pf_parts - max_parts))`
	// satisfies `forward(u) >= D_gross` (overshoot of 0 or 1 unit, depending on
	// modular alignment of pf*u and max*u). The forward path is then re-evaluated
	// downstream against the actual `u`, so the slight overshoot propagates
	// pool-favorably without breaking the round-trip invariant.
	let pf_parts = protocol_fee.deconstruct() as u128;
	let max_parts = max_slip_fee.deconstruct() as u128;
	let denom_parts = PERMILL_SCALE.checked_sub(pf_parts)?.checked_sub(max_parts)?;
	if denom_parts == 0 {
		return None;
	}
	let u_hp = U256::from(d_gross)
		.checked_mul(U256::from(PERMILL_SCALE))?
		.checked_div(U256::from(denom_parts))?;
	to_balance!(u_hp).ok()
}

/// Uncapped sell-side inverse (quadratic with two cases).
///
/// Kept private — production callers go through [`invert_sell_side_fees`].
fn invert_sell_side_fees_uncapped(
	d_gross: Balance,
	protocol_fee: Permill,
	l: Balance,
	c: SignedBalance,
) -> Option<Balance> {
	let abs_c = c.abs();
	let c_is_positive = c.is_positive();

	// S = L + C (must be > 0)
	let s: u128 = if c_is_positive {
		l.checked_add(abs_c)?
	} else {
		l.checked_sub(abs_c)?
	};

	let pf_parts = protocol_fee.deconstruct() as u128;
	let k_parts = 1_000_000u128 - pf_parts;
	let scale = U256::from(1_000_000u64);

	// Try Case B first when C > 0 (opposing flow, u <= C)
	if c_is_positive {
		// pf*u² + (D + kS - C)*u - DS = 0
		let u_b = if pf_parts == 0 {
			// Linear: (D + kS - C)u = DS. With k=1: (D + S - C)u = DS → (D + L)u = DS
			// Safe: u128 * u128 fits in U256
			let num = U256::from(d_gross).saturating_mul(U256::from(s));
			let denom = U256::from(d_gross).saturating_add(U256::from(l));
			let u_hp = num.checked_div(denom)?;
			let u = to_balance!(u_hp).ok()?;
			u.checked_add(Balance::one())
		} else {
			// Quadratic: pf_parts*u² + (kS + (D-C)*10^6)*u - DS*10^6 = 0
			let a_u256 = U256::from(pf_parts);
			// Safe: u128 * u128 fits in U256
			let ks = U256::from(k_parts).saturating_mul(U256::from(s));
			let d_scaled = U256::from(d_gross).saturating_mul(scale);
			let c_scaled = U256::from(abs_c).saturating_mul(scale);

			// b = kS + D*10^6 - C*10^6 (can be negative)
			// Safe: all terms are products of u128 values, sum fits in U256
			let b_sum = ks.saturating_add(d_scaled);
			let b_positive = b_sum >= c_scaled;
			// Safe: guarded by comparison above
			let b_abs = if b_positive {
				b_sum.saturating_sub(c_scaled)
			} else {
				c_scaled.saturating_sub(b_sum)
			};

			// disc = b² + 4*a*D*S*10^6 (always positive since last term > 0)
			let b_sq = b_abs.checked_mul(b_abs)?;
			let four_a_ds = U256::from(4u64)
				.checked_mul(a_u256)?
				.checked_mul(U256::from(d_gross))?
				.checked_mul(U256::from(s))?
				.checked_mul(scale)?;
			// Safe: both terms are products of bounded factors, sum fits in U256
			let disc = b_sq.saturating_add(four_a_ds);
			let sqrt_disc = disc.integer_sqrt();
			// Safe: small constant * u128 fits in U256
			let two_a = a_u256.saturating_mul(U256::from(2u64));

			// Solve for positive root of a*u² + b*u - c = 0
			// When b > 0: use u = 2c / (b + sqrt(disc)) to avoid catastrophic cancellation
			// When b <= 0: use u = (-b + sqrt(disc)) / (2a)
			// Safe: u128 * u128 * 10^6 fits in U256
			let c_u256 = U256::from(d_gross).saturating_mul(U256::from(s)).saturating_mul(scale);
			let u_hp = if b_positive {
				// Safe: both are <= disc, sum fits in U256
				let denom = b_abs.saturating_add(sqrt_disc);
				U256::from(2u64).checked_mul(c_u256)?.checked_div(denom)?
			} else {
				// Safe: both are <= disc, sum fits in U256
				let numerator = b_abs.saturating_add(sqrt_disc);
				numerator.checked_div(two_a)?
			};
			let u = to_balance!(u_hp).ok()?;
			u.checked_add(Balance::one())
		};

		if let Some(u) = u_b {
			if u <= abs_c {
				return Some(u);
			}
		}
		// u > C or Case B failed: fall through to Case A
	}

	// Case A: (k+1)u² - (kS + C + D)u + DS = 0
	// Applies when C <= 0 (always) or when C > 0 but Case B yielded u > C.
	let a_u256 = U256::from(k_parts + 1_000_000);
	// Safe: u128 * u128 fits in U256
	let ks = U256::from(k_parts).saturating_mul(U256::from(s));
	let d_scaled = U256::from(d_gross).saturating_mul(scale);
	let c_scaled = U256::from(abs_c).saturating_mul(scale);
	// b = kS + (C + D)*10^6  (treating C with its sign)
	// Safe: all terms are products of u128 values, sum fits in U256
	let b_u256 = if c_is_positive {
		ks.saturating_add(d_scaled).saturating_add(c_scaled)
	} else {
		let sum = ks.saturating_add(d_scaled);
		sum.checked_sub(c_scaled)?
	};
	// Safe: u128 * u128 * 10^6 fits in U256
	let c_u256 = U256::from(d_gross).saturating_mul(U256::from(s)).saturating_mul(scale);
	let b_sq = b_u256.checked_mul(b_u256)?;
	let four_ac = U256::from(4u64).checked_mul(a_u256)?.checked_mul(c_u256)?;
	if b_sq < four_ac {
		return None;
	}
	// Safe: guarded by b_sq >= four_ac check above
	let disc = b_sq.saturating_sub(four_ac);
	let sqrt_disc = disc.integer_sqrt();
	// Safe: small constant * u128 fits in U256
	let two_a = a_u256.saturating_mul(U256::from(2u64));
	if b_u256 < sqrt_disc {
		return None;
	}
	// Safe: guarded by b_u256 >= sqrt_disc check above
	let u_hp = (b_u256.saturating_sub(sqrt_disc)).checked_div(two_a)?;
	let u = to_balance!(u_hp).ok()?;
	u.checked_add(Balance::one())
}
