use crate::types::Balance;
use num_traits::Zero;

pub fn calculate_pool_trade_fee(amount: Balance, fee: (u32, u32)) -> Option<Balance> {
	let numerator = fee.0;
	let denominator = fee.1;

	if denominator.is_zero() || numerator.is_zero() {
		return Some(0);
	}

	if denominator == numerator {
		return Some(amount);
	}

	amount
		.checked_div(denominator as Balance)?
		.checked_mul(numerator as Balance)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fee_calculations_should_work() {
		let default_fee = (2, 1_000);

		assert_eq!(calculate_pool_trade_fee(1_000, default_fee), Some(2));
		assert_eq!(
			calculate_pool_trade_fee(1_000_000_000_000, default_fee),
			Some(2_000_000_000)
		);

		let ten_percent_fee = (1, 10);

		assert_eq!(calculate_pool_trade_fee(1_000, ten_percent_fee), Some(100));
		assert_eq!(
			calculate_pool_trade_fee(1_000_000_000_000, ten_percent_fee),
			Some(100_000_000_000)
		);

		assert_eq!(calculate_pool_trade_fee(1_000, (1, 10)), Some(100));
		assert_eq!(
			calculate_pool_trade_fee(1_000_000_000_000, (1, 10)),
			Some(100_000_000_000)
		);

		let max_amount = Balance::MAX;

		assert_eq!(
			calculate_pool_trade_fee(max_amount, default_fee),
			Some(680564733841876926926749214863536422)
		);
		assert_eq!(
			calculate_pool_trade_fee(max_amount, ten_percent_fee),
			Some(34028236692093846346337460743176821145)
		);

		let max_fee = (1, 1);

		assert_eq!(calculate_pool_trade_fee(max_amount, max_fee), Some(max_amount));
		assert_eq!(calculate_pool_trade_fee(1_000, max_fee), Some(1_000));

		assert_eq!(calculate_pool_trade_fee(max_amount, (1, 1)), Some(max_amount));
		assert_eq!(calculate_pool_trade_fee(1_000, (1, 1)), Some(1_000));

		let zero_amount = 0u128;
		assert_eq!(calculate_pool_trade_fee(zero_amount, default_fee), Some(0));

		let unrealistic_fee = (1, u32::MAX);

		assert_eq!(
			calculate_pool_trade_fee(max_amount, unrealistic_fee),
			Some(79228162532711081671548469249)
		);

		assert_eq!(
			calculate_pool_trade_fee(max_amount, (1, u32::MAX)),
			Some(79228162532711081671548469249)
		);

		let unrealistic_fee = (u32::MAX, 1);

		assert_eq!(calculate_pool_trade_fee(max_amount, unrealistic_fee), None);
		assert_eq!(calculate_pool_trade_fee(max_amount, (u32::MAX, 1)), None);

		let zero_fee = (0, 0);

		assert_eq!(calculate_pool_trade_fee(max_amount, zero_fee), Some(0));
		assert_eq!(calculate_pool_trade_fee(1_000, zero_fee), Some(0));

		assert_eq!(calculate_pool_trade_fee(max_amount, (0, 0)), Some(0));
		assert_eq!(calculate_pool_trade_fee(1_000, (0, 0)), Some(0));

		assert_eq!(calculate_pool_trade_fee(max_amount, (1, 0)), Some(0));
		assert_eq!(calculate_pool_trade_fee(1_000, (0, 1)), Some(0));
	}
}
