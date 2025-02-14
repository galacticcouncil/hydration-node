use crate::stableswap::types::AssetReserve;

use crate::support::rational::round_to_rational;
use crate::to_u256;
use crate::types::{AssetId, Balance, Ratio};
use num_traits::{CheckedDiv, CheckedMul, CheckedSub, One, SaturatingMul, Zero};
use primitive_types::U256;
use sp_arithmetic::helpers_128bit::multiply_by_rational_with_rounding;
use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};
use sp_std::ops::Div;
use sp_std::prelude::*;
use sp_std::vec;

pub const MAX_Y_ITERATIONS: u8 = 128;
pub const MAX_D_ITERATIONS: u8 = 64;

// Precision to convert reserves and amounts to.
const TARGET_PRECISION: u8 = 18;

// Convergence precision used in Newton's method.
const PRECISION: u8 = 1;

/// Calculate amount to be received from the pool given the amount to be sent to the pool.
/// D - number of iterations to use for Newton's formula to calculate parameter D ( it should be >=1 otherwise it wont converge at all and will always fail
/// Y - number of iterations to use for Newton's formula to calculate reserve Y ( it should be >=1 otherwise it wont converge at all and will always fail
pub fn calculate_out_given_in<const D: u8, const Y: u8>(
	initial_reserves: &[AssetReserve],
	idx_in: usize,
	idx_out: usize,
	amount_in: Balance,
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	if idx_in == idx_out {
		return None;
	}
	if idx_in >= initial_reserves.len() || idx_out >= initial_reserves.len() {
		return None;
	}
	let reserves = normalize_reserves(initial_reserves);
	let amount_in = normalize_value(
		amount_in,
		initial_reserves[idx_in].decimals,
		TARGET_PRECISION,
		Rounding::Up,
	);
	let new_reserve_out = calculate_y_given_in::<D, Y>(amount_in, idx_in, idx_out, &reserves, amplification, pegs)?;

	let amount_out = reserves[idx_out].checked_sub(new_reserve_out)?;
	let amount_out = normalize_value(
		amount_out,
		TARGET_PRECISION,
		initial_reserves[idx_out].decimals,
		Rounding::Down,
	);
	Some(amount_out.saturating_sub(1u128))
}

/// Calculate amount to be sent to the pool given the amount to be received from the pool.
/// D - number of iterations to use for Newton's formula ( it should be >=1 otherwise it wont converge at all and will always fail
/// Y - number of iterations to use for Newton's formula to calculate reserve Y ( it should be >=1 otherwise it wont converge at all and will always fail
pub fn calculate_in_given_out<const D: u8, const Y: u8>(
	initial_reserves: &[AssetReserve],
	idx_in: usize,
	idx_out: usize,
	amount_out: Balance,
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	if idx_in == idx_out {
		return None;
	}
	if idx_in >= initial_reserves.len() || idx_out >= initial_reserves.len() {
		return None;
	}
	let reserves = normalize_reserves(initial_reserves);
	let amount_out = normalize_value(
		amount_out,
		initial_reserves[idx_out].decimals,
		TARGET_PRECISION,
		Rounding::Down,
	);
	let new_reserve_in = calculate_y_given_out::<D, Y>(amount_out, idx_in, idx_out, &reserves, amplification, pegs)?;
	let amount_in = new_reserve_in.checked_sub(reserves[idx_in])?;
	let amount_in = normalize_value(
		amount_in,
		TARGET_PRECISION,
		initial_reserves[idx_in].decimals,
		Rounding::Up,
	);
	Some(amount_in.saturating_add(1u128))
}

/// Calculate amount to be received from the pool given the amount to be sent to the pool with fee applied.
pub fn calculate_out_given_in_with_fee<const D: u8, const Y: u8>(
	initial_reserves: &[AssetReserve],
	idx_in: usize,
	idx_out: usize,
	amount_in: Balance,
	amplification: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Balance)> {
	if idx_in == idx_out {
		return None;
	}
	let amount_out = calculate_out_given_in::<D, Y>(initial_reserves, idx_in, idx_out, amount_in, amplification, pegs)?;
	let fee_amount = calculate_fee_amount(amount_out, fee, Rounding::Down);
	let amount_out = amount_out.checked_sub(fee_amount)?;
	Some((amount_out, fee_amount))
}

/// Calculate amount to be sent to the pool given the amount to be received from the pool with fee applied.
pub fn calculate_in_given_out_with_fee<const D: u8, const Y: u8>(
	initial_reserves: &[AssetReserve],
	idx_in: usize,
	idx_out: usize,
	amount_out: Balance,
	amplification: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Balance)> {
	if idx_in == idx_out {
		return None;
	}
	let amount_in = calculate_in_given_out::<D, Y>(initial_reserves, idx_in, idx_out, amount_out, amplification, pegs)?;
	let fee_amount = calculate_fee_amount(amount_in, fee, Rounding::Up);
	let amount_in = amount_in.checked_add(fee_amount)?;
	Some((amount_in, fee_amount))
}

