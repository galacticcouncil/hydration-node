use crate::assert_approx_eq;
use crate::omnipool::types::{AssetReserveState, Position};
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
	ONE / 10..10000 * ONE
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

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_update_invariants_no_fees(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount()
	) {
		let result = calculate_sell_state_changes(&asset_in, &asset_out, amount,
			Permill::from_percent(0),
			Permill::from_percent(0),
			Permill::from_percent(0),
			None,
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
	fn sell_hub_update_invariants_no_fees(asset_out in asset_state(),
		amount in trade_amount(),
	) {
		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			None,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_hub_update_invariants_no_fees_extreme(asset_out in high_asset_state(),
		amount in trade_amount(),
	) {
		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			None,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_hub_update_invariants_with_fees(asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
	) {
		let result = calculate_sell_hub_state_changes(&asset_out, amount,
			asset_fee,
			None,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  None, "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_hub_update_invariants_no_fees(asset_out in asset_state(),
		amount in trade_amount(),
	) {
		let result = calculate_buy_for_hub_asset_state_changes(&asset_out, amount,
			Permill::from_percent(0),
			None,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  Some(FixedU128::from((TOLERANCE, ONE))), "Sell update invariant - token out");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_hub_update_invariants_with_fees(asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
	) {
		let result = calculate_buy_for_hub_asset_state_changes(&asset_out, amount,
			asset_fee,
			None,
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let asset_out_state = asset_out.clone();
		let asset_out_state = asset_out_state.delta_update(&state_changes.asset).unwrap();
		assert_asset_invariant(&asset_out, &asset_out_state,  None, "Sell update invariant - token out");
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
		Permill::from_percent(0),
		None,
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
			Permill::from_percent(0),
			None,
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
	) {
		let result = calculate_add_liquidity_state_changes(&asset,
			amount,
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
	) {
		let result = calculate_remove_liquidity_state_changes(&asset,
			position.amount,
			&position,
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

mod slip_fee_level {
	use super::*;
	use crate::omnipool::types::{HubTradeSlipFees, SignedBalance, TradeSlipFees};
	use sp_arithmetic::Percent;

	/// The buy-side inverses round up by one unit; same allowance as `assert_buy_round_trip`.
	const INVERSE_TOLERANCE: Balance = 5;

	fn level() -> impl Strategy<Value = Percent> {
		(0..=100u8).prop_map(Percent::from_percent)
	}

	fn nonzero_level() -> impl Strategy<Value = Percent> {
		(1..=100u8).prop_map(Percent::from_percent)
	}

	fn cap() -> impl Strategy<Value = Permill> {
		(1..=1_000_000u32).prop_map(Permill::from_parts)
	}

	/// Prior flow of either sign, up to half the smallest reserve the strategies produce.
	fn prior_delta() -> impl Strategy<Value = SignedBalance> {
		(0..BALANCE_RANGE.0 / 2, any::<bool>()).prop_map(|(v, positive)| {
			if positive {
				SignedBalance::Positive(v)
			} else {
				SignedBalance::Negative(v)
			}
		})
	}

	fn flow(amount: Balance, sell_side: bool) -> SignedBalance {
		if sell_side {
			SignedBalance::Negative(amount)
		} else {
			SignedBalance::Positive(amount)
		}
	}

	fn fee_at(
		q0: Balance,
		prior: SignedBalance,
		delta: SignedBalance,
		cap: Permill,
		level: Percent,
	) -> Option<Balance> {
		let (q0, cap) = apply_slip_fee_level(q0, cap, level);
		calculate_slip_fee_amount(q0, prior, delta, cap, delta.abs())
	}

	fn trade_slip(
		in_state: &AssetReserveState<Balance>,
		level_in: Percent,
		out_state: &AssetReserveState<Balance>,
		level_out: Percent,
		cap: Permill,
	) -> TradeSlipFees {
		let (asset_in_hub_reserve, asset_in_max_slip_fee) = apply_slip_fee_level(in_state.hub_reserve, cap, level_in);
		let (asset_out_hub_reserve, asset_out_max_slip_fee) =
			apply_slip_fee_level(out_state.hub_reserve, cap, level_out);
		TradeSlipFees {
			asset_in_hub_reserve,
			asset_in_delta: SignedBalance::zero(),
			asset_out_hub_reserve,
			asset_out_delta: SignedBalance::zero(),
			asset_in_max_slip_fee,
			asset_out_max_slip_fee,
		}
	}

	fn hub_slip(state: &AssetReserveState<Balance>, level: Percent, cap: Permill) -> HubTradeSlipFees {
		let (asset_hub_reserve, max_slip_fee) = apply_slip_fee_level(state.hub_reserve, cap, level);
		HubTradeSlipFees {
			asset_hub_reserve,
			asset_delta: SignedBalance::zero(),
			max_slip_fee,
		}
	}

	proptest! {
		#![proptest_config(ProptestConfig::with_cases(1000))]

		#[test]
		fn apply_slip_fee_level_should_not_panic_when_inputs_are_arbitrary(
			q0 in any::<u128>(),
			cap in (0..=1_000_000u32).prop_map(Permill::from_parts),
			level in level(),
		) {
			let (_, returned_cap) = apply_slip_fee_level(q0, cap, level);
			if level.is_zero() {
				prop_assert_eq!(apply_slip_fee_level(q0, cap, level), (q0, Permill::zero()));
			} else {
				prop_assert_eq!(returned_cap, cap);
			}
		}

		#[test]
		fn apply_slip_fee_level_should_never_shrink_reserve(
			q0 in any::<u128>(),
			cap in cap(),
			level in level(),
		) {
			let (reserve, _) = apply_slip_fee_level(q0, cap, level);
			prop_assert!(reserve >= q0);
			if level == Percent::one() {
				prop_assert_eq!(reserve, q0);
			}
		}

		#[test]
		fn calculate_slip_fee_amount_should_not_grow_when_level_is_lowered(
			state in asset_state(),
			amount in trade_amount(),
			prior in prior_delta(),
			sell_side in any::<bool>(),
			cap in cap(),
			a in nonzero_level(),
			b in nonzero_level(),
		) {
			let (low, high) = if a <= b { (a, b) } else { (b, a) };
			let delta = flow(amount, sell_side);
			if let Some(fee_high) = fee_at(state.hub_reserve, prior, delta, cap, high) {
				let fee_low = fee_at(state.hub_reserve, prior, delta, cap, low);
				prop_assert!(fee_low.is_some());
				prop_assert!(fee_low.unwrap() <= fee_high, "{:?} > {}", fee_low, fee_high);
			}
		}

		#[test]
		fn calculate_slip_fee_amount_should_return_zero_at_zero_level_when_full_level_succeeds(
			state in asset_state(),
			amount in trade_amount(),
			prior in prior_delta(),
			sell_side in any::<bool>(),
			cap in cap(),
		) {
			let delta = flow(amount, sell_side);
			let full = fee_at(state.hub_reserve, prior, delta, cap, Percent::one());
			let off = fee_at(state.hub_reserve, prior, delta, cap, Percent::zero());
			prop_assert_eq!(full.is_some(), off.is_some());
			if off.is_some() {
				prop_assert_eq!(off, Some(0));
			}
		}

		#[test]
		fn calculate_sell_state_changes_should_not_panic_when_levels_are_arbitrary(
			in_state in asset_state(),
			out_state in asset_state(),
			amount in trade_amount(),
			asset_fee in fee(),
			protocol_fee in fee(),
			cap in cap(),
			level_in in level(),
			level_out in level(),
		) {
			let sell = |level_in, level_out| {
				let slip = trade_slip(&in_state, level_in, &out_state, level_out, cap);
				calculate_sell_state_changes(&in_state, &out_state, amount, asset_fee, protocol_fee, Permill::zero(), Some(&slip))
			};
			let lowered = sell(level_in, level_out);
			if let Some(full) = sell(Percent::one(), Percent::one()) {
				prop_assert!(lowered.is_some());
				prop_assert!(*lowered.unwrap().asset_out.delta_reserve >= *full.asset_out.delta_reserve);
			}
		}

		#[test]
		fn calculate_buy_state_changes_should_not_panic_when_levels_are_arbitrary(
			in_state in asset_state(),
			out_state in asset_state(),
			amount in trade_amount(),
			asset_fee in fee(),
			protocol_fee in fee(),
			cap in cap(),
			level_in in level(),
			level_out in level(),
		) {
			let buy = |level_in, level_out| {
				let slip = trade_slip(&in_state, level_in, &out_state, level_out, cap);
				calculate_buy_state_changes(&in_state, &out_state, amount, asset_fee, protocol_fee, Permill::zero(), Some(&slip))
			};
			let lowered = buy(level_in, level_out);
			if let Some(full) = buy(Percent::one(), Percent::one()) {
				prop_assert!(lowered.is_some());
				prop_assert!(*lowered.unwrap().asset_in.delta_reserve <= *full.asset_in.delta_reserve + INVERSE_TOLERANCE);
			}
		}

		#[test]
		fn calculate_sell_hub_state_changes_should_not_panic_when_level_is_arbitrary(
			out_state in asset_state(),
			amount in trade_amount(),
			asset_fee in fee(),
			cap in cap(),
			level in level(),
		) {
			let sell = |level| calculate_sell_hub_state_changes(&out_state, amount, asset_fee, Some(&hub_slip(&out_state, level, cap)));
			let lowered = sell(level);
			if let Some(full) = sell(Percent::one()) {
				prop_assert!(lowered.is_some());
				prop_assert!(*lowered.unwrap().asset.delta_reserve >= *full.asset.delta_reserve);
			}
		}

		#[test]
		fn calculate_buy_for_hub_asset_state_changes_should_not_panic_when_level_is_arbitrary(
			out_state in asset_state(),
			amount in trade_amount(),
			asset_fee in fee(),
			cap in cap(),
			level in level(),
		) {
			let buy = |level| calculate_buy_for_hub_asset_state_changes(&out_state, amount, asset_fee, Some(&hub_slip(&out_state, level, cap)));
			let lowered = buy(level);
			if let Some(full) = buy(Percent::one()) {
				prop_assert!(lowered.is_some());
				prop_assert!(*lowered.unwrap().asset.delta_hub_reserve <= *full.asset.delta_hub_reserve + INVERSE_TOLERANCE);
			}
		}
	}
}
