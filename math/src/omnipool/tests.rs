use crate::omnipool::types::{AssetReserveState, BalanceUpdate, Position, TradeFee, I129};
use crate::omnipool::{
	calculate_add_liquidity_state_changes, calculate_buy_for_hub_asset_state_changes, calculate_buy_state_changes,
	calculate_cap_difference, calculate_delta_imbalance, calculate_fee_amount_for_buy,
	calculate_remove_liquidity_state_changes, calculate_sell_hub_state_changes, calculate_sell_state_changes,
	calculate_tvl_cap_difference, calculate_withdrawal_fee, verify_asset_cap,
};
use crate::types::Balance;
use num_traits::{One, Zero};
use sp_arithmetic::{FixedU128, Permill};
use std::str::FromStr;

const UNIT: Balance = 1_000_000_000_000;

#[test]
fn calculate_sell_should_work_when_correct_input_provided() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(0);
	let protocol_fee = Permill::from_percent(0);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset_in.delta_hub_reserve,
		BalanceUpdate::Decrease(5714285714285u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2666666666666u128)
	);
	assert_eq!(
		state_changes.asset_out.delta_hub_reserve,
		BalanceUpdate::Increase(5714285714285u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(0u128));
	assert_eq!(state_changes.hdx_hub_amount, 0u128);
	assert_eq!(state_changes.fee, TradeFee::default());
}

#[test]
fn calculate_sell_should_return_correct_when_protocol_fee_is_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(0);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();
	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2666666666666u128 - state_changes.fee.asset_fee)
	);
	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 26666666667,
			protocol_fee: 0,
		}
	);
}

#[test]
fn calculate_sell_should_return_correct_when_protocol_fee_is_not_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(1);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());
	let state_changes = state_changes.unwrap();
	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 26541554960,
			protocol_fee: 57142857142,
		}
	);
}

#[test]
fn calculate_sell_with_fees_should_work_when_correct_input_provided() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(1);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset_in.delta_hub_reserve,
		BalanceUpdate::Decrease(5714285714285u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2627613941018u128)
	);
	assert_eq!(
		state_changes.asset_out.delta_hub_reserve,
		BalanceUpdate::Increase(5657142857143u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(57142857142u128));
	assert_eq!(state_changes.hdx_hub_amount, 0u128);

	// Verify if fee + delta amount == delta with fee
	let f = 57142857142u128 + 5657142857143u128;
	let no_fees_amount: Balance = *state_changes.asset_in.delta_hub_reserve;
	assert_eq!(f, no_fees_amount);
}

#[test]
fn calculate_sell_hub_asset_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(0);
	let imbalance = I129 {
		value: 2 * UNIT,
		negative: true,
	};
	let total_hub_reserve = 40 * UNIT;

	let state_changes =
		calculate_sell_hub_state_changes(&asset_state, amount_to_sell, asset_fee, imbalance, total_hub_reserve);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1666666666666u128)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(amount_to_sell)
	);

	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Decrease(7454545454546));
	assert_eq!(state_changes.fee, TradeFee::default());
}

#[test]
fn calculate_sell_hub_asset_with_fee_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_sell = 4 * UNIT;
	let asset_fee = Permill::from_percent(1);
	let imbalance = I129 {
		value: 2 * UNIT,
		negative: true,
	};
	let total_hub_reserve = 40 * UNIT;

	let state_changes =
		calculate_sell_hub_state_changes(&asset_state, amount_to_sell, asset_fee, imbalance, total_hub_reserve);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1649999999999u128)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(amount_to_sell)
	);

	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Decrease(7454545454546));
	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 16666666667,
			protocol_fee: 0,
		}
	);
}

#[test]
fn calculate_buy_should_work_when_correct_input_provided() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(0);
	let protocol_fee = Permill::from_percent(0);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(666666666668u128)
	);
	assert_eq!(
		state_changes.asset_in.delta_hub_reserve,
		BalanceUpdate::Decrease(1250000000001u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset_out.delta_hub_reserve,
		BalanceUpdate::Increase(1250000000001u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(0u128));
	assert_eq!(state_changes.hdx_hub_amount, 0u128);
}

#[test]
fn calculate_buy_should_return_correct_fee_when_protocol_fee_is_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(0);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(675675675677)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 10101010102,
			protocol_fee: 0,
		}
	)
}

#[test]
fn calculate_buy_should_return_correct_fee_when_protocol_fee_is_non_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(1);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(682966807814)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 10101010102,
			protocol_fee: 12786088735,
		}
	)
}