/// Calculate amount of shares to be given to LP after LP provided liquidity of some assets to the pool.
pub fn calculate_shares<const D: u8>(
	initial_reserves: &[AssetReserve],
	updated_reserves: &[AssetReserve],
	amplification: Balance,
	share_issuance: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Vec<Balance>)> {
	if initial_reserves.len() != updated_reserves.len() {
		return None;
	}
	let n_coins = initial_reserves.len();
	if n_coins <= 1 {
		return None;
	}
	let initial_d = calculate_d::<D>(initial_reserves, amplification, pegs.clone())?;

	// We must make sure the updated_d is rounded *down* so that we are not giving the new position too many shares.
	// calculate_d can return a D value that is above the correct D value by up to 2, so we subtract 2.
	let updated_d = calculate_d::<D>(updated_reserves, amplification, pegs.clone())?.checked_sub(2_u128)?;
	if updated_d < initial_d {
		return None;
	}
	let fixed_fee = FixedU128::from(fee);
	let fee = fixed_fee
		.checked_mul(&FixedU128::from(n_coins as u128))?
		.checked_div(&FixedU128::from(4 * (n_coins - 1) as u128))?;

	let (d0, d1) = to_u256!(initial_d, updated_d);

	let mut fees = vec![];
	let adjusted_reserves = if share_issuance > 0 {
		updated_reserves
			.iter()
			.enumerate()
			.map(|(idx, asset_reserve)| -> Option<AssetReserve> {
				let (initial_reserve, updated_reserve) = to_u256!(initial_reserves[idx].amount, asset_reserve.amount);
				let ideal_balance = d1.checked_mul(initial_reserve)?.checked_div(d0)?;
				let diff = Balance::try_from(updated_reserve.abs_diff(ideal_balance)).ok()?;
				let fee_amount = fee.checked_mul_int(diff)?;
				fees.push(fee_amount);
				Some(AssetReserve::new(
					asset_reserve.amount.saturating_sub(fee_amount),
					asset_reserve.decimals,
				))
			})
			.collect::<Option<Vec<AssetReserve>>>()?
	} else {
		updated_reserves.to_vec()
	};
	let adjusted_d = calculate_d::<D>(&adjusted_reserves, amplification, pegs)?;

	if share_issuance == 0 {
		// if first liquidity added
		Some((updated_d, fees))
	} else {
		let (issuance_hp, d_diff, d0) = to_u256!(share_issuance, adjusted_d.checked_sub(initial_d)?, initial_d);
		let share_amount = issuance_hp.checked_mul(d_diff)?.checked_div(d0)?;
		let shares_amount = Balance::try_from(share_amount).ok()?;

		Some((shares_amount, fees))
	}
}

/// Calculate amount of shares to be given to LP after LP provided liquidity of one asset with given amount.
pub fn calculate_shares_for_amount<const D: u8>(
	initial_reserves: &[AssetReserve],
	asset_idx: usize,
	amount: Balance,
	amplification: Balance,
	share_issuance: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Vec<Balance>)> {
	let n_coins = initial_reserves.len();
	if n_coins <= 1 {
		return None;
	}
	if asset_idx >= n_coins {
		return None;
	}
	let fixed_fee = FixedU128::from(fee);
	let fee = fixed_fee
		.checked_mul(&FixedU128::from(n_coins as u128))?
		.checked_div(&FixedU128::from(4 * (n_coins - 1) as u128))?;

	let updated_reserves: Vec<AssetReserve> = initial_reserves
		.iter()
		.enumerate()
		.map(|(idx, v)| -> Option<AssetReserve> {
			if idx == asset_idx {
				Some(AssetReserve::new(v.amount.checked_sub(amount)?, v.decimals))
			} else {
				Some(*v)
			}
		})
		.collect::<Option<Vec<AssetReserve>>>()?;

	let initial_d = calculate_d::<D>(initial_reserves, amplification, pegs.clone())?;
	let updated_d = calculate_d::<D>(&updated_reserves, amplification, pegs.clone())?;
	let (d1, d0) = to_u256!(updated_d, initial_d);
	let mut fees = vec![];
	let adjusted_reserves: Vec<AssetReserve> = updated_reserves
		.iter()
		.enumerate()
		.map(|(idx, asset_reserve)| -> Option<AssetReserve> {
			let (initial_reserve, updated_reserve) = to_u256!(initial_reserves[idx].amount, asset_reserve.amount);
			let ideal_balance = d1.checked_mul(initial_reserve)?.checked_div(d0)?;
			let diff = Balance::try_from(updated_reserve.abs_diff(ideal_balance)).ok()?;
			let fee_amount = fee.checked_mul_int(diff)?;
			fees.push(fee_amount);
			Some(AssetReserve::new(
				asset_reserve.amount.saturating_sub(fee_amount),
				asset_reserve.decimals,
			))
		})
		.collect::<Option<Vec<AssetReserve>>>()?;

	let adjusted_d = calculate_d::<D>(&adjusted_reserves, amplification, pegs)?;
	let (d_diff, issuance_hp) = to_u256!(initial_d.checked_sub(adjusted_d)?, share_issuance);
	let share_amount = issuance_hp
		.checked_mul(d_diff)?
		.checked_div(d0)?
		.checked_add(U256::one())?;
	let shares = Balance::try_from(share_amount).ok()?;

	Some((shares, fees))
}

pub fn calculate_liquidity_out(reserve: Balance, share_amount: Balance, share_issuance: Balance) -> Option<Balance> {
	let issuance_u256 = U256::from(share_issuance);
	let share_amount_u256 = U256::from(share_amount);
	Some(
		U256::from(reserve)
			.checked_mul(share_amount_u256)?
			.checked_div(issuance_u256)?
			.as_u128(),
	)
}

