#![cfg_attr(not(feature = "std"), no_std)]

pub mod omni;
#[cfg(test)]
mod tests;

#[macro_export]
macro_rules! rational_to_f64 {
	($x:expr, $y:expr) => {
		($x as f64) / ($y as f64)
		//FixedU128::from_rational($x, $y).to_float()
	};
}
#[macro_export]
macro_rules! to_f64_by_decimals {
	($x:expr, $y:expr) => {
		($x as f64) / (10u128.pow($y as u32) as f64)
		//FixedU128::from_rational($x, 10u128.pow($y as u32)).to_float()
	};
}
