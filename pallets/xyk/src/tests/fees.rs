pub use super::mock::*;
use crate::Error;
use frame_support::assert_noop;

#[test]
fn fee_calculation() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(XYK::calculate_fee(100_000), Ok(200));
		assert_eq!(XYK::calculate_fee(10_000), Ok(20));
	});
	ExtBuilder::default()
		.with_exchange_fee((10, 1000))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100_000), Ok(1_000));
			assert_eq!(XYK::calculate_fee(10_000), Ok(100));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 0))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100000), Ok(0));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 1))
		.build()
		.execute_with(|| {
			assert_noop!(XYK::calculate_fee(u128::MAX), Error::<Test>::FeeAmountInvalid);
		});
}