/// Given amount of shares and asset reserves, calculate corresponding amount of selected asset to be withdrawn.
/// Returns amount of asset to be withdrawn and fee amount. Note that fee amount is not deducted from amount of asset to be withdrawn.
pub fn calculate_withdraw_one_asset<const D: u8, const Y: u8>(
	reserves: &[AssetReserve],
	shares: Balance,
	asset_index: usize,
	share_asset_issuance: Balance,
	amplification: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Balance)> {
	if share_asset_issuance.is_zero() {
		return None;
	}

	if asset_index >= reserves.len() {
		return None;
	}

	if shares > share_asset_issuance {
		return None;
	}

	let n_coins = reserves.len();
	if n_coins <= 1 {
		return None;
	}
	let asset_out_decimals = reserves[asset_index].decimals;
	let reserves = normalize_reserves(reserves);

	let fixed_fee = FixedU128::from(fee);
	let fee = fixed_fee
		.checked_mul(&FixedU128::from(n_coins as u128))?
		.checked_div(&FixedU128::from(4 * (n_coins - 1) as u128))?;

	let initial_d = calculate_d_internal::<D>(&reserves, amplification, pegs.clone())?;
	let (shares_hp, issuance_hp, d_hp) = to_u256!(shares, share_asset_issuance, initial_d);

	let d1 = d_hp.checked_sub(shares_hp.checked_mul(d_hp)?.checked_div(issuance_hp)?)?;

	let xp: Vec<Balance> = reserves
		.iter()
		.enumerate()
		.filter(|(idx, _)| *idx != asset_index)
		.map(|(_, v)| *v)
		.collect();

	let (r_pegs, peg_omit) = if let Some(pegs) = pegs {
		let p_omit = pegs[asset_index];
		let r = pegs
			.into_iter()
			.enumerate()
			.filter(|(idx, _)| *idx != asset_index)
			.map(|(_, v)| v)
			.collect();
		(Some(r), Some(p_omit))
	} else {
		(None, None)
	};
	let y = calculate_y_internal::<Y>(
		&xp,
		Balance::try_from(d1).ok()?,
		amplification,
		r_pegs.clone(),
		peg_omit,
	)?;
	let xp_hp: Vec<U256> = reserves.iter().map(|v| to_u256!(*v)).collect();
	let y_hp = to_u256!(y);

	let mut reserves_reduced: Vec<Balance> = Vec::new();
	let mut asset_reserve: Balance = Balance::zero();

	for (idx, reserve) in xp_hp.iter().enumerate() {
		let dx_expected = if idx == asset_index {
			// dx_expected = xp[j] * d1 / d0 - new_y
			reserve.checked_mul(d1)?.checked_div(d_hp)?.checked_sub(y_hp)?
		} else {
			// dx_expected = xp[j] - xp[j] * d1 / d0
			reserve.checked_sub(reserve.checked_mul(d1)?.checked_div(d_hp)?)?
		};

		let expected = Balance::try_from(dx_expected).ok()?;
		let reduced = Balance::try_from(*reserve)
			.ok()?
			.checked_sub(fee.checked_mul_int(expected)?)?;

		if idx != asset_index {
			reserves_reduced.push(reduced);
		} else {
			asset_reserve = reduced;
		}
	}

	let y1 = calculate_y_internal::<Y>(
		&reserves_reduced,
		Balance::try_from(d1).ok()?,
		amplification,
		r_pegs,
		peg_omit,
	)?;
	let dy = asset_reserve.checked_sub(y1)?;
	let dy_0 = reserves[asset_index].checked_sub(y)?;
	let fee = dy_0.checked_sub(dy)?;

	let amount_out = normalize_value(dy, TARGET_PRECISION, asset_out_decimals, Rounding::Down);
	let fee = normalize_value(fee, TARGET_PRECISION, asset_out_decimals, Rounding::Down);
	Some((amount_out, fee))
}

