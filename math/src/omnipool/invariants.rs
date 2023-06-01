use crate::assert_approx_eq;
use crate::omnipool::types::{AssetReserveState, BalanceUpdate, Position, I129};
use crate::omnipool::*;
use crate::to_balance;
use crate::types::Balance;
use crate::MathError::Overflow;
use primitive_types::U256;
use proptest::prelude::*;
use sp_arithmetic::{traits::Zero, FixedPointNumber, FixedU128, Permill};

pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000;

#[macro_export]
macro_rules! assert_eq_approx_ordered {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		if $x < $y {
			panic!($r);
		}
		let diff = to_balance!($x - $y).unwrap();
		let diff_percent = FixedU128::from((diff, to_balance!($y).unwrap()));
		let fixed_tolerance = FixedU128::from((TOLERANCE, ONE));
		if diff_percent > fixed_tolerance {
			panic!("\n{} not equal\n left: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);
const HIGH_BALANCE_RANGE: (Balance, Balance) = (900_000_000_000 * ONE, 900_000_000_001 * ONE);

fn asset_state() -> impl Strategy<Value = AssetReserveState<Balance>> {
	(
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
	)
		.prop_map(|(reserve, hub_reserve, shares, protocol_shares)| AssetReserveState {
			reserve,
			hub_reserve,
			shares,
			protocol_shares,
		})
}

fn high_asset_state() -> impl Strategy<Value = AssetReserveState<Balance>> {
	(
		HIGH_BALANCE_RANGE.0..HIGH_BALANCE_RANGE.1,
		HIGH_BALANCE_RANGE.0..HIGH_BALANCE_RANGE.1,
		HIGH_BALANCE_RANGE.0..HIGH_BALANCE_RANGE.1,
		HIGH_BALANCE_RANGE.0..HIGH_BALANCE_RANGE.1,
	)
		.prop_map(|(reserve, hub_reserve, shares, protocol_shares)| AssetReserveState {
			reserve,
			hub_reserve,
			shares,
			protocol_shares,
		})
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1_000_000_000..10000 * ONE
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

fn fee() -> impl Strategy<Value = Permill> {
	(1u32..5u32, prop_oneof![Just(1000u32), Just(10000u32), Just(100_000u32)])
		.prop_map(|(n, d)| Permill::from_rational(n, d))
}

fn position() -> impl Strategy<Value = Position<Balance>> {
	(trade_amount(), price()).prop_map(|(amount, price)| Position {
		amount,
		shares: amount,
		price: (price.into_inner(), 1_000_000_000_000_000_000),
	})
}

fn some_imbalance() -> impl Strategy<Value = I129<Balance>> {
	(0..10000 * ONE).prop_map(|value| I129 { value, negative: true })
}

fn high_imbalance() -> impl Strategy<Value = I129<Balance>> {
	(800_000_000_000 * ONE..800_000_000_001 * ONE).prop_map(|value| I129 { value, negative: true })
}

fn assert_asset_invariant(
	old_state: &AssetReserveState<Balance>,
	new_state: &AssetReserveState<Balance>,
	max_tolerance: Option<FixedU128>,
	desc: &str,
) {
	let new_s = U256::from(new_state.reserve) * U256::from(new_state.hub_reserve);
	let s1 = new_s.integer_sqrt();

	let old_s = U256::from(old_state.reserve) * U256::from(old_state.hub_reserve);
	let s2 = old_s.integer_sqrt();

	assert!(new_s >= old_s, "Invariant decreased for {desc}");

	if let Some(tolerance) = max_tolerance {
		let s1_u128 = Balance::try_from(s1).unwrap();
		let s2_u128 = Balance::try_from(s2).unwrap();

		let invariant = FixedU128::from((s1_u128, ONE)) / FixedU128::from((s2_u128, ONE));
		assert_approx_eq!(invariant, FixedU128::from(1u128), tolerance, desc);
	}
}

fn assert_imbalance_update(
	old_imbalance: I129<Balance>,
	new_imbalance: I129<Balance>,
	old_hub_reserve: Balance,
	new_hub_reserve: Balance,
	desc: &str,
) {
	let q = U256::from(old_hub_reserve);
	let q_plus = U256::from(new_hub_reserve);
	let l = U256::from(old_imbalance.value);
	let l_plus = U256::from(new_imbalance.value);

	let left = q.checked_mul(q.checked_sub(l).unwrap()).unwrap();
	let right = q_plus.checked_mul(q_plus.checked_sub(l_plus).unwrap()).unwrap();

	assert!(left >= right, "{}", desc);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_update_invariants_no_fees(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount()
	) {
		let result = calculate_sell_state_changes(&asset_in, &asset_out, amount,
			Permill::from_percent(0),
			Permill::from_percent(0),
			Balance::default()
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_in_state = asset_in.clone();
		let asset_in_state = asset_in_state.delta_update(&state_changes.asset_in).unwrap();

		assert_asset_invariant(&asset_in, &asset_in_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token in");

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset_out).unwrap();

		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_update_invariants_with_fees(asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * BALANCE_RANGE.1;

		let result = calculate_sell_state_changes(&asset_in, &asset_out, amount,
			asset_fee,
			protocol_fee,
			imbalance.value,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_in_state = asset_in.clone();
		let asset_in_state = asset_in_state.delta_update(&state_changes.asset_in).unwrap();
		assert_asset_invariant(&asset_in, &asset_in_state,  None, "Sell update invariant - token in");

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset_out).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  None, "Sell update invariant - token out");

		let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount)).unwrap()
				).unwrap();

		let q_plus = match delta_hub_asset {
			BalanceUpdate::Increase(v) => total_hub_reserve.checked_add(v).unwrap(),
			BalanceUpdate::Decrease(v) => total_hub_reserve.checked_sub(v).unwrap(),
		};

		let imbalance_plus = match state_changes.delta_imbalance {
			BalanceUpdate::Increase(v) => imbalance.value.checked_sub(v).unwrap(),
			BalanceUpdate::Decrease(v) => imbalance.value.checked_add(v).unwrap(),
		};

		let left = total_hub_reserve - imbalance.value;
		let right = q_plus - imbalance_plus;

		assert_eq!(left, right);
		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance_plus, negative: true},
			total_hub_reserve,
			q_plus,
			"sell imbalance invariant failed" );

	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_hub_update_invariants_no_fees(asset_out in asset_state(),
		amount in trade_amount(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * ONE * BALANCE_RANGE.1;

		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			imbalance,
			total_hub_reserve,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();

		let new_total_hub_reserve = total_hub_reserve + *state_changes.asset.delta_hub_reserve;

		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance.value + *state_changes.delta_imbalance, negative: true},
			total_hub_reserve,
			new_total_hub_reserve,
			"sell hub imbalance invariant failed" );

		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_hub_update_invariants_no_fees_extreme(asset_out in high_asset_state(),
		amount in trade_amount(),
		imbalance in high_imbalance(),
	) {
		let total_hub_reserve = 100 * ONE + asset_out.hub_reserve;

		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			imbalance,
			total_hub_reserve,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();

		let new_total_hub_reserve = total_hub_reserve + *state_changes.asset.delta_hub_reserve;

		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance.value + *state_changes.delta_imbalance, negative: true},
			total_hub_reserve,
			new_total_hub_reserve,
			"sell hub imbalance invariant failed" );

		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_hub_update_invariants_with_fees(asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * ONE + asset_out.hub_reserve;

		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			asset_fee,
			imbalance,
			total_hub_reserve,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();

		let new_total_hub_reserve = total_hub_reserve + *state_changes.asset.delta_hub_reserve;

		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance.value + *state_changes.delta_imbalance, negative: true},
			total_hub_reserve,
			new_total_hub_reserve,
			"sell hub imbalance invariant failed" );

		assert_asset_invariant(&asset_out, &asset_out_state,  None, "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_hub_update_invariants_no_fees(asset_out in asset_state(),
		amount in trade_amount(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * ONE + asset_out.hub_reserve;

		let result = calculate_buy_for_hub_asset_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			imbalance,
			total_hub_reserve,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();

		let new_total_hub_reserve = total_hub_reserve + *state_changes.asset.delta_hub_reserve;

		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance.value + *state_changes.delta_imbalance, negative: true},
			total_hub_reserve,
			new_total_hub_reserve,
			"buy hub imbalance invariant failed" );

	   assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_hub_update_invariants_with_fees(asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * ONE + asset_out.hub_reserve;

		let result = calculate_buy_for_hub_asset_state_changes(&asset_out, amount,
			asset_fee,
			imbalance,
			total_hub_reserve,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();

		let new_total_hub_reserve = total_hub_reserve + *state_changes.asset.delta_hub_reserve;

		assert_imbalance_update(
			imbalance,
			I129::<Balance>{value: imbalance.value + *state_changes.delta_imbalance, negative: true},
			total_hub_reserve,
			new_total_hub_reserve,
			"buy hub imbalance invariant failed" );

		assert_asset_invariant(&asset_out, &asset_out_state,  None, "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_update_invariants_with_fees(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let total_hub_reserve = 100 * BALANCE_RANGE.1;

		let result = calculate_buy_state_changes(&asset_in, &asset_out, amount,
			asset_fee,
			protocol_fee,
			imbalance.value,
		);

		// ignore the invalid result
		if let Some(state_changes) = result {
			let asset_in_state = asset_in.clone();
			let asset_in_state = asset_in_state.delta_update(&state_changes.asset_in).unwrap();
			assert_asset_invariant(&asset_in, &asset_in_state,  None, "Buy update invariant - token in");

			let asset_out_state = asset_out.clone();
			let asset_out_state = asset_out_state.delta_update(&state_changes.asset_out).unwrap();
			assert_asset_invariant(&asset_out, &asset_out_state,  None, "Buy update invariant - token out");

			let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount)).unwrap()
				).unwrap();

			let q_plus = match delta_hub_asset {
				BalanceUpdate::Increase(v) => total_hub_reserve.checked_add(v).unwrap(),
				BalanceUpdate::Decrease(v) => total_hub_reserve.checked_sub(v).unwrap(),
			};

			let imbalance_plus = match state_changes.delta_imbalance {
				BalanceUpdate::Increase(v) => imbalance.value.checked_sub(v).unwrap(),
				BalanceUpdate::Decrease(v) => imbalance.value.checked_add(v).unwrap(),
			};

			let left = total_hub_reserve - imbalance.value;
			let right = q_plus - imbalance_plus;

			assert_eq!(left, right);

			assert_imbalance_update(
				imbalance,
				I129::<Balance>{value: imbalance_plus, negative: true},
				total_hub_reserve,
				q_plus,
				"buy imbalance invariant failed" );
		}
	}
}
#[test]
fn buy_update_invariants_no_fees_case() {
	let asset_in = AssetReserveState {
		reserve: 10_000_000_000_000_000,
		hub_reserve: 10_000_000_000_000_000,
		shares: 10_000_000_000_000_000,
		protocol_shares: 10_000_000_000_000_000,
	};
	let asset_out = AssetReserveState {
		reserve: 10_000_000_000_000_000,
		hub_reserve: 89_999_999_999_999_991,
		shares: 10_000_000_000_000_000,
		protocol_shares: 10_000_000_000_000_000,
	};
	let amount = 1_000_000_000_000_000;

	let result = calculate_buy_state_changes(
		&asset_in,
		&asset_out,
		amount,
		Permill::from_percent(0),
		Permill::from_percent(0),
		Balance::default(),
	);

	assert!(result.is_none()); // This fails because of not enough asset out in pool out
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_update_invariants_no_fees(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount()
	) {
		let result = calculate_buy_state_changes(&asset_in, &asset_out, amount,
			Permill::from_percent(0),
			Permill::from_percent(0),
			Balance::default()
		);

		// perform assertion only when result is valid
		if let Some(state_changes) = result {
			let asset_in_state = asset_in.clone();
			let asset_in_state = asset_in_state.delta_update(&state_changes.asset_in).unwrap();
			assert_asset_invariant(&asset_in, &asset_in_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Buy update invariant - token in");

			let asset_out_state = asset_out.clone();
			let asset_out_state = asset_out_state.delta_update(&state_changes.asset_out).unwrap();
			assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Buy update invariant - token out");
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn price_should_not_change_when_liquidity_added(asset in asset_state(),
		amount in trade_amount(),
		imbalance in some_imbalance(),
	) {
		let result = calculate_add_liquidity_state_changes(&asset,
			amount,
			imbalance,
			100 * ONE,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let new_asset_state= asset.clone();
		let new_asset_state = new_asset_state.delta_update(&state_changes.asset).unwrap();

		// Price should not change
		assert_approx_eq!(asset.price().unwrap(),
			new_asset_state.price().unwrap(),
			FixedU128::from_float(0.0000000001),
			"Price has changed after add liquidity");

		let shares = U256::from(asset.shares);
		let shares_updated = U256::from(new_asset_state.shares);
		let reserve = U256::from(asset.reserve);
		let reserve_updated = U256::from(new_asset_state.reserve);

		// Shares should be approximately correct
		// Rounding errors in share calculation should favor pool
		// R^+ * S ~= R * S^+
		assert_eq_approx_ordered!(reserve_updated.checked_mul(shares).unwrap(), reserve.checked_mul(shares_updated).unwrap(), TOLERANCE,
			"Invariant is not correct after add liquidity");

	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn price_should_not_change_when_liquidity_removed(asset in asset_state(),
		position in position(),
		imbalance in some_imbalance(),
	) {
		let result = calculate_remove_liquidity_state_changes(&asset,
			position.amount,
			&position,
			imbalance,
			100 * ONE,
			FixedU128::zero(),
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let new_asset_state= asset.clone();
		let new_asset_state = new_asset_state.delta_update(&state_changes.asset).unwrap();

		assert_ne!(new_asset_state.reserve, asset.reserve);

		// Price should not change
		assert_approx_eq!(asset.price().unwrap(),
			new_asset_state.price().unwrap(),
			FixedU128::from_float(0.0000000001),
			"Price has changed after remove liquidity");

		let shares  = U256::from(asset.shares);
		let shares_updated = U256::from(new_asset_state.shares);
		let reserve = U256::from(asset.reserve);
		let reserve_updated = U256::from(new_asset_state.reserve);

		// Shares should be approximately correct
		// Rounding errors in share calculation should favor pool
		// R^+ * S ~= R * S^+
		assert_eq_approx_ordered!(reserve_updated.checked_mul(shares).unwrap(), reserve.checked_mul(shares_updated).unwrap(), TOLERANCE,
			"Invariant is not correct after remove liquidity");

		let delta_b = U256::from(new_asset_state.protocol_shares) - U256::from(asset.protocol_shares);
		let price_x_r = U256::from(position.price().unwrap().checked_mul_int(asset.reserve).unwrap());
		let hub_reserve = U256::from(asset.hub_reserve);
		let position_shares = U256::from(position.shares);

		// Rounding errors in protocol owned share calculation should favor pool
		// dB (pa R + Q) >= sa (pa R - Q)
		if delta_b > U256::from(0_u128) {
			assert_eq_approx_ordered!(delta_b * (price_x_r + hub_reserve), position_shares * (price_x_r - hub_reserve), TOLERANCE,
				"Protocol owned share calculation incorrect in remove liquidity");
		}
		// Rounding errors in LRNA dispersal should favor pool
		// dq * [(Q + pa R) * S / (Q - pa R)] <= Q * s
		else {
			let dq = U256::from(state_changes.lp_hub_amount);
			assert_eq_approx_ordered!(hub_reserve * position_shares, dq * (((hub_reserve + price_x_r) * shares) / (hub_reserve - price_x_r)), TOLERANCE,
				"Protocol owned share calculation incorrect in remove liquidity");
		}
	}
}
