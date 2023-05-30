use crate::types::Balance;
use sp_arithmetic::FixedU128;

/// Fee parameters - minimum and maximum fee, decay and amplification.
#[derive(Debug, Clone)]
pub struct FeeParams<Fee> {
	pub min_fee: Fee,
	pub max_fee: Fee,
	pub decay: FixedU128,
	pub amplification: FixedU128,
}

/// Oracle entry data for an asset, providing amount in and out and total liquidity of an asset.
#[derive(Debug, Clone)]
pub struct OracleEntry {
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub liquidity: Balance,
}

impl OracleEntry {
	/// Returns the difference between the in and out balance and information if the difference is negative.
	pub(super) fn net_volume(&self, direction: NetVolumeDirection) -> (Balance, bool) {
		match direction {
			NetVolumeDirection::OutIn => (
				self.amount_out.abs_diff(self.amount_in),
				self.amount_out < self.amount_in,
			),
			NetVolumeDirection::InOut => (
				self.amount_out.abs_diff(self.amount_in),
				self.amount_out > self.amount_in,
			),
		}
	}
}

/// Internal helper enum to indicate the direction of the liquidity.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) enum NetVolumeDirection {
	/// Amount Out - Amount in. Used to calculate asset fee.
	OutIn,
	/// Amount In - Amount Out. Used to calculate protocol fee.
	InOut,
}
