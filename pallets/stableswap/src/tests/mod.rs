mod add_liquidity;
mod amplification;
mod creation;
mod invariants;
pub(crate) mod mock;
mod remove_liquidity;
mod trades;
mod update_pool;

#[macro_export]
macro_rules! to_precision {
	($e:expr, $f:expr) => {
		$e * 10u128.pow($f as u32)
	};
}