/// Calculate amount of an asset that has to be added as liquidity to the pool in exchange of given amount of shares.
pub fn calculate_add_one_asset<const D: u8, const Y: u8>(
	reserves: &[AssetReserve],
	shares: Balance,
	asset_index: usize,
	share_asset_issuance: Balance,
	amplification: Balance,
	fee: Permill,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Balance)> {
	if share_asset_issuance.is_zero() {
		return None;
	}
	if asset_index >= reserves.len() {
		return None;
	}
	if shares > share_asset_issuance {
		return None;
	}
	let n_coins = reserves.len();
	if n_coins <= 1 {
		return None;
	}

	let asset_in_decimals = reserves[asset_index].decimals;
	let reserves = normalize_reserves(reserves);

	let initial_d = calculate_d_internal::<D>(&reserves, amplification, pegs.clone())?;
	let (shares_hp, issuance_hp, d_hp) = to_u256!(shares, share_asset_issuance, initial_d);

	let d1 = d_hp.checked_add(shares_hp.checked_mul(d_hp)?.checked_div(issuance_hp)?)?;

	let xp: Vec<Balance> = reserves
		.iter()
		.enumerate()
		.filter(|(idx, _)| *idx != asset_index)
		.map(|(_, v)| *v)
		.collect();

	let (r_pegs, peg_omit) = if let Some(pegs) = pegs {
		let p_omit = pegs[asset_index];
		let r = pegs
			.into_iter()
			.enumerate()
			.filter(|(idx, _)| *idx != asset_index)
			.map(|(_, v)| v)
			.collect();
		(Some(r), Some(p_omit))
	} else {
		(None, None)
	};

	let y = calculate_y_internal::<Y>(
		&xp,
		Balance::try_from(d1).ok()?,
		amplification,
		r_pegs.clone(),
		peg_omit,
	)?;

	let fixed_fee = FixedU128::from(fee);
	let fee = fixed_fee
		.checked_mul(&FixedU128::from(n_coins as u128))?
		.checked_div(&FixedU128::from(4 * (n_coins - 1) as u128))?;

	let xp_hp: Vec<U256> = reserves.iter().map(|v| to_u256!(*v)).collect();
	let y_hp = to_u256!(y);

	let mut reserves_reduced: Vec<Balance> = Vec::new();
	let mut asset_reserve: Balance = Balance::zero();

	for (idx, reserve) in xp_hp.iter().enumerate() {
		let dx_expected = if idx == asset_index {
			y_hp.checked_sub(reserve.checked_mul(d1)?.checked_div(d_hp)?)?
		} else {
			reserve.checked_mul(d1)?.checked_div(d_hp)?.checked_sub(*reserve)?
		};

		let expected = Balance::try_from(dx_expected).ok()?;
		let reduced = Balance::try_from(*reserve)
			.ok()?
			.checked_sub(fee.checked_mul_int(expected)?)?;

		if idx != asset_index {
			reserves_reduced.push(reduced);
		} else {
			asset_reserve = reduced;
		}
	}

	let y1 = calculate_y_internal::<Y>(
		&reserves_reduced,
		Balance::try_from(d1).ok()?,
		amplification,
		r_pegs,
		peg_omit,
	)?;
	let dy = y1.checked_sub(asset_reserve)?;
	let dy_0 = y.checked_sub(asset_reserve)?;
	let fee = dy.checked_sub(dy_0)?;
	let amount_in = normalize_value(dy, TARGET_PRECISION, asset_in_decimals, Rounding::Up);
	let fee = normalize_value(fee, TARGET_PRECISION, asset_in_decimals, Rounding::Down);
	Some((amount_in, fee))
}
pub fn calculate_d<const D: u8>(
	reserves: &[AssetReserve],
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	let n_reserves = normalize_reserves(reserves);
	calculate_d_internal::<D>(&n_reserves, amplification, pegs)
}

const fn calculate_ann(n: usize, amplification: Balance) -> Option<Balance> {
	amplification.checked_mul(n as u128)
}

/// Calculate new amount of reserve ID given amount to be added to the pool
pub(crate) fn calculate_y_given_in<const D: u8, const Y: u8>(
	amount: Balance,
	idx_in: usize,
	idx_out: usize,
	reserves: &[Balance],
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	if idx_in >= reserves.len() || idx_out >= reserves.len() {
		return None;
	}

	let new_reserve_in = reserves[idx_in].checked_add(amount)?;

	let d = calculate_d_internal::<D>(reserves, amplification, pegs.clone())?;

	let xp: Vec<Balance> = reserves
		.iter()
		.enumerate()
		.filter(|(idx, _)| *idx != idx_out)
		.map(|(idx, v)| if idx == idx_in { new_reserve_in } else { *v })
		.collect();

	let (r_pegs, peg_omit) = if let Some(pegs) = pegs.clone() {
		let p_omit = pegs[idx_out];
		let r = pegs
			.into_iter()
			.enumerate()
			.filter(|(idx, _)| *idx != idx_out)
			.map(|(_, v)| v)
			.collect();
		(Some(r), Some(p_omit))
	} else {
		(None, None)
	};

	calculate_y_internal::<Y>(&xp, d, amplification, r_pegs, peg_omit)
}

/// Calculate new amount of reserve IN given amount to be withdrawn from the pool
pub(crate) fn calculate_y_given_out<const D: u8, const Y: u8>(
	amount: Balance,
	idx_in: usize,
	idx_out: usize,
	reserves: &[Balance],
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	if idx_in >= reserves.len() || idx_out >= reserves.len() {
		return None;
	}
	let new_reserve_out = reserves[idx_out].checked_sub(amount)?;

	let d = calculate_d_internal::<D>(reserves, amplification, pegs.clone())?;
	let xp: Vec<Balance> = reserves
		.iter()
		.enumerate()
		.filter(|(idx, _)| *idx != idx_in)
		.map(|(idx, v)| if idx == idx_out { new_reserve_out } else { *v })
		.collect();

	let (r_pegs, peg_omit) = if let Some(pegs) = pegs {
		let p_omit = pegs[idx_in];
		let r = pegs
			.into_iter()
			.enumerate()
			.filter(|(idx, _)| *idx != idx_in)
			.map(|(_, v)| v)
			.collect();
		(Some(r), Some(p_omit))
	} else {
		(None, None)
	};

	calculate_y_internal::<Y>(&xp, d, amplification, r_pegs, peg_omit)
}

