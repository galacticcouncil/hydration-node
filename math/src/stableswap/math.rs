use crate::to_u256;
use crate::types::Balance;
use num_traits::{CheckedDiv, CheckedMul, Zero};
use primitive_types::U256;
use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};
use sp_std::prelude::*;

pub const MAX_Y_ITERATIONS: u8 = 128;
pub const MAX_D_ITERATIONS: u8 = 64;

const PRECISION: u8 = 1;

/// Calculating amount to be received from the pool given the amount to be sent to the pool and both reserves.
/// N - number of iterations to use for Newton's formula to calculate parameter D ( it should be >=1 otherwise it wont converge at all and will always fail
/// N_Y - number of iterations to use for Newton's formula to calculate reserve Y ( it should be >=1 otherwise it wont converge at all and will always fail
pub fn calculate_out_given_in<const N: u8, const N_Y: u8>(
    balances: &[Balance],
    idx_in: usize,
    idx_out: usize,
    amount_in: Balance,
    amplification: Balance,
) -> Option<Balance> {
    if idx_in >= balances.len() || idx_out >= balances.len() {
        return None;
    }
    let new_reserve_out = calculate_y_given_in::<N, N_Y>(amount_in, idx_in, idx_out, balances, amplification)?;
    balances[idx_out].checked_sub(new_reserve_out)
}

/// Calculating amount to be sent to the pool given the amount to be received from the pool and both reserves.
/// N - number of iterations to use for Newton's formula ( it should be >=1 otherwise it wont converge at all and will always fail
/// N_Y - number of iterations to use for Newton's formula to calculate reserve Y ( it should be >=1 otherwise it wont converge at all and will always fail
pub fn calculate_in_given_out<const N: u8, const N_Y: u8>(
    balances: &[Balance],
    idx_in: usize,
    idx_out: usize,
    amount_out: Balance,
    amplification: Balance,
) -> Option<Balance> {
    if idx_in >= balances.len() || idx_out >= balances.len() {
        return None;
    }
    let new_reserve_in = calculate_y_given_out::<N, N_Y>(amount_out, idx_in, idx_out, balances, amplification)?;
    new_reserve_in.checked_sub(balances[idx_in])
}

/// Calculating amount to be received from the pool given the amount to be sent to the pool and both reserves and apply a fee.
pub fn calculate_out_given_in_with_fee<const N: u8, const N_Y: u8>(
    balances: &[Balance],
    idx_in: usize,
    idx_out: usize,
    amount_in: Balance,
    amplification: Balance,
    fee: Permill,
) -> Option<(Balance, Balance)> {
    let amount_out = calculate_out_given_in::<N, N_Y>(balances, idx_in, idx_out, amount_in, amplification)?;
    let fee_amount = calculate_fee_amount(amount_out, fee, Rounding::Down);

    let amount_out = amount_out.checked_sub(fee_amount)?;

    Some((amount_out, fee_amount))
}

/// Calculating amount to be sent to the pool given the amount to be received from the pool and both reserves with fee applied.
pub fn calculate_in_given_out_with_fee<const N: u8, const N_Y: u8>(
    balances: &[Balance],
    idx_in: usize,
    idx_out: usize,
    amount_out: Balance,
    amplification: Balance,
    fee: Permill,
) -> Option<(Balance, Balance)> {
    let amount_in = calculate_in_given_out::<N, N_Y>(balances, idx_in, idx_out, amount_out, amplification)?;
    let fee_amount = calculate_fee_amount(amount_in, fee, Rounding::Up);

    let amount_in = amount_in.checked_add(fee_amount)?;

    Some((amount_in, fee_amount))
}

/// Calculate amount of shares to be given to LP after LP provided liquidity of some assets to the pool.
pub fn calculate_shares<const N: u8>(
    initial_reserves: &[Balance],
    updated_reserves: &[Balance],
    amplification: Balance,
    share_issuance: Balance,
) -> Option<Balance> {
    if initial_reserves.len() != updated_reserves.len() {
        return None;
    }

    let initial_d = calculate_d::<N>(initial_reserves, amplification)?;

    // We must make sure the updated_d is rounded *down* so that we are not giving the new position too many shares.
    // calculate_d can return a D value that is above the correct D value by up to 2, so we subtract 2.
    let updated_d = calculate_d::<N>(updated_reserves, amplification)?.checked_sub(2_u128)?;

    if updated_d < initial_d {
        return None;
    }

    if share_issuance == 0 {
        // if first liquidity added
        Some(updated_d)
    } else {
        let (issuance_hp, d_diff, d0) = to_u256!(share_issuance, updated_d.checked_sub(initial_d)?, initial_d);
        let share_amount = issuance_hp.checked_mul(d_diff)?.checked_div(d0)?;
        Balance::try_from(share_amount).ok()
    }
}

