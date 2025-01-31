use super::types::*;
use crate::dynamic_fees::types::NetVolumeDirection::{InOut, OutIn};
use num_traits::One;
use num_traits::Zero;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::FixedPointNumber;
use sp_arithmetic::{FixedPointOperand, FixedU128, PerThing};

/// Recalculate Omnipool's asset fee given previously calculated fee and oracle data.
///
/// `volume` is the asset volume data provided by the oracle.
/// `current_asset_liquidity` is the current asset liquidity.
/// `previous_fee` is the previous-calculated asset fee.
/// `last_block_diff` is the difference between the current block height and the previous block height when asset fee was calculated.
/// `params` is the fee parameters, such as minimum fee, maximum fee, decay and amplification.
pub fn recalculate_asset_fee<Fee: PerThing>(
	last_entry: OracleEntry,
	current_asset_liquidity: u128,
	previous_fee: Fee,
	block_diff: u128,
	params: FeeParams<Fee>,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	compute_dynamic_fee(
		last_entry,
		current_asset_liquidity,
		params,
		previous_fee,
		block_diff,
		OutIn,
	)
}

/// Recalculate Omnipool's protocol fee given previously calculated fee and oracle data.
///
/// `volume` is the asset volume data provided by the oracle.
/// 'current_asset_liquidity' is the current asset liquidity.
/// `previous_fee` is the previous-calculated protocol fee.
/// `last_block_diff` is the difference between the current block height and the previous block height when asset fee was calculated.
/// `params` is the fee parameters, such as minimum fee, maximum fee, decay and amplification.
pub fn recalculate_protocol_fee<Fee: PerThing>(
	last_entry: OracleEntry,
	current_asset_liquidity: u128,
	previous_fee: Fee,
	block_diff: u128,
	params: FeeParams<Fee>,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	compute_dynamic_fee(
		last_entry,
		current_asset_liquidity,
		params,
		previous_fee,
		block_diff,
		InOut,
	)
}
pub(super) fn compute_dynamic_fee<Fee: PerThing>(
	last_entry: OracleEntry,
	liquidity: u128,
	params: FeeParams<Fee>,
	previous_fee: Fee,
	block_diff: u128,
	net_direction: NetVolumeDirection,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	if params.amplification.is_zero() || block_diff.is_zero() {
		return previous_fee;
	}
	if last_entry.decay_factor.is_zero() {
		return previous_fee; //otherwise we would divide by zero
	}

	let (net_volume, neg) = last_entry.net_volume(net_direction);
	let (net_liquidity, liquid_neg) = (
		last_entry.liquidity.abs_diff(liquidity),
		last_entry.liquidity < liquidity,
	);

	let m = block_diff.min(20u128);
	let (x, x_neg) = (
		FixedU128::from_rational(params.amplification.saturating_mul_int(net_volume), liquidity),
		neg,
	);
	let mut j_sum = FixedU128::zero();

	let w = FixedU128::one().saturating_sub(last_entry.decay_factor);

	for j in 0..m {
		let oracle_value = w.saturating_pow(j as usize);
		let n = FixedU128::from_rational(net_liquidity, liquidity);
		let p = n.saturating_mul(oracle_value);
		let denom = if liquid_neg {
			FixedU128::one().saturating_sub(p)
		} else {
			FixedU128::one().saturating_add(p)
		};
		// this should not happen but let's be cautious
		if denom.is_zero() {
			// let's make fuzzer happy to panic here!
			debug_assert!(false, "Denominator is zero");
			return previous_fee;
		}
		j_sum = j_sum.saturating_add(oracle_value.div(denom)); //safe because of previous check
	}

	let w_term = w
		.saturating_mul(
			w.saturating_pow(m as usize)
				.saturating_sub(w.saturating_pow(block_diff as usize)),
		)
		.div(last_entry.decay_factor); //safe because of previous check

	let p1 = j_sum.saturating_add(w_term);
	let p2 = x.saturating_mul(p1);

	let bd = FixedU128::from(block_diff);
	let f = params.decay.saturating_mul(bd);
	let (delta, delta_neg) = if x_neg {
		(p2.saturating_add(f), true)
	} else if f > p2 {
		(f.saturating_sub(p2), true)
	} else {
		(p2.saturating_sub(f), false)
	};
	let fixed_previous_fee: FixedU128 = previous_fee.into();
	if delta_neg {
		fixed_previous_fee.saturating_sub(delta)
	} else {
		fixed_previous_fee.saturating_add(delta)
	}
	.into_clamped_perthing::<Fee>()
	.clamp(params.min_fee, params.max_fee)
}
