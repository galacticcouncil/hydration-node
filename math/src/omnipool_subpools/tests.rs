use crate::omnipool::types::Position;
use crate::omnipool_subpools::convert_position;
use crate::omnipool_subpools::types::MigrationDetails;
use crate::types::Balance;

const ONE: Balance = 1_000_000_000_000;

#[test]
fn convert_position_should_work() {
	let position = Position::<Balance> {
		amount: 1_000 * ONE,
		shares: 2_000 * ONE,
		price: (2_000_000_000 * ONE, 2_000 * ONE),
	};

	let details = MigrationDetails {
		price: (1_000 * ONE, 1_000_000_000 * ONE),
		shares: 10_000 * ONE,
		hub_reserve: 5_000 * ONE,
		share_tokens: 3_000 * ONE,
	};

	let updated_position = convert_position(position, details);

	assert_eq!(
		updated_position.unwrap(),
		Position {
			amount: 300 * ONE,
			shares: 1_000 * ONE,
			price: (244140625000000000000000000000000000000, 244140625000000000000000000),
		}
	);
}