/// Calculate D invariant. Reserves must be already normalized.
pub(crate) fn calculate_d_internal<const D: u8>(
	xp: &[Balance],
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Balance> {
	let two_u256 = to_u256!(2_u128);

	let xp = if let Some(pegs) = pegs {
		if pegs.len() != xp.len() {
			return None;
		}

		xp.iter()
			.zip(pegs.iter())
			.map(|(v, peg)| {
				multiply_by_rational_with_rounding(*v, peg.0, peg.1, sp_arithmetic::per_things::Rounding::Down)
			})
			.collect::<Vec<Option<Balance>>>()
			.into_iter()
			.collect::<Option<Vec<Balance>>>()?
	} else {
		xp.to_vec()
	};

	// Filter out zero balance assets, and return error if there is one.
	// Either all assets are zero balance, or none are zero balance.
	// Otherwise, it breaks the math.
	let mut xp_hp: Vec<U256> = xp.iter().filter(|v| !(*v).is_zero()).map(|v| to_u256!(*v)).collect();
	if xp_hp.len() != xp.len() && !xp_hp.is_empty() {
		return None;
	}
	xp_hp.sort();

	let ann = calculate_ann(xp_hp.len(), amplification)?;
	let n_coins = to_u256!(xp_hp.len());

	let mut s_hp = U256::zero();
	for x in xp_hp.iter() {
		s_hp = s_hp.checked_add(*x)?;
	}

	if s_hp == U256::zero() {
		return Some(Balance::zero());
	}

	let mut d = s_hp;

	let (ann_hp, precision_hp) = to_u256!(ann, PRECISION as u128);

	for _ in 0..D {
		let d_p = xp_hp
			.iter()
			.try_fold(d, |acc, v| acc.checked_mul(d)?.checked_div(v.checked_mul(n_coins)?))?;
		let d_prev = d;

		d = ann_hp
			.checked_mul(s_hp)?
			.checked_add(d_p.checked_mul(n_coins)?)?
			.checked_mul(d)?
			.checked_div(
				ann_hp
					.checked_sub(U256::one())?
					.checked_mul(d)?
					.checked_add(n_coins.checked_add(U256::one())?.checked_mul(d_p)?)?,
			)?
			// adding two here is sufficient to account for rounding
			// errors, AS LONG AS the minimum reserves are 2 for each
			// asset. I.e., as long as xp_hp[0] >= 2 and xp_hp[1] >= 2
			// adding two guarantees that this function will return
			// a value larger than or equal to the correct D invariant
			.checked_add(two_u256)?;

		if has_converged(d_prev, d, precision_hp) {
			// If runtime-benchmarks - don't return and force max iterations
			#[cfg(not(feature = "runtime-benchmarks"))]
			return Balance::try_from(d).ok();
		}
	}

	Balance::try_from(d).ok()
}

/// Calculate Y. Reserves must be already normalized.
fn calculate_y_internal<const D: u8>(
	xp: &[Balance],
	d: Balance,
	amplification: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
	peg_omit: Option<(Balance, Balance)>,
) -> Option<Balance> {
	let xp = if let Some(pegs) = pegs {
		if pegs.len() != xp.len() {
			return None;
		}
		xp.iter()
			.zip(pegs.iter())
			.map(|(v, peg)| {
				multiply_by_rational_with_rounding(*v, peg.0, peg.1, sp_arithmetic::per_things::Rounding::Down)
			})
			.collect::<Vec<Option<Balance>>>()
			.into_iter()
			.collect::<Option<Vec<Balance>>>()?
	} else {
		xp.to_vec()
	};

	// Filter out zero balance assets, and return error if there is one.
	// Either all assets are zero balance, or none are zero balance.
	// Otherwise, it breaks the math.
	let mut xp_hp: Vec<U256> = xp.iter().filter(|v| !(*v).is_zero()).map(|v| to_u256!(*v)).collect();
	if xp_hp.len() != xp.len() && !xp_hp.is_empty() {
		return None;
	}
	xp_hp.sort();

	let ann = calculate_ann(xp_hp.len().checked_add(1)?, amplification)?;

	let (d_hp, n_coins_hp, ann_hp, precision_hp) = to_u256!(d, xp_hp.len().checked_add(1)?, ann, PRECISION as u128);

	let two_hp = to_u256!(2u128);
	let mut s_hp = U256::zero();
	for x in xp_hp.iter() {
		s_hp = s_hp.checked_add(*x)?;
	}
	let mut c = d_hp;

	for reserve in xp_hp.iter() {
		c = c.checked_mul(d_hp)?.checked_div(reserve.checked_mul(n_coins_hp)?)?;
	}

	c = c.checked_mul(d_hp)?.checked_div(ann_hp.checked_mul(n_coins_hp)?)?;

	let b = s_hp.checked_add(d_hp.checked_div(ann_hp)?)?;
	let mut y = d_hp;

	for _i in 0..D {
		let y_prev = y;
		y = y
			.checked_mul(y)?
			.checked_add(c)?
			.checked_div(two_hp.checked_mul(y)?.checked_add(b)?.checked_sub(d_hp)?)?
			.checked_add(two_hp)?;

		if has_converged(y_prev, y, precision_hp) {
			// If runtime-benchmarks - don't return and force max iterations
			if !cfg!(feature = "runtime-benchmarks") {
				let r = Balance::try_from(y).ok()?;
				if let Some(peg) = peg_omit {
					return multiply_by_rational_with_rounding(
						r,
						peg.1,
						peg.0,
						sp_arithmetic::per_things::Rounding::Down,
					);
				} else {
					return Some(r);
				}
			}
		}
	}
	let r = Balance::try_from(y).ok()?;
	if let Some(peg) = peg_omit {
		multiply_by_rational_with_rounding(r, peg.1, peg.0, sp_arithmetic::per_things::Rounding::Down)
	} else {
		Some(r)
	}
}

/// Calculate current amplification value.
pub fn calculate_amplification(
	initial_amplification: u128,
	final_amplification: u128,
	initial_block: u128,
	final_block: u128,
	current_block: u128,
) -> u128 {
	// short circuit if block parameters are invalid or start block is not reached yet
	if current_block < initial_block || final_block <= initial_block {
		return initial_amplification;
	}

	// short circuit if already reached desired block
	if current_block >= final_block {
		return final_amplification;
	}

	let step = final_amplification
		.abs_diff(initial_amplification)
		.saturating_mul(current_block.saturating_sub(initial_block))
		.div(final_block.saturating_sub(initial_block));

	if final_amplification > initial_amplification {
		initial_amplification.saturating_add(step)
	} else {
		initial_amplification.saturating_sub(step)
	}
}

#[inline]
fn has_converged(v0: U256, v1: U256, precision: U256) -> bool {
	let diff = abs_diff(v0, v1);

	(v1 <= v0 && diff < precision) || (v1 > v0 && diff <= precision)
}

#[inline]
fn abs_diff(d0: U256, d1: U256) -> U256 {
	if d1 >= d0 {
		// This is safe due the previous condition
		d1 - d0
	} else {
		d0 - d1
	}
}

pub(crate) enum Rounding {
	Down,
	Up,
}

fn calculate_fee_amount(amount: Balance, fee: Permill, rounding: Rounding) -> Balance {
	match rounding {
		Rounding::Down => fee.mul_floor(amount),
		Rounding::Up => fee.mul_ceil(amount),
	}
}

pub(crate) fn normalize_reserves(reserves: &[AssetReserve]) -> Vec<Balance> {
	reserves
		.iter()
		.map(|v| normalize_value(v.amount, v.decimals, TARGET_PRECISION, Rounding::Down))
		.collect()
}

pub(crate) fn normalize_value(amount: Balance, decimals: u8, target_decimals: u8, rounding: Rounding) -> Balance {
	if target_decimals == decimals {
		return amount;
	}
	let diff = target_decimals.abs_diff(decimals);
	if target_decimals > decimals {
		amount.saturating_mul(10u128.saturating_pow(diff as u32))
	} else {
		match rounding {
			Rounding::Down => amount.div(10u128.saturating_pow(diff as u32)),
			Rounding::Up => amount
				.div(10u128.saturating_pow(diff as u32))
				.saturating_add(Balance::one()),
		}
	}
}

pub fn calculate_share_prices<const D: u8>(
	reserves: &[AssetReserve],
	amplification: Balance,
	issuance: Balance,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<Vec<(Balance, Balance)>> {
	let n = reserves.len();
	if n <= 1 {
		return None;
	}

	let d = calculate_d::<D>(reserves, amplification, pegs.clone())?;

	let mut r = Vec::with_capacity(n);

	for idx in 0..n {
		let price = calculate_share_price::<D>(reserves, amplification, issuance, idx, Some(d), pegs.clone())?;
		r.push(price);
	}
	Some(r)
}

pub fn calculate_share_price<const D: u8>(
	reserves: &[AssetReserve],
	amplification: Balance,
	issuance: Balance,
	asset_idx: usize,
	provided_d: Option<Balance>,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<(Balance, Balance)> {
	let n = reserves.len() as u128;
	if n <= 1 || asset_idx >= reserves.len() {
		return None;
	}
	let d = if let Some(v) = provided_d {
		v
	} else {
		calculate_d::<D>(reserves, amplification, pegs.clone())?
	};
	let (adjusted_reserves, asset_peg) = if let Some(p) = pegs {
		if p.len() != reserves.len() {
			return None;
		}

		let mut x = vec![];
		for (v, mpl) in reserves.iter().zip(p.iter()) {
			let r_new =
				multiply_by_rational_with_rounding(v.amount, mpl.0, mpl.1, sp_arithmetic::per_things::Rounding::Down)?;
			x.push(AssetReserve {
				amount: r_new,
				decimals: v.decimals,
			});
		}
		(x, Some(p[asset_idx]))
	} else {
		(reserves.to_vec(), None)
	};
	let n_reserves = normalize_reserves(&adjusted_reserves);

	let c = n_reserves
		.iter()
		.try_fold(FixedU128::one(), |acc, reserve| {
			acc.checked_mul(&FixedU128::checked_from_rational(d, n.checked_mul(*reserve)?)?)
		})?
		.checked_mul_int(d)?;

	let ann = calculate_ann(n_reserves.len(), amplification)?;

	let (d, c, xi, n, ann, issuance) = to_u256!(d, c, n_reserves[asset_idx], n, ann, issuance);

	let xann = xi.checked_mul(ann)?;
	let p1 = d.checked_mul(xann)?;
	let p2 = xi.checked_mul(c)?.checked_mul(n.saturating_add(U256::one()))?;
	let p3 = xi.checked_mul(d)?;

	let num = p1.checked_add(p2)?.checked_sub(p3)?;
	let denom = issuance.checked_mul(xann.checked_add(c)?)?;

	let p_diff = U256::from(10u128.saturating_pow(18u8.saturating_sub(reserves[asset_idx].decimals) as u32));
	let (num, denom) = if let Some(v) = denom.checked_mul(p_diff) {
		(num, v)
	} else {
		// Rare scenario
		// In case of overflow, we can just simply divide the numerator
		// We loose little bit of precision but it is acceptable
		// Can be with asset with 6 decimals.
		let num = num.checked_div(p_diff)?;
		(num, denom)
	};
	let (num, denom) = round_to_rational((num, denom), crate::support::rational::Rounding::Down);
	if let Some(peg) = asset_peg {
		let c: Ratio = (num, denom).into();
		let peg: Ratio = peg.into();
		let result = c.saturating_div(&peg);
		Some((result.n, result.d))
	} else {
		Some((num, denom))
	}
}

const STABLE_ASSET: bool = false;
const SHARE_ASSET: bool = true;

/// Calculating spot price between two assets in stablepool, including the impact of the fee
///
/// An asset can be either a stable asset or a share asset
///
/// Returns price of asset_out denominated in asset_in (asset_in/asset_out)
///
/// - `pool_id` - id of the pool
/// - `reserves` - reserve balances of assets
/// - `amplification` - curve AMM pool amplification parameter
/// - `asset_in` - asset id of asset in
/// - `asset_out` - asset id of asset out
/// - `share_issuance` - total issuance of the share
/// - `min_trade_amount` - min trade amount of stableswap
/// - `pool_fee` - fee of the pool
///
#[allow(clippy::too_many_arguments)]
pub fn calculate_spot_price(
	pool_id: AssetId,
	asset_reserves: Vec<(AssetId, AssetReserve)>,
	amplification: Balance,
	asset_in: AssetId,
	asset_out: AssetId,
	share_issuance: Balance,
	min_trade_amount: Balance,
	fee: Option<Permill>,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<FixedU128> {
	let reserves = asset_reserves
		.clone()
		.into_iter()
		.map(|(_, v)| v)
		.collect::<Vec<AssetReserve>>();

	let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amplification, pegs.clone())?;

	match (asset_in == pool_id, asset_out == pool_id) {
		(STABLE_ASSET, STABLE_ASSET) => {
			let asset_in_idx = asset_reserves.iter().position(|r| r.0 == asset_in)?;
			let asset_out_idx = asset_reserves.iter().position(|r| r.0 == asset_out)?;
			calculate_spot_price_between_two_stable_assets(
				&reserves,
				amplification,
				d,
				asset_in_idx,
				asset_out_idx,
				fee,
				pegs,
			)
		}
		(SHARE_ASSET, STABLE_ASSET) => {
			let asset_out_idx = asset_reserves.iter().position(|r| r.0 == asset_out)?;
			let (shares, _fees) = calculate_shares_for_amount::<MAX_D_ITERATIONS>(
				&reserves,
				asset_out_idx,
				min_trade_amount,
				amplification,
				share_issuance,
				fee.unwrap_or(Permill::zero()),
				pegs.clone(),
			)?;

			FixedU128::checked_from_rational(shares, min_trade_amount)
		}
		(STABLE_ASSET, SHARE_ASSET) => {
			let added_asset = (asset_in, min_trade_amount);

			let mut updated_reserves = asset_reserves.clone();
			for reserve in updated_reserves.iter_mut() {
				if reserve.0 == added_asset.0 {
					reserve.1.amount = reserve.1.amount.checked_add(added_asset.1)?;
				}
			}

			let update_reserves: &Vec<AssetReserve> = &updated_reserves.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
			let (shares_for_min_trade, _fees) = calculate_shares::<MAX_D_ITERATIONS>(
				&reserves,
				update_reserves,
				amplification,
				share_issuance,
				fee.unwrap_or(Permill::zero()),
				pegs.clone(),
			)?;

			FixedU128::checked_from_rational(min_trade_amount, shares_for_min_trade)
		}
		_ => None,
	}
}

/// Calculating spot price between two stable asset AB, including the impact of the fee
///
/// Returns price of asset_out denominated in asset_in (asset_in/asset_out)
///
/// - `reserves` - reserve balances of assets
/// - `amplification` - curve AMM pool amplification parameter
/// - `d` - D invariant
/// - `asset_in_idx` - asset in index
/// - `asset_out_idx` - asset out index
/// - `fee` - fee of the pool
///
pub fn calculate_spot_price_between_two_stable_assets(
	reserves: &[AssetReserve],
	amplification: Balance,
	d: Balance,
	asset_in_idx: usize,
	asset_out_idx: usize,
	fee: Option<Permill>,
	pegs: Option<Vec<(Balance, Balance)>>,
) -> Option<FixedU128> {
	let n = reserves.len();
	if n <= 1 || asset_in_idx >= n || asset_out_idx >= n {
		return None;
	}
	let ann = calculate_ann(n, amplification)?;

	let (adjusted_reserves, asset_peg_in, asset_peg_out) = if let Some(p) = pegs {
		if p.len() != reserves.len() {
			return None;
		}

		let mut x = vec![];
		for (v, mpl) in reserves.iter().zip(p.iter()) {
			let r_new =
				multiply_by_rational_with_rounding(v.amount, mpl.0, mpl.1, sp_arithmetic::per_things::Rounding::Down)?;
			x.push(AssetReserve {
				amount: r_new,
				decimals: v.decimals,
			});
		}
		(x, Some(p[asset_in_idx]), Some(p[asset_out_idx]))
	} else {
		(reserves.to_vec(), None, None)
	};

	let mut n_reserves = normalize_reserves(&adjusted_reserves);

	let x0 = n_reserves[asset_in_idx];
	let xi = n_reserves[asset_out_idx];

	let (n, d, ann, x0, xi) = to_u256!(n, d, ann, x0, xi);

	n_reserves.sort();
	let reserves_hp: Vec<U256> = n_reserves.iter().map(|v| U256::from(*v)).collect();
	let c = reserves_hp
		.iter()
		.try_fold(d, |acc, val| acc.checked_mul(d)?.checked_div(val.checked_mul(n)?))?;

	let num = x0.checked_mul(ann.checked_mul(xi)?.checked_add(c)?)?;
	let denom = xi.checked_mul(ann.checked_mul(x0)?.checked_add(c)?)?;

	let spot_price = round_to_rational((num, denom), crate::support::rational::Rounding::Down);

	let spot_price = if let (Some(peg_in), Some(peg_out)) = (asset_peg_in, asset_peg_out) {
		let price: Ratio = spot_price.into();
		let peg_in: Ratio = peg_in.into();
		let peg_out: Ratio = peg_out.into();
		let result = price.saturating_mul(&peg_in).saturating_div(&peg_out);
		(result.n, result.d)
	} else {
		spot_price
	};

	if let Some(fee) = fee {
		// Amount_out is reduced by fee in SELL, making asset_out more expensive, so the asset_in/asset_out spot price should be increased.
		// So divide spot-price-without-fee by (1-fee) to reflect correct amount out after the fee deduction
		let fee_multiplier = Permill::from_percent(100).checked_sub(&fee)?;
		FixedU128::checked_from_rational(spot_price.0, fee_multiplier.mul_floor(spot_price.1))
	} else {
		FixedU128::checked_from_rational(spot_price.0, spot_price.1)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::assert_approx_eq;

	#[test]
	fn test_normalize_value_same_decimals() {
		let amount = 1_000_000_000_000;
		let decimals = 12;
		let target_decimals = 12;
		let expected: Balance = amount;
		let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
		assert_eq!(actual, expected);
	}

	#[test]
	fn test_normalize_value_target_greater_than_decimals() {
		let amount = 1_000_000_000_000;
		let decimals = 12;
		let target_decimals = 18;
		let expected: Balance = 1_000_000_000_000_000_000;
		let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
		assert_eq!(actual, expected);
	}

	#[test]
	fn test_normalize_value_target_less_than_decimals() {
		let amount: Balance = 1_000_000_000_000_000_000;
		let decimals = 18;
		let target_decimals = 12;
		let expected: Balance = 1_000_000_000_000;
		let actual = normalize_value(amount, decimals, target_decimals, Rounding::Down);
		assert_eq!(actual, expected);
	}

	#[test]
	fn spot_price_calculation_should_work_with_12_decimals() {
		let reserves = vec![
			AssetReserve::new(478_626_000_000_000_000_000, 12),
			AssetReserve::new(487_626_000_000_000_000_000, 12),
			AssetReserve::new(866_764_000_000_000_000_000, 12),
			AssetReserve::new(518_696_000_000_000_000_000, 12),
		];
		let amp = 319u128;
		let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, None).unwrap();
		let p = calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 0, 1, None, None).unwrap();
		assert_approx_eq!(
			p,
			FixedU128::from_rational(
				259416830506303392284340673024338472588,
				259437723055509887749072196895052016056
			),
			FixedU128::from((2, (1_000_000_000_000u128 / 10_000))),
			"the relative difference is not as expected"
		);
		let reserves = vec![
			AssetReserve::new(1_001_000_000_000_000_000, 12),
			AssetReserve::new(1_000_000_000_000_000_000, 12),
			AssetReserve::new(1_000_000_000_000_000_000, 12),
			AssetReserve::new(1_000_000_000_000_000_000, 12),
		];
		let amp = 10u128;
		let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, None).unwrap();
		let p = calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 0, 1, None, None).unwrap();
		assert_approx_eq!(
			p,
			FixedU128::from_rational(
				320469570070413807187663384895131457597,
				320440458954331380180651678529102355242
			),
			FixedU128::from((2, (1_000_000_000_000u128 / 10_000))),
			"the relative difference is not as expected"
		);
	}

	#[test]
	fn spot_price_calculation_should_fail_gracefully_with_invalid_indexes() {
		let reserves = vec![
			AssetReserve::new(478_626_000_000_000_000_000, 12),
			AssetReserve::new(487_626_000_000_000_000_000, 12),
			AssetReserve::new(866_764_000_000_000_000_000, 12),
			AssetReserve::new(518_696_000_000_000_000_000, 12),
		];
		let amp = 10u128;
		let d = calculate_d::<MAX_D_ITERATIONS>(&reserves, amp, None).unwrap();

		assert!(calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 4, 1, None, None).is_none());
		assert!(calculate_spot_price_between_two_stable_assets(&reserves, amp, d, 1, 4, None, None).is_none());
	}

	#[test]
	fn share_price_calculation_should_fail_gracefully_with_invalid_indexes() {
		let reserves = vec![
			AssetReserve::new(478_626_000_000_000_000_000, 12),
			AssetReserve::new(487_626_000_000_000_000_000, 12),
			AssetReserve::new(866_764_000_000_000_000_000, 12),
			AssetReserve::new(518_696_000_000_000_000_000, 12),
		];
		let amp = 10u128;

		assert!(calculate_share_price::<MAX_D_ITERATIONS>(&reserves, amp, 1000000000000000, 4, None, None).is_none());
	}
}
