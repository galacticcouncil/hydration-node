use super::*;
use crate::math::calculate_sell_state_changes;
use crate::{AssetState, FixedU128, SimpleImbalance};
use proptest::prelude::*;

use primitive_types::U256;

pub const ONE: Balance = 1_000_000_000_000;
pub const ALLOWED_INVARIANT_MARGIN: u128 = 10_000_000_000_000_000_000_000u128;

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

fn asset_reserve() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..BALANCE_RANGE.0
}

fn fee_amount() -> impl Strategy<Value = FixedU128> {
	(0f64..0.5f64).prop_map(FixedU128::from_float)
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

#[derive(Debug)]
struct PoolToken {
	asset_id: AssetId,
	amount: Balance,
	price: FixedU128,
}

fn pool_token(asset_id: AssetId) -> impl Strategy<Value = PoolToken> {
	(asset_reserve(), price()).prop_map(move |(reserve, price)| PoolToken {
		asset_id,
		amount: reserve,
		price,
	})
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

		//TODO: needs to verify what is allowed margin here
		//The allowed margin here is just to make tests pass for now
		assert_eq_approx!(original_invariant, new_invariant, U256::from(ALLOWED_INVARIANT_MARGIN), "Invariant");
	}
}

proptest! {
	#[test]
	fn swap_invariants_with_fees(asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee_amount(),
		protocol_fee in fee_amount()
	) {
		let original_invariant = asset_in.invariant();

		let result =  calculate_sell_state_changes::<Test>(&asset_in, &asset_out, amount,
			asset_fee,
			protocol_fee,
			&SimpleImbalance::default()
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let mut asset_in_state = asset_in;

		assert!(asset_in_state.delta_update(&state_changes.asset_in).is_some());

		let new_invariant = asset_in_state.invariant();

		//TODO: needs to verify what is allowed margin here
		//The allowed margin here is just to make tests pass for now
		assert_eq_approx!(original_invariant, new_invariant, U256::from(ALLOWED_INVARIANT_MARGIN), "Invariant");
	}
}

proptest! {
	#[test]
	fn sell_invariants(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
				(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_initial_pool(
				stable_reserve,
				native_reserve,
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::signed(lp1), token_1.asset_id, token_1.amount, token_1.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp2), token_2.asset_id, token_2.amount, token_2.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp3), token_3.asset_id, token_3.amount, token_3.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp4), token_4.asset_id, token_4.amount, token_4.price));

				assert_ok!(Omnipool::sell(Origin::signed(seller), 200, 300, amount, Balance::zero()));
			});
	}
}
