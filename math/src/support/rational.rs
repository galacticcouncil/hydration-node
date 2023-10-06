use primitive_types::{U256, U512};

/// Enum to specify how to round a rational number.
/// `Nearest` rounds both numerator and denominator down.
/// `Down` ensures the output is less than or equal to the input.
/// `Up` ensures the output is greater than or equal to the input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rounding {
	Nearest,
	Down,
	Up,
}

impl Rounding {
	pub fn to_bias(self, magnitude: u128) -> (u128, u128) {
		match self {
			Rounding::Nearest => (0, 0),
			Rounding::Down => (0, magnitude),
			Rounding::Up => (magnitude, 0),
		}
	}
}

pub fn round_to_rational((n, d): (U256, U256), rounding: Rounding) -> (u128, u128) {
	let shift = n.bits().max(d.bits()).saturating_sub(128);
	let (n, d) = if shift > 0 {
		let min_n = if n.is_zero() { 0 } else { 1 };
		let (bias_n, bias_d) = rounding.to_bias(1);
		let shifted_n = (n >> shift).low_u128();
		let shifted_d = (d >> shift).low_u128();
		(
			shifted_n.saturating_add(bias_n).max(min_n),
			shifted_d.saturating_add(bias_d).max(1),
		)
	} else {
		(n.low_u128(), d.low_u128())
	};
	(n, d)
}

pub fn round_u512_to_rational((n, d): (U512, U512), rounding: Rounding) -> (u128, u128) {
	let shift = n.bits().max(d.bits()).saturating_sub(128);
	let (n, d) = if shift > 0 {
		let min_n = if n.is_zero() { 0 } else { 1 };
		let (bias_n, bias_d) = rounding.to_bias(1);
		let shifted_n = (n >> shift).low_u128();
		let shifted_d = (d >> shift).low_u128();
		(
			shifted_n.saturating_add(bias_n).max(min_n),
			shifted_d.saturating_add(bias_d).max(1),
		)
	} else {
		(n.low_u128(), d.low_u128())
	};
	(n, d)
}

#[test]
fn round_to_rational_should_work() {
	let res = round_to_rational((U256::from(1), U256::from(1)), Rounding::Nearest);
	let expected: (u128, u128) = (1, 1);
	assert_eq!(res, expected);

	let res = round_to_rational((U256::MAX, U256::MAX), Rounding::Nearest);
	let expected = (u128::MAX, u128::MAX);
	assert_eq!(res, expected);

	let res = round_to_rational((U256::MAX, U256::from(1)), Rounding::Nearest);
	let expected = (u128::MAX, 1u128);
	assert_eq!(res, expected);

	let res = round_to_rational((U256::from(1), U256::MAX), Rounding::Nearest);
	let expected = (1u128, u128::MAX);
	assert_eq!(res, expected);
}