#[test]
fn calculate_buy_with_fees_should_work_when_correct_input_provided() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(1);
	let protocol_fee = Permill::from_percent(1);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(682966807814u128)
	);
	assert_eq!(
		state_changes.asset_in.delta_hub_reserve,
		BalanceUpdate::Decrease(1278608873546)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset_out.delta_hub_reserve,
		BalanceUpdate::Increase(1265822784811u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(12786088735u128));
	assert_eq!(state_changes.hdx_hub_amount, 0u128);

	// Verify if fee + delta amount == delta with fee
	let f = 1265822784811u128 + 12786088735u128;
	let no_fees_amount: Balance = *state_changes.asset_in.delta_hub_reserve;
	assert_eq!(f, no_fees_amount);
}

#[test]
fn calculate_buy_for_hub_asset_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = 2 * UNIT;
	let asset_fee = Permill::from_percent(0);
	let imbalance = I129 {
		value: 2 * UNIT,
		negative: true,
	};
	let total_hub_reserve = 40 * UNIT;

	let state_changes =
		calculate_buy_for_hub_asset_state_changes(&asset_state, amount_to_buy, asset_fee, imbalance, total_hub_reserve);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(5000000000001u128)
	);

	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Decrease(9222222222224));
	assert_eq!(state_changes.fee, TradeFee::default());
}

#[test]
fn calculate_buy_for_hub_asset_with_fee_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = 2 * UNIT;
	let asset_fee = Permill::from_percent(1);
	let imbalance = I129 {
		value: 2 * UNIT,
		negative: true,
	};
	let total_hub_reserve = 40 * UNIT;

	let state_changes =
		calculate_buy_for_hub_asset_state_changes(&asset_state, amount_to_buy, asset_fee, imbalance, total_hub_reserve);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(5063291139241u128)
	);

	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Decrease(9332954060590));
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(5063291139241)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 20_202_020_203,
			protocol_fee: 0,
		}
	);
}

#[test]
fn calculate_add_liquidity_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_add = 2 * UNIT;
	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let total_hub_reserve = 22 * UNIT;

	let state_changes =
		calculate_add_liquidity_state_changes(&asset_state, amount_to_add, imbalance, total_hub_reserve);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Increase(amount_to_add)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(state_changes.asset.delta_shares, BalanceUpdate::Increase(amount_to_add));

	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Decrease(181818181818u128));

	assert_eq!(state_changes.delta_position_reserve, BalanceUpdate::Increase(0u128),);

	assert_eq!(state_changes.delta_position_shares, BalanceUpdate::Increase(0u128));

	assert_eq!(state_changes.lp_hub_amount, 0u128);
}

#[test]
fn calculate_remove_liquidity_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_remove = 2 * UNIT;

	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let total_hub_reserve = 22 * UNIT;

	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(0.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
		imbalance,
		total_hub_reserve,
		FixedU128::zero(),
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_remove)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Decrease(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset.delta_shares,
		BalanceUpdate::Decrease(amount_to_remove)
	);
	assert_eq!(
		state_changes.asset.delta_protocol_shares,
		BalanceUpdate::Increase(0u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(181818181818u128));

	assert_eq!(
		state_changes.delta_position_reserve,
		BalanceUpdate::Decrease(2000000000000u128)
	);

	assert_eq!(
		state_changes.delta_position_shares,
		BalanceUpdate::Decrease(amount_to_remove)
	);

	assert_eq!(state_changes.lp_hub_amount, 3174887892376u128);
}

#[test]
fn calculate_remove_liquidity_should_work_when_current_price_is_smaller_than_position_price() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_remove = 2 * UNIT;

	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let total_hub_reserve = 22 * UNIT;

	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(2.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
		imbalance,
		total_hub_reserve,
		FixedU128::zero(),
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1891252955082u128)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Decrease(3782505910164u128)
	);
	assert_eq!(
		state_changes.asset.delta_shares,
		BalanceUpdate::Decrease(1891252955082u128)
	);
	assert_eq!(
		state_changes.asset.delta_protocol_shares,
		BalanceUpdate::Increase(108747044918u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(171932086825u128));

	assert_eq!(
		state_changes.delta_position_reserve,
		BalanceUpdate::Decrease(2000000000000u128)
	);

	assert_eq!(
		state_changes.delta_position_shares,
		BalanceUpdate::Decrease(amount_to_remove)
	);

	assert_eq!(state_changes.lp_hub_amount, 0u128);
}

#[test]
fn calculate_delta_imbalance_for_asset_should_work_when_correct_input_provided() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount = 2 * UNIT;
	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let hub_reserve = 11 * UNIT;

	let d = asset_state.hub_reserve * amount / asset_state.reserve;

	let delta_imbalance = calculate_delta_imbalance(d, imbalance, hub_reserve);

	assert!(delta_imbalance.is_some());

	let delta_imbalance = delta_imbalance.unwrap();

	assert_eq!(delta_imbalance, 363636363636u128);
}

