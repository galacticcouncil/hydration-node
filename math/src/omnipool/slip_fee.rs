use crate::omnipool::types::SignedBalance;
use crate::types::Balance;
use crate::MathError::Overflow;
use crate::{to_balance, to_u256};
use num_traits::One;
use primitive_types::U256;
use sp_arithmetic::Permill;

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

	// Check if rate exceeds max_slip_fee: |cum| * 1_000_000 / denom > max_parts?
	// Safe: u128 * 1_000_000 fits in U256
	let rate_millionths = U256::from(abs_cumulative)
		.saturating_mul(U256::from(1_000_000u64))
		.checked_div(U256::from(denom))?;
	let max_parts = max_slip_fee.deconstruct() as u64;

	if rate_millionths > U256::from(max_parts) {
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

/// Invert buy-side slip fee: given D_net (hub asset actually entering buy pool),
/// find D_gross (hub asset before buy-side slip deduction).
///
/// Forward formula: `D_net = D_gross - |cum| * D_gross / (L + cum)` where `cum = C + D_gross`.
///
/// - `d_net` — hub asset entering the buy pool after slip deduction
/// - `l` — hub reserve at block start (Q₀)
/// - `c` — cumulative signed hub asset delta before this trade
///
/// Two cases based on the sign of cumulative after the trade:
/// - Case 1 (cum >= 0): `D_gross = D_net * (L+C) / (L - D_net)` (linear)
/// - Case 2 (C < 0, D_gross < |C|): quadratic inversion
pub(crate) fn invert_buy_side_slip(d_net: Balance, l: Balance, c: SignedBalance) -> Option<Balance> {
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

	if !c.is_negative() || d_gross_case1 >= c.abs() {
		// Case 1 valid: cum = C + D_gross >= 0
		return Some(d_gross_case1);
	}

	// Case 2: C < 0, D_gross < |C|, cum stays negative
	// 2*D_gross² + (L + 2C - D_net)*D_gross - D_net*S_buy = 0
	let abs_c = c.abs();
	let two_c = abs_c.checked_mul(2)?;

	// b = L + 2C - D_net. Since C < 0: b = L - 2|C| - D_net
	let l_minus_2c = if l >= two_c { l - two_c } else { return None };
	let b_positive = l_minus_2c >= d_net;
	let b_abs = if b_positive {
		l_minus_2c - d_net
	} else {
		d_net - l_minus_2c
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
/// given D_gross (hub asset after sell-side deductions).
///
/// Forward formula: `D_gross = u*(1-pf) - slip_fee(u)` where `u = delta_hub_reserve_in`,
/// `pf = protocol_fee`, and `slip_fee = |C - u| * u / (L + C - u)`.
///
/// - `d_gross` — hub asset remaining after protocol fee and sell-side slip
/// - `protocol_fee` — protocol fee rate
/// - `l` — hub reserve at block start (Q₀) for the sell pool
/// - `c` — cumulative signed hub asset delta before this trade (for the sell pool)
///
/// Two cases based on the sign of cumulative = C - u (hub asset outflow is negative):
/// - Case A (u > C, cumulative < 0): `(k+1)*u² - (kS + C + D)*u + DS = 0`
/// - Case B (u <= C, C > 0, opposing flow): `pf*u² + (D + kS - C)*u - DS = 0`
pub(crate) fn invert_sell_side_fees(
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

	// Case A solver: (k+1)u² - (kS + C + D)u + DS = 0
	let solve_case_a = || -> Option<Balance> {
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
	};

	if c_is_positive {
		// C > 0 (opposing flow): try Case B first (assuming u <= C)
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

		match u_b {
			Some(u) if u <= abs_c => Some(u),
			_ => solve_case_a(), // u > C or Case B failed: use Case A
		}
	} else {
		// C <= 0: always Case A (u > 0 > C, so u > C)
		solve_case_a()
	}
}