/// Given amount of shares and asset reserves, calculate corresponding amount of selected asset to be withdrawn.
pub fn calculate_withdraw_one_asset<const N: u8, const N_Y: u8>(
    reserves: &[Balance],
    shares: Balance,
    asset_index: usize,
    share_asset_issuance: Balance,
    amplification: Balance,
    fee: Permill,
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
    let fixed_fee = FixedU128::from(fee);
    let fee = fixed_fee
        .checked_mul(&FixedU128::from(n_coins as u128))?
        .checked_div(&FixedU128::from(4 * (n_coins - 1) as u128))?;

    let initial_d = calculate_d::<N>(reserves, amplification)?;

    let (shares_hp, issuance_hp, d_hp) = to_u256!(shares, share_asset_issuance, initial_d);

    let d1 = d_hp.checked_sub(shares_hp.checked_mul(d_hp)?.checked_div(issuance_hp)?)?;

    let xp: Vec<Balance> = reserves
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != asset_index)
        .map(|(_, v)| *v)
        .collect();

    let y = calculate_y::<N_Y>(&xp, Balance::try_from(d1).ok()?, amplification)?;

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
            reserve.checked_sub(xp_hp[idx].checked_mul(d1)?.checked_div(d_hp)?)?
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

    let y1 = calculate_y::<N_Y>(&reserves_reduced, Balance::try_from(d1).ok()?, amplification)?;

    let dy = asset_reserve.checked_sub(y1)?;

    let dy_0 = reserves[asset_index].checked_sub(y)?;

    let fee = dy_0.checked_sub(dy)?;

    Some((dy, fee))
}

/// amplification * n^n where n is number of assets in pool.
pub(crate) fn calculate_ann(len: usize, amplification: Balance) -> Option<Balance> {
    (0..len).try_fold(amplification, |acc, _| acc.checked_mul(len as u128))
}

pub(crate) fn calculate_y_given_in<const N: u8, const N_Y: u8>(
    amount: Balance,
    idx_in: usize,
    idx_out: usize,
    balances: &[Balance],
    amplification: Balance,
) -> Option<Balance> {
    if idx_in >= balances.len() || idx_out >= balances.len() {
        return None;
    }

    let new_reserve_in = balances[idx_in].checked_add(amount)?;

    let d = calculate_d::<N>(balances, amplification)?;

    let xp: Vec<Balance> = balances
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != idx_out)
        .map(|(idx, v)| if idx == idx_in { new_reserve_in } else { *v })
        .collect();

    calculate_y::<N_Y>(&xp, d, amplification)
}

/// Calculate new amount of reserve IN given amount to be withdrawn from the pool
pub(crate) fn calculate_y_given_out<const N: u8, const N_Y: u8>(
    amount: Balance,
    idx_in: usize,
    idx_out: usize,
    balances: &[Balance],
    amplification: Balance,
) -> Option<Balance> {
    if idx_in >= balances.len() || idx_out >= balances.len() {
        return None;
    }
    let new_reserve_out = balances[idx_out].checked_sub(amount)?;

    let d = calculate_d::<N>(balances, amplification)?;
    let xp: Vec<Balance> = balances
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != idx_in)
        .map(|(idx, v)| if idx == idx_out { new_reserve_out } else { *v })
        .collect();

    calculate_y::<N_Y>(&xp, d, amplification)
}

pub fn calculate_d<const N: u8>(xp: &[Balance], amplification: Balance) -> Option<Balance> {
    let two_u256 = to_u256!(2_u128);

    //let mut xp_hp: [U256; 2] = [to_u256!(xp[0]), to_u256!(xp[1])];
    let mut xp_hp: Vec<U256> = xp.iter().filter(|v| !(*v).is_zero()).map(|v| to_u256!(*v)).collect();
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

    for _ in 0..N {
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

pub(crate) fn calculate_y<const N: u8>(xp: &[Balance], d: Balance, amplification: Balance) -> Option<Balance> {
    let mut xp_hp: Vec<U256> = xp.iter().filter(|v| !(*v).is_zero()).map(|v| to_u256!(*v)).collect();
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

    for _i in 0..N {
        let y_prev = y;
        y = y
            .checked_mul(y)?
            .checked_add(c)?
            .checked_div(two_hp.checked_mul(y)?.checked_add(b)?.checked_sub(d_hp)?)?
            .checked_add(two_hp)?;

        if has_converged(y_prev, y, precision_hp) {
            // If runtime-benchmarks - don't return and force max iterations
            #[cfg(not(feature = "runtime-benchmarks"))]
            return Balance::try_from(y).ok();
        }
    }
    Balance::try_from(y).ok()
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

enum Rounding {
    Down,
    Up,
}

fn calculate_fee_amount(amount: Balance, fee: Permill, rounding: Rounding) -> Balance {
    match rounding {
        Rounding::Down => fee.mul_floor(amount),
        Rounding::Up => fee.mul_ceil(amount),
    }
}
