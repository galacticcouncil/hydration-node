use crate::stableswap::calculate_amplification;

#[test]
fn calculate_amplification_should_short_circuit_when_future_and_initial_amp_are_equal() {
	let result = calculate_amplification(10, 10, 0, 100, 50);
	assert_eq!(result, 10);
}

#[test]
fn calculate_amplification_should_short_circuit_when_current_timestamp_is_greater_than_future_timestamp() {
	let result = calculate_amplification(10, 20, 0, 100, 150);
	assert_eq!(result, 20);
}

#[test]
fn test_calculate_amplification_increase_amplification() {
	let result = calculate_amplification(10, 20, 0, 100, 50);
	assert_eq!(result, 15);
}

#[test]
fn test_calculate_amplification_decrease_amplification() {
	let result = calculate_amplification(20, 10, 0, 100, 50);
	assert_eq!(result, 15);
}

#[test]
fn test_calculate_amplification_step_increase_amplification() {
	for idx in 0..1000 {
		let result = calculate_amplification(2000, 5000, 0, 1000, idx);
		assert_eq!(result, 2000 + idx * 3);
	}
}

#[test]
fn test_calculate_amplification_step_decrease_amplification() {
	for idx in 0..1000 {
		let result = calculate_amplification(5000, 2000, 0, 1000, idx);
		assert_eq!(result, 5000 - idx * 3);
	}
}
