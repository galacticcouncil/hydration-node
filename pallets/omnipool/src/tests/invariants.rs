use crate::math::calculate_sell_state_changes;
use crate::mock::*;
use crate::{AssetState, FixedU128, SimpleImbalance};
use proptest::prelude::*;

use primitive_types::U256;

pub const ONE: Balance = 1_000_000_000_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 100_000_000 * ONE);

impl AssetState<Balance> {
	#[cfg(test)]
	pub(super) fn invariant(&self) -> U256 {
		U256::from(self.reserve) * U256::from(self.hub_reserve)
	}
}

fn asset_state() -> impl Strategy<Value = AssetState<Balance>> {
	(
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
	)
		.prop_map(|(reserve, hub_reserve, shares, protocol_shares, tvl)| AssetState {
			reserve,
			hub_reserve,
			shares,
			protocol_shares,
			tvl,
			..Default::default()
		})
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..BALANCE_RANGE.0
}

proptest! {
	#[test]
	fn swap_invariants(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount()
	) {
		let original_invariant = asset_in.invariant();

		let result =  calculate_sell_state_changes::<Test>(&asset_in, &asset_out, amount,
			FixedU128::from(0u128),
			FixedU128::from(0u128),
			&SimpleImbalance::default()
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let mut asset_in_state = asset_in;

		assert!(asset_in_state.delta_update(&state_changes.asset_in).is_some());

		let new_invariant = asset_in_state.invariant();

		assert_eq!(original_invariant, new_invariant);
	}
}
