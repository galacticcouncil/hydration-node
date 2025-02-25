mod data;
mod problem;
#[cfg(test)]
mod tests;
pub mod types;
pub mod v3;

const LOG_TARGET: &str = "ice-old-solver";

#[macro_export]
macro_rules! to_f64_by_decimals {
	($x:expr, $y:expr) => {
		($x as f64) / (10u128.pow($y as u32) as f64)
	};
}
