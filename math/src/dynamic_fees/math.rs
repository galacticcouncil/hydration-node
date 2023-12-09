use super::types::*;
use crate::dynamic_fees::types::NetVolumeDirection::{InOut, OutIn};
use num_traits::Zero;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::{FixedPointOperand, FixedU128, PerThing};

/// Recalculate Omnipool's asset fee given previously calculated fee and oracle data.
///
/// `volume` is the asset volume data provided by the oracle.
/// `previous_fee` is the previous calculated asset fee.
/// `last_block_diff` is the difference between the current block height and the previous block height when asset fee was calculated.
/// `params` is the fee parameters, such as minimum fee, maximum fee, decay and amplification.
pub fn recalculate_asset_fee<Fee: PerThing>(
	volume: OracleEntry,
	previous_fee: Fee,
	last_block_diff: u128,
	params: FeeParams<Fee>,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	recalculate_fee(volume, previous_fee, last_block_diff, params, OutIn)
}

/// Recalculate Omnipool's protocol fee given previously calculated fee and oracle data.
///
/// `volume` is the asset volume data provided by the oracle.
/// `previous_fee` is the previous calculated protocol fee.
/// `last_block_diff` is the difference between the current block height and the previous block height when asset fee was calculated.
/// `params` is the fee parameters, such as minimum fee, maximum fee, decay and amplification.
pub fn recalculate_protocol_fee<Fee: PerThing>(
	volume: OracleEntry,
	previous_fee: Fee,
	last_block_diff: u128,
	params: FeeParams<Fee>,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	recalculate_fee(volume, previous_fee, last_block_diff, params, InOut)
}

fn recalculate_fee<Fee: PerThing>(
	volume: OracleEntry,
	previous_fee: Fee,
	last_block_diff: u128,
	params: FeeParams<Fee>,
	direction: NetVolumeDirection,
) -> Fee
where
	<Fee as PerThing>::Inner: FixedPointOperand,
{
	// Adjust previous fee which may not have been calculated in previous block
	let fixed_previous_fee = if last_block_diff > 1 {
		let decaying = params
			.decay
			.saturating_mul(FixedU128::from(last_block_diff.saturating_sub(1)));
		FixedU128::from(previous_fee)
			.saturating_sub(decaying)
			.max(params.min_fee.into())
	} else {
		previous_fee.into()
	};

	// 1. if asset fee, direction OutIn : x = (Vo - Vi) / L
	// 2. if protocol fee, direction InOut: x = (Vi - Vo) / L
	let (x, x_neg) = if !volume.liquidity.is_zero() {
		let (diff, neg) = volume.net_volume(direction);
		(FixedU128::from_rational(diff, volume.liquidity), neg)
	} else {
		(FixedU128::zero(), false)
	};

	let a_x = params.amplification.saturating_mul(x);

	// Work out fee adjustment taking into account possible negative result
	let (delta_f, neg) = if x_neg {
		(a_x.saturating_add(params.decay), true)
	} else if a_x > params.decay {
		(a_x.saturating_sub(params.decay), false)
	} else {
		(params.decay.saturating_sub(a_x), true)
	};

	if neg {
		fixed_previous_fee.saturating_sub(delta_f)
	} else {
		fixed_previous_fee.saturating_add(delta_f)
	}
	.into_clamped_perthing::<Fee>()
	.clamp(params.min_fee, params.max_fee)
}
