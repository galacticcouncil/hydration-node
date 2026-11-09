//! USD value carried as a fixed-point i128 with 6 decimals (USDT's own scale).
//!
//! The reference tool carried these as bare `i128` with the scale living in comments;
//! sign/scale mix-ups made the newtype worth it.

use core::fmt;
use core::iter::Sum;
use core::ops::Neg;

/// Signed USD amount, 6 decimal places (1_000_000 = $1). All arithmetic saturates:
/// detection must degrade to a clamped number, never wrap or panic.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct Usd6(pub i128);

impl Usd6 {
	pub const ZERO: Usd6 = Usd6(0);
	pub const ONE: Usd6 = Usd6(1_000_000);

	pub fn from_dollars(d: f64) -> Self {
		Usd6((d * 1e6) as i128)
	}

	pub fn to_dollars(self) -> f64 {
		self.0 as f64 / 1e6
	}

	pub fn saturating_add(self, rhs: Usd6) -> Usd6 {
		Usd6(self.0.saturating_add(rhs.0))
	}

	pub fn saturating_sub(self, rhs: Usd6) -> Usd6 {
		Usd6(self.0.saturating_sub(rhs.0))
	}

	pub fn abs(self) -> Usd6 {
		Usd6(self.0.saturating_abs())
	}

	pub fn is_positive(self) -> bool {
		self.0 > 0
	}
}

impl Neg for Usd6 {
	type Output = Usd6;
	fn neg(self) -> Usd6 {
		Usd6(self.0.saturating_neg())
	}
}

impl Sum for Usd6 {
	fn sum<I: Iterator<Item = Usd6>>(iter: I) -> Usd6 {
		iter.fold(Usd6::ZERO, Usd6::saturating_add)
	}
}

impl fmt::Display for Usd6 {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let sign = if self.0 < 0 { "-" } else { "" };
		let a = self.0.unsigned_abs();
		write!(f, "{sign}${}.{:04}", a / 1_000_000, (a % 1_000_000) / 100)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn display_should_render_four_decimals_when_formatting() {
		assert_eq!(Usd6(4_327_820_297).to_string(), "$4327.8202");
		assert_eq!(Usd6(-280_000).to_string(), "-$0.2800");
		assert_eq!(Usd6::ZERO.to_string(), "$0.0000");
	}

	#[test]
	fn arithmetic_should_saturate_when_overflowing() {
		assert_eq!(Usd6(i128::MAX).saturating_add(Usd6::ONE).0, i128::MAX);
		assert_eq!(Usd6(i128::MIN).saturating_sub(Usd6::ONE).0, i128::MIN);
	}

	#[test]
	fn sum_should_fold_all_items_when_collected() {
		let total: Usd6 = [Usd6::ONE, Usd6::ONE, Usd6(-500_000)].into_iter().sum();
		assert_eq!(total, Usd6(1_500_000));
	}
}
