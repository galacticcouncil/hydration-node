use crate::{SimpleImbalance, Tradability};

#[test]
fn tradability_should_allow_all_when_default() {
	let default_tradability = Tradability::default();

	assert!(default_tradability.contains(Tradability::BUY));
	assert!(default_tradability.contains(Tradability::SELL));
	assert!(default_tradability.contains(Tradability::ADD_LIQUIDITY));
	assert!(default_tradability.contains(Tradability::REMOVE_LIQUIDITY));
}

#[test]
fn simple_imbalance_addition_works() {
	assert_eq!(
		SimpleImbalance {
			value: 100,
			negative: false
		} + 200,
		Some(SimpleImbalance {
			value: 300,
			negative: false
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 100,
			negative: true
		} + 200,
		Some(SimpleImbalance {
			value: 100,
			negative: false
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 500,
			negative: true
		} + 200,
		Some(SimpleImbalance {
			value: 300,
			negative: true
		})
	);

	assert_eq!(
		SimpleImbalance {
			value: 500,
			negative: true
		} + 500,
		Some(SimpleImbalance {
			value: 0,
			negative: true
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 0,
			negative: true
		} + 500,
		Some(SimpleImbalance {
			value: 500,
			negative: false
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 0,
			negative: false
		} + 500,
		Some(SimpleImbalance {
			value: 500,
			negative: false
		})
	);

	assert_eq!(
		SimpleImbalance {
			value: 1u128,
			negative: true
		} + u128::MAX,
		Some(SimpleImbalance {
			value: u128::MAX - 1,
			negative: false
		})
	);

	assert_eq!(
		SimpleImbalance {
			value: u128::MAX,
			negative: false
		} + 1,
		None
	);
	assert_eq!(
		SimpleImbalance {
			value: 1u128,
			negative: false
		} + u128::MAX,
		None
	);
}

#[test]
fn simple_imbalance_subtraction_works() {
	assert_eq!(
		SimpleImbalance {
			value: 200,
			negative: false
		} - 300,
		Some(SimpleImbalance {
			value: 100,
			negative: true
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 200,
			negative: true
		} - 300,
		Some(SimpleImbalance {
			value: 500,
			negative: true
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 300,
			negative: false
		} - 300,
		Some(SimpleImbalance {
			value: 0,
			negative: true,
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 0,
			negative: false
		} - 300,
		Some(SimpleImbalance {
			value: 300,
			negative: true
		})
	);
	assert_eq!(
		SimpleImbalance {
			value: 0,
			negative: true
		} - 300,
		Some(SimpleImbalance {
			value: 300,
			negative: true
		})
	);

	assert_eq!(
		SimpleImbalance {
			value: 1u128,
			negative: false
		} - u128::MAX,
		Some(SimpleImbalance {
			value: u128::MAX - 1,
			negative: true
		})
	);

	assert_eq!(
		SimpleImbalance {
			value: u128::MAX,
			negative: true
		} - 1,
		None
	);
	assert_eq!(
		SimpleImbalance {
			value: 1u128,
			negative: true
		} - u128::MAX,
		None
	);
}