#[test]
fn calculate_cap_diff_should_work_correctly() {
	let asset_state = AssetReserveState {
		hub_reserve: 80,
		reserve: 160,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_state_2 = AssetReserveState {
		hub_reserve: 20,
		reserve: 100,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = calculate_cap_difference(&asset_state, 800_000_000_000_000_000, 100);
	assert_eq!(result, Some(0));
	let result = calculate_cap_difference(&asset_state_2, 300_000_000_000_000_000, 100);
	assert_eq!(result, Some(33));

	let asset_state_2 = AssetReserveState {
		hub_reserve: 2218128255986034,
		reserve: 52301491602723449004308,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = calculate_cap_difference(&asset_state_2, 1_000_000_000_000_000_000, 5651225591124720);
	assert_eq!(result, Some(31772950583866634024008));

	let asset_state_2 = AssetReserveState {
		hub_reserve: 1584818376248207,
		reserve: 675534123147791411,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = calculate_cap_difference(&asset_state_2, 100_000_000_000_000_000, 5651225591124720);
	assert_eq!(result, Some(0));
}

#[test]
fn verify_cap_diff_should_work_correctly() {
	let asset_state = AssetReserveState {
		hub_reserve: 80,
		reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = verify_asset_cap(&asset_state, 800_000_000_000_000_000, 20, 100);
	assert_eq!(result, Some(false));

	let asset_state = AssetReserveState {
		hub_reserve: 60,
		reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = verify_asset_cap(&asset_state, 800_000_000_000_000_000, 20, 100);
	assert_eq!(result, Some(true));

	let asset_state = AssetReserveState {
		hub_reserve: 100,
		reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let result = verify_asset_cap(&asset_state, 1_000_000_000_000_000_000, 20, 100);
	assert_eq!(result, Some(true));
}

#[test]
fn calculate_tvl_cap_diff_should_work_correctly() {
	let asset_state = AssetReserveState {
		hub_reserve: 3306347306384663,
		reserve: 67829448624524361905510,
		..Default::default()
	};

	let stable_asset = AssetReserveState {
		hub_reserve: 3306347306384663,
		reserve: 67829448624524361905510,
		..Default::default()
	};

	let tvl_cap: Balance = 222_222_000_000_000_000_000_000;
	let total_hub_resrerve = 11413797633709387;

	let result = calculate_tvl_cap_difference(&asset_state, &stable_asset, tvl_cap, total_hub_resrerve);
	assert_eq!(result, Some(0));
}

#[test]
fn calculate_remove_liquidity_should_apply_correct_fee() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_remove = 2 * UNIT;

	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let total_hub_reserve = 22 * UNIT;

	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(2.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
		imbalance,
		total_hub_reserve,
		FixedU128::from_float(0.01),
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1872340425531u128)
	);
	assert_eq!(
		state_changes.asset.delta_hub_reserve,
		BalanceUpdate::Decrease(3744680851062)
	);
	assert_eq!(
		state_changes.asset.delta_shares,
		BalanceUpdate::Decrease(1891252955082u128)
	);
	assert_eq!(
		state_changes.asset.delta_protocol_shares,
		BalanceUpdate::Increase(108747044918u128)
	);
	assert_eq!(state_changes.delta_imbalance, BalanceUpdate::Increase(170212765957u128));

	assert_eq!(
		state_changes.delta_position_reserve,
		BalanceUpdate::Decrease(2000000000000u128)
	);

	assert_eq!(
		state_changes.delta_position_shares,
		BalanceUpdate::Decrease(amount_to_remove)
	);

	assert_eq!(state_changes.lp_hub_amount, 0u128);
}

#[test]
fn calculate_remove_liquidity_should_apply_fee_to_hub_amount() {
	let asset_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_remove = 2 * UNIT;

	let imbalance = I129 {
		value: UNIT,
		negative: true,
	};
	let total_hub_reserve = 22 * UNIT;

	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(0.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
		imbalance,
		total_hub_reserve,
		FixedU128::from_float(0.01),
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(state_changes.lp_hub_amount, 3143139013452u128);
}

#[test]
fn calculate_withdrawal_fee_should_work_correctly() {
	// Test case 1: Oracle price <= spot price, min fee = 0.001%
	let spot_price = FixedU128::from_str("100000000000000000000").unwrap();
	let oracle_price = FixedU128::from_str("95000000000000000000").unwrap();

	let min_withdrawal_fee = Permill::from_float(0.00001);
	let expected_fee = FixedU128::from_inner(52631578947368421);
	assert_eq!(
		calculate_withdrawal_fee(spot_price, oracle_price, min_withdrawal_fee),
		expected_fee
	);

	// Test case 2: Spot price < oracle price, min fee = 0.1%
	let spot_price = FixedU128::from_str("500000000000000000000").unwrap();
	let oracle_price = FixedU128::from_str("600000000000000000000").unwrap();
	let min_withdrawal_fee = Permill::from_float(0.001);
	let expected_fee = FixedU128::from_inner(166666666666666667);
	assert_eq!(
		calculate_withdrawal_fee(spot_price, oracle_price, min_withdrawal_fee),
		expected_fee
	);

	// Test case 3: Spot price < oracle price, min fee = 17% - should return min fee
	let spot_price = FixedU128::from_str("500000000000000000000").unwrap();
	let oracle_price = FixedU128::from_str("600000000000000000000").unwrap();
	let min_withdrawal_fee = Permill::from_float(0.17);
	assert_eq!(
		calculate_withdrawal_fee(spot_price, oracle_price, min_withdrawal_fee),
		min_withdrawal_fee.into()
	);

	// Test case 4: Oracle price == spot price, min fee = 0.05% - should return min fee
	let spot_price = FixedU128::from_str("200000000000000000000").unwrap();
	let oracle_price = FixedU128::from_str("200000000000000000000").unwrap();
	let min_withdrawal_fee = Permill::from_float(0.05);
	assert_eq!(
		calculate_withdrawal_fee(spot_price, oracle_price, min_withdrawal_fee),
		min_withdrawal_fee.into()
	);

	// Test case 5: Oracle price > spot price, min fee = 1%
	let spot_price = FixedU128::from_str("800000000000000000000").unwrap();
	let oracle_price = FixedU128::from_str("900000000000000000000").unwrap();
	let min_withdrawal_fee = Permill::from_percent(1);
	let expected_fee = FixedU128::from_inner(111111111111111111);
	assert_eq!(
		calculate_withdrawal_fee(spot_price, oracle_price, min_withdrawal_fee),
		expected_fee
	);

	// Test case 6: Oracle price is zero, should return None
	let expected_fee: FixedU128 = Permill::from_percent(1).into();
	assert_eq!(
		calculate_withdrawal_fee(FixedU128::from(100), FixedU128::from(0), Permill::from_percent(1)),
		expected_fee
	);

	// Test case 7: Spot price is zero, should return 100%
	assert_eq!(
		calculate_withdrawal_fee(FixedU128::from(0), FixedU128::from(100), Permill::from_percent(0)),
		FixedU128::one()
	);

	// Test case 8: Both prices are zero, should return None
	let expected_fee: FixedU128 = Permill::from_percent(1).into();
	assert_eq!(
		calculate_withdrawal_fee(FixedU128::from(0), FixedU128::from(0), Permill::from_percent(1)),
		expected_fee
	);
}

#[test]
fn test_fee_amount() {
	assert_eq!(calculate_fee_amount_for_buy(Permill::from_float(0.01), 99), 2);
	assert_eq!(
		calculate_fee_amount_for_buy(Permill::from_percent(10), 50_000_000_000_000),
		5555555555556
	);
	assert_eq!(calculate_fee_amount_for_buy(Permill::from_percent(100), 99), 99);
	assert_eq!(calculate_fee_amount_for_buy(Permill::from_percent(0), 99), 0);
}

#[test]
fn calculate_buy_should_charge_less_when_fee_is_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(0);
	let protocol_fee = Permill::from_percent(0);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(666_666_666_668)
	);
	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 0,
			protocol_fee: 0,
		}
	);
}

#[test]
fn calculate_buy_should_charge_more_when_fee_is_not_zero() {
	let asset_in_state = AssetReserveState {
		reserve: 10 * UNIT,
		hub_reserve: 20 * UNIT,
		shares: 10 * UNIT,
		protocol_shares: 0u128,
	};
	let asset_out_state = AssetReserveState {
		reserve: 5 * UNIT,
		hub_reserve: 5 * UNIT,
		shares: 20 * UNIT,
		protocol_shares: 0u128,
	};

	let amount_to_buy = UNIT;
	let asset_fee = Permill::from_percent(10);
	let protocol_fee = Permill::from_percent(5);
	let imbalance = 2 * UNIT;

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		imbalance,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(813_008_130_082) // compared to previous testcase (calculate_buy_should_charge_less_when_fee_is_zero)
	);
	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 111_111_111_112,
			protocol_fee: 75_187_969_924,
		}
	);
}
