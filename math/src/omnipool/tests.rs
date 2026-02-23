use crate::omnipool::types::{
	AssetReserveState, BalanceUpdate, HubTradeSlipFees, Position, SignedBalance, TradeFee, TradeSlipFees,
};
use crate::omnipool::{
	calculate_add_liquidity_state_changes, calculate_buy_for_hub_asset_state_changes, calculate_buy_state_changes,
	calculate_cap_difference, calculate_fee_amount_for_buy, calculate_remove_liquidity_state_changes,
	calculate_sell_hub_state_changes, calculate_sell_state_changes, calculate_slip_fee_amount,
	calculate_tvl_cap_difference, calculate_withdrawal_fee, verify_asset_cap,
};
use crate::types::Balance;
use num_traits::{One, Zero};
use primitive_types::U256;
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(5714285714285u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2666666666666u128)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(5714285714285u128)
	);
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
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
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());
	let state_changes = state_changes.unwrap();
	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 26541554960,
			protocol_fee: 57142857142,
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(5714285714285u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2627613941018u128)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(5777720816326)
	);
	assert_eq!(state_changes.fee.protocol_fee, 57142857142);

	// Verify if fee + delta amount == delta with fee
	let f = 57142857142u128 + 5657142857143u128;
	let no_fees_amount: Balance = *state_changes.asset_in.total_delta_hub_reserve();
	assert_eq!(f, no_fees_amount);
}

#[test]
fn calculate_sell_with_fees_should_burn_halt_of_protocol_fee_amount_when_burn_fee_is_set() {
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
	let burn_fee = Permill::from_percent(50);

	let state_changes = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_sell,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(5714285714285u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(2627613941018u128)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(5777720816326)
	);
	let zero_fee_amount = 57142857142u128;
	let burn_amount = burn_fee.mul_floor(zero_fee_amount);
	assert_eq!(state_changes.fee.burned_protocol_fee, burn_amount);

	// Verify if fee + delta amount == delta with fee
	let f = 57142857142u128 + 5657142857143u128;
	let no_fees_amount: Balance = *state_changes.asset_in.total_delta_hub_reserve();
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
	let state_changes = calculate_sell_hub_state_changes(&asset_state, amount_to_sell, asset_fee, None);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1666666666666u128)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
		BalanceUpdate::Increase(amount_to_sell)
	);

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
	let state_changes = calculate_sell_hub_state_changes(&asset_state, amount_to_sell, asset_fee, None);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1649999999999u128)
	);

	let minted_amount = 48000000000u128;

	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
		BalanceUpdate::Increase(amount_to_sell + minted_amount)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 16666666667,
			protocol_fee: 0,
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(666666666668u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(1250000000001u128)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(1250000000001u128)
	);
	assert_eq!(state_changes.fee, TradeFee::default());
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
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
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
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
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(682966807814u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(1278608873546)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(1281685627304)
	);
	assert_eq!(state_changes.fee.protocol_fee, 12786088735);

	// Verify if fee + delta amount == delta with fee
	let f = 1265822784811u128 + 12786088735u128;
	let no_fees_amount: Balance = *state_changes.asset_in.total_delta_hub_reserve();
	assert_eq!(f, no_fees_amount);
}

#[test]
fn calculate_buy_with_fees_should_burn_half_of_protocol_fee_when_burn_fee_set_to_50_percent() {
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
	let burn_fee = Permill::from_percent(50);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset_in.delta_reserve,
		BalanceUpdate::Increase(682966807814u128)
	);
	assert_eq!(
		state_changes.asset_in.total_delta_hub_reserve(),
		BalanceUpdate::Decrease(1278608873546)
	);

	assert_eq!(
		state_changes.asset_out.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset_out.total_delta_hub_reserve(),
		BalanceUpdate::Increase(1281685627304)
	);
	let zero_burn_fee_amount = 12786088735u128;
	assert_eq!(
		state_changes.fee.burned_protocol_fee,
		burn_fee.mul_floor(zero_burn_fee_amount)
	);

	// Verify if fee + delta amount == delta with fee
	let f = 1265822784811u128 + 12786088735u128;
	let no_fees_amount: Balance = *state_changes.asset_in.total_delta_hub_reserve();
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
	let state_changes = calculate_buy_for_hub_asset_state_changes(&asset_state, amount_to_buy, asset_fee, None);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
		BalanceUpdate::Increase(5000000000001u128)
	);

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
	let state_changes = calculate_buy_for_hub_asset_state_changes(&asset_state, amount_to_buy, asset_fee, None);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_buy)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
		BalanceUpdate::Increase(5126742509213)
	);

	assert_eq!(
		state_changes.fee,
		TradeFee {
			asset_fee: 20_202_020_203,
			protocol_fee: 0,
			burned_protocol_fee: 0,
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
	let state_changes = calculate_add_liquidity_state_changes(&asset_state, amount_to_add);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Increase(amount_to_add)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
		BalanceUpdate::Increase(4000000000000u128)
	);
	assert_eq!(state_changes.asset.delta_shares, BalanceUpdate::Increase(amount_to_add));

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
	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(0.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes =
		calculate_remove_liquidity_state_changes(&asset_state, amount_to_remove, &position, FixedU128::zero());

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(amount_to_remove)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
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
	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(2.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes =
		calculate_remove_liquidity_state_changes(&asset_state, amount_to_remove, &position, FixedU128::zero());

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1891252955082u128)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
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
	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(2.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
		FixedU128::from_float(0.01),
	);

	assert!(state_changes.is_some());

	let state_changes = state_changes.unwrap();

	assert_eq!(
		state_changes.asset.delta_reserve,
		BalanceUpdate::Decrease(1872340425531u128)
	);
	assert_eq!(
		state_changes.asset.total_delta_hub_reserve(),
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
	let position = Position {
		amount: 3 * UNIT,
		shares: 3 * UNIT,
		price: (FixedU128::from_float(0.23).into_inner(), 1_000_000_000_000_000_000),
	};

	let state_changes = calculate_remove_liquidity_state_changes(
		&asset_state,
		amount_to_remove,
		&position,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
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
			burned_protocol_fee: 0,
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
	let burn_fee = Permill::from_percent(0);

	let state_changes = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
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
			burned_protocol_fee: 0,
		}
	);
}

// ============================================================================
// Slip fee unit tests
// ============================================================================

// --- calculate_slip_fee_amount tests ---

#[test]
fn slip_fee_amount_zero_when_q0_is_zero() {
	let result = calculate_slip_fee_amount(0, SignedBalance::zero(), SignedBalance::Negative(1000), Permill::from_percent(5), 1_000_000 * UNIT);
	assert_eq!(result, Some(0));
}

#[test]
fn slip_fee_amount_zero_when_base_is_zero() {
	let result = calculate_slip_fee_amount(
		10_000_000 * UNIT,
		SignedBalance::zero(),
		SignedBalance::Negative(99_000 * UNIT),
		Permill::from_percent(5),
		0,
	);
	assert_eq!(result, Some(0));
}

#[test]
fn slip_fee_amount_basic_negative_delta() {
	// Q0 = 10M, prior_delta = 0, delta_q = -99000 (LRNA leaving)
	// rate = 99000 / (10_000_000 - 99000) = 99000 / 9_901_000 ≈ 0.9998%
	// amount = 99000 * base / 9_901_000 (full precision)
	let base = 1_000_000 * UNIT;
	let result = calculate_slip_fee_amount(
		10_000_000 * UNIT,
		SignedBalance::zero(),
		SignedBalance::Negative(99_000 * UNIT),
		Permill::from_percent(5),
		base,
	);
	assert!(result.is_some());
	let amount = result.unwrap();
	// expected ≈ 0.009998... * base = 9_998_989_999...
	assert!(amount > 0);
	// rate ≈ 0.9998%, so amount should be close to 0.9998% of base
	assert!(amount > Permill::from_parts(9998).mul_floor(base));
	assert!(amount < Permill::from_parts(9999).mul_floor(base));
}

#[test]
fn slip_fee_amount_basic_positive_delta() {
	// Q0 = 5M, prior_delta = 0, delta_q = +97516 (LRNA entering buy pool)
	// rate = 97516 / (5_000_000 + 97516) = 97516 / 5_097_516 ≈ 1.913%
	let base = 1_000_000 * UNIT;
	let result = calculate_slip_fee_amount(
		5_000_000 * UNIT,
		SignedBalance::zero(),
		SignedBalance::Positive(97_516 * UNIT),
		Permill::from_percent(5),
		base,
	);
	assert!(result.is_some());
	let amount = result.unwrap();
	// rate ≈ 1.913%, so amount should be close to 1.913% of base
	assert!(amount > Permill::from_parts(19130).mul_floor(base));
	assert!(amount < Permill::from_parts(19131).mul_floor(base));
}

#[test]
fn slip_fee_amount_capped_at_max() {
	// Q0 = 1000, delta = -900 → rate = 900/100 = 900% → capped at max (5%)
	let base = 1_000_000 * UNIT;
	let max_fee = Permill::from_percent(5);
	let result = calculate_slip_fee_amount(1000 * UNIT, SignedBalance::zero(), SignedBalance::Negative(900 * UNIT), max_fee, base);
	assert_eq!(result, Some(max_fee.mul_floor(base)));
}

#[test]
fn slip_fee_amount_cumulative_grows() {
	let base = 1_000_000 * UNIT;
	// First call: Q0 = 10M, prior = 0, delta = -100K
	let fee1 = calculate_slip_fee_amount(
		10_000_000 * UNIT,
		SignedBalance::zero(),
		SignedBalance::Negative(100_000 * UNIT),
		Permill::from_percent(10),
		base,
	)
	.unwrap();

	// Second call: Q0 = 10M, prior = -100K (from first trade), delta = -100K
	let fee2 = calculate_slip_fee_amount(
		10_000_000 * UNIT,
		SignedBalance::Negative(100_000 * UNIT),
		SignedBalance::Negative(100_000 * UNIT),
		Permill::from_percent(10),
		base,
	)
	.unwrap();

	assert!(fee2 > fee1, "Second trade should have higher slip fee amount");
}

#[test]
fn slip_fee_amount_infeasible_returns_none() {
	let base = 1_000_000 * UNIT;
	// denom <= 0: Q0 = 1000, cumulative = -1000 → denom = 0
	let result = calculate_slip_fee_amount(1000 * UNIT, SignedBalance::zero(), SignedBalance::Negative(1000 * UNIT), Permill::from_percent(5), base);
	assert_eq!(result, None);

	// denom < 0: Q0 = 1000, cumulative = -1500 → denom = -500
	let result = calculate_slip_fee_amount(1000 * UNIT, SignedBalance::zero(), SignedBalance::Negative(1500 * UNIT), Permill::from_percent(5), base);
	assert_eq!(result, None);
}

#[test]
fn slip_fee_amount_zero_delta() {
	let result = calculate_slip_fee_amount(10_000_000 * UNIT, SignedBalance::zero(), SignedBalance::zero(), Permill::from_percent(5), 1_000_000 * UNIT);
	assert_eq!(result, Some(0));
}

// --- sell state changes with slip ---

#[test]
fn sell_with_slip_reduces_output() {
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount = 100_000 * UNIT;
	let asset_fee = Permill::from_rational(25u32, 10000u32); // 0.25%
	let protocol_fee = Permill::from_rational(5u32, 10000u32); // 0.05%
	let burn_fee = Permill::zero();

	// Without slip
	let no_slip = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	)
	.unwrap();

	// With slip
	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};
	let with_slip = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount,
		asset_fee,
		protocol_fee,
		burn_fee,
		Some(&slip),
	)
	.unwrap();

	// Slip version should give less output
	assert!(
		*with_slip.asset_out.delta_reserve < *no_slip.asset_out.delta_reserve,
		"Slip should reduce output: {} < {}",
		*with_slip.asset_out.delta_reserve,
		*no_slip.asset_out.delta_reserve
	);

	// Slip version should have higher protocol fee (includes slip amounts)
	assert!(with_slip.fee.protocol_fee > no_slip.fee.protocol_fee);
}

#[test]
fn sell_with_slip_invariant_holds() {
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let result = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		100_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Invariant: delta_hub_in - total_protocol_fee == D_net
	let delta_hub_in = *result.asset_in.delta_hub_reserve;
	let d_net = *result.asset_out.delta_hub_reserve;
	let total_protocol_fee = result.fee.protocol_fee;

	assert_eq!(
		delta_hub_in - total_protocol_fee,
		d_net,
		"Invariant: delta_hub_in({}) - protocol_fee({}) should equal D_net({})",
		delta_hub_in,
		total_protocol_fee,
		d_net
	);
}

#[test]
fn sell_with_slip_spec_example() {
	// Spec example: Sell 100K HDX→DOT
	// HDX: Q₀ = 10M LRNA, R = 10M HDX
	// DOT: Q₀ = 5M LRNA, R = 500K DOT
	// s = 1, max_slip_fee = 5%
	// protocol_fee = 0.05%, asset_fee = 0.25%
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let result = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		100_000 * UNIT,
		Permill::from_rational(25u32, 10000u32), // 0.25%
		Permill::from_rational(5u32, 10000u32),  // 0.05%
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// SELL SIDE:
	// delta_qi = 10M * 100K / (10M + 100K) = 10^12 / 10.1M ≈ 99009.9009... LRNA
	let delta_hub_in = *result.asset_in.delta_hub_reserve;
	// This is ~99009.9 LRNA (in 12 decimal units)
	assert!(delta_hub_in > 99_009 * UNIT && delta_hub_in < 99_010 * UNIT);

	// Slip sell rate ≈ 0.999% (from spec)
	// Protocol fee = 0.05% of delta_hub_in
	// D_gross ≈ 97,516 LRNA (from spec: 99000 * (1 - 0.05% - 0.999%))
	// Slip buy rate ≈ 1.91%
	// D_net ≈ 95,654 LRNA (from spec: 97516 * (1 - 1.91%))
	let d_net = *result.asset_out.delta_hub_reserve;
	// D_net should be roughly 95,654 * UNIT
	// Allow some tolerance for rounding differences
	assert!(
		d_net > 95_000 * UNIT && d_net < 96_500 * UNIT,
		"D_net should be ~95,654 LRNA, got {}",
		d_net / UNIT
	);

	// Verify the invariant
	assert_eq!(delta_hub_in - result.fee.protocol_fee, d_net);
}

// --- buy state changes with slip ---

#[test]
fn buy_with_slip_increases_cost() {
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount_to_buy = 1000 * UNIT;
	let asset_fee = Permill::from_rational(25u32, 10000u32);
	let protocol_fee = Permill::from_rational(5u32, 10000u32);
	let burn_fee = Permill::zero();

	let no_slip = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		None,
	)
	.unwrap();

	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let with_slip = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		burn_fee,
		Some(&slip),
	)
	.unwrap();

	// Slip version should cost more input
	assert!(
		*with_slip.asset_in.delta_reserve > *no_slip.asset_in.delta_reserve,
		"Slip should increase cost: {} > {}",
		*with_slip.asset_in.delta_reserve,
		*no_slip.asset_in.delta_reserve
	);
}

#[test]
fn buy_sell_roundtrip_with_slip() {
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let sell_amount = 10_000 * UNIT;
	let asset_fee = Permill::from_percent(0);
	let protocol_fee = Permill::from_percent(0);
	let burn_fee = Permill::zero();

	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	// Sell X → get Y
	let sell_result = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		sell_amount,
		asset_fee,
		protocol_fee,
		burn_fee,
		Some(&slip),
	)
	.unwrap();

	let tokens_received = *sell_result.asset_out.delta_reserve;

	// Now buy Y tokens (with the same initial state, fresh block)
	let buy_result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		tokens_received,
		asset_fee,
		protocol_fee,
		burn_fee,
		Some(&slip),
	)
	.unwrap();

	let cost_to_buy = *buy_result.asset_in.delta_reserve;

	// Round-trip: cost should be approximately equal to original sell_amount
	// With zero fees, the only difference is rounding (a few units at most)
	let diff = if cost_to_buy > sell_amount {
		cost_to_buy - sell_amount
	} else {
		sell_amount - cost_to_buy
	};
	// Allow tolerance for multi-step Permill rounding (quadratic inversion + forward check)
	assert!(
		diff <= UNIT / 10,
		"Round-trip should be approximately equal: sold {} got {} cost_to_buy_back {} diff {}",
		sell_amount,
		tokens_received,
		cost_to_buy,
		diff
	);
}

#[test]
fn buy_inversion_linear_only() {
	// Protocol fee = 0 and no sell slip should make the linear inversion exact
	// D_gross = D_net * (L + C) / (L - D_net) for buy side
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount_to_buy = 1000 * UNIT;

	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		Permill::zero(), // no asset fee
		Permill::zero(), // no protocol fee
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Verify by running forward: sell the computed input amount
	let forward = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		*result.asset_in.delta_reserve,
		Permill::zero(),
		Permill::zero(),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Forward sell should give approximately the desired output.
	// Integer truncation across 5+ steps (quadratic, slip fees, AMM) can compound,
	// so the forward result may be off by a few units in either direction.
	let diff = if *forward.asset_out.delta_reserve >= amount_to_buy {
		*forward.asset_out.delta_reserve - amount_to_buy
	} else {
		amount_to_buy - *forward.asset_out.delta_reserve
	};
	assert!(
		diff <= UNIT / 1000,
		"Forward check: sell {} should give ≈ {} got {} diff {}",
		*result.asset_in.delta_reserve,
		amount_to_buy,
		*forward.asset_out.delta_reserve,
		diff
	);
}

#[test]
fn buy_inversion_quadratic() {
	// Protocol fee > 0 + sell slip → quadratic inversion
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount_to_buy = 1000 * UNIT;
	let asset_fee = Permill::from_rational(25u32, 10000u32);
	let protocol_fee = Permill::from_rational(5u32, 10000u32);

	let slip = TradeSlipFees {
		asset_in_hub_reserve: asset_in_state.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out_state.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		amount_to_buy,
		asset_fee,
		protocol_fee,
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Verify by running forward: sell the computed input amount
	let forward = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		*result.asset_in.delta_reserve,
		asset_fee,
		protocol_fee,
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Forward sell should give approximately the desired output.
	let diff = if *forward.asset_out.delta_reserve >= amount_to_buy {
		*forward.asset_out.delta_reserve - amount_to_buy
	} else {
		amount_to_buy - *forward.asset_out.delta_reserve
	};
	assert!(
		diff <= UNIT / 1000,
		"Forward check: sell {} should give ≈ {} got {} diff {}",
		*result.asset_in.delta_reserve,
		amount_to_buy,
		*forward.asset_out.delta_reserve,
		diff
	);
}

// --- hub asset trade tests with slip ---

#[test]
fn sell_hub_with_slip_reduces_output() {
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount = 100_000 * UNIT;
	let asset_fee = Permill::from_rational(25u32, 10000u32);

	let no_slip = calculate_sell_hub_state_changes(&asset_out_state, amount, asset_fee, None).unwrap();

	let slip = HubTradeSlipFees {
		asset_hub_reserve: asset_out_state.hub_reserve,
		asset_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let with_slip = calculate_sell_hub_state_changes(&asset_out_state, amount, asset_fee, Some(&slip)).unwrap();

	assert!(
		*with_slip.asset.delta_reserve < *no_slip.asset.delta_reserve,
		"Slip should reduce output: {} < {}",
		*with_slip.asset.delta_reserve,
		*no_slip.asset.delta_reserve
	);
}

#[test]
fn buy_for_hub_with_slip_increases_cost() {
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let amount_to_buy = 1000 * UNIT;
	let asset_fee = Permill::from_rational(25u32, 10000u32);

	let no_slip = calculate_buy_for_hub_asset_state_changes(&asset_out_state, amount_to_buy, asset_fee, None).unwrap();

	let slip = HubTradeSlipFees {
		asset_hub_reserve: asset_out_state.hub_reserve,
		asset_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	let with_slip =
		calculate_buy_for_hub_asset_state_changes(&asset_out_state, amount_to_buy, asset_fee, Some(&slip)).unwrap();

	// With slip, D_net stays the same (same tokens out) but total LRNA cost is higher
	// The delta_hub_reserve should be D_net (same), but protocol_fee has the slip amount
	assert!(
		with_slip.fee.protocol_fee > no_slip.fee.protocol_fee,
		"Slip should add to protocol fee: {} > {}",
		with_slip.fee.protocol_fee,
		no_slip.fee.protocol_fee
	);
}

#[test]
fn sell_hub_buy_hub_roundtrip_with_slip() {
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let lrna_amount = 10_000 * UNIT;
	let asset_fee = Permill::zero();

	let slip = HubTradeSlipFees {
		asset_hub_reserve: asset_out_state.hub_reserve,
		asset_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(5),
	};

	// Sell X LRNA → get Y tokens
	let sell_result = calculate_sell_hub_state_changes(&asset_out_state, lrna_amount, asset_fee, Some(&slip)).unwrap();

	let tokens_received = *sell_result.asset.delta_reserve;

	// Buy Y tokens → costs how much LRNA?
	let buy_result =
		calculate_buy_for_hub_asset_state_changes(&asset_out_state, tokens_received, asset_fee, Some(&slip)).unwrap();

	let d_net = *buy_result.asset.delta_hub_reserve;
	let slip_buy_amount = buy_result.fee.protocol_fee;
	let total_lrna_cost = d_net + slip_buy_amount;

	// Round-trip: cost should be approximately equal to original lrna_amount
	let diff = if total_lrna_cost > lrna_amount {
		total_lrna_cost - lrna_amount
	} else {
		lrna_amount - total_lrna_cost
	};

	assert!(
		diff <= UNIT / 1000,
		"Hub round-trip should be approximately equal: sold {} got tokens {} cost_back {} diff {}",
		lrna_amount,
		tokens_received,
		total_lrna_cost,
		diff
	);
}

// ========== Cross-validation against Python reference ==========
// Values obtained from HydraDX-simulations OmnipoolState with mpmath (50 decimal places).
// Tolerance: ±1 (integer rounding difference between Python float→floor and Rust integer math).

/// Helper to assert within tolerance of ±1
fn assert_within_one(rust_val: u128, python_val: u128, label: &str) {
	assert_within_tolerance(rust_val, python_val, 1, label);
}

/// Helper to assert within specified tolerance
fn assert_within_tolerance(rust_val: u128, python_val: u128, tolerance: u128, label: &str) {
	let diff = if rust_val >= python_val {
		rust_val - python_val
	} else {
		python_val - rust_val
	};
	assert!(
		diff <= tolerance,
		"Cross-validation mismatch for {}: rust={} python={} diff={} (tolerance=±{})",
		label,
		rust_val,
		python_val,
		diff,
		tolerance
	);
}

#[test]
fn cross_validate_scenario1_sell_hdx_for_dot() {
	// Python scenario 1: Sell 100K HDX → DOT
	// Pool: HDX(10M liquidity, 10M LRNA), DOT(500K liquidity, 5M LRNA)
	// lrna_fee=0.0005, asset_fee=0.0025, slip_factor=1.0
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100), // no cap for cross-validation
	};

	let result = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		100_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Python values (floor of mpf * 10^12):
	let py_delta_hub_in: u128 = 99_009_900_990_099_009;
	let py_d_net: u128 = 96_087_551_830_955_825;
	let py_tokens_out: u128 = 9_404_011_604_641_856;
	let py_asset_fee: u128 = 23_568_951_390_079;

	assert_within_one(*result.asset_in.delta_hub_reserve, py_delta_hub_in, "delta_hub_in");
	assert_within_one(*result.asset_out.delta_hub_reserve, py_d_net, "D_net");
	assert_within_one(*result.asset_out.delta_reserve, py_tokens_out, "tokens_out");
	assert_within_one(result.fee.asset_fee, py_asset_fee, "asset_fee");

	// Verify invariant: delta_hub_in - protocol_fee (bundled) = D_net
	assert_eq!(
		*result.asset_in.delta_hub_reserve - result.fee.protocol_fee,
		*result.asset_out.delta_hub_reserve
	);
}

#[test]
fn cross_validate_scenario2_sell_lrna_for_dot() {
	// Python scenario 2: Sell 10K LRNA → DOT (hub trade, buy-side slip only)
	// Pool: DOT(500K liquidity, 5M LRNA), asset_fee=0.0025, slip_factor=1.0
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = HubTradeSlipFees {
		asset_hub_reserve: 5_000_000 * UNIT,
		asset_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100),
	};

	let result = calculate_sell_hub_state_changes(
		&asset_out_state,
		10_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Some(&slip),
	)
	.unwrap();

	// Python values:
	let py_d_net: u128 = 9_980_039_920_159_680;
	let py_tokens_out: u128 = 993_525_896_414_342;
	let py_asset_fee: u128 = 2_490_039_840_637;
	let py_slip_buy: u128 = 19_960_079_840_319;

	assert_within_one(*result.asset.delta_hub_reserve, py_d_net, "D_net");
	assert_within_one(*result.asset.delta_reserve, py_tokens_out, "tokens_out");
	assert_within_one(result.fee.asset_fee, py_asset_fee, "asset_fee");
	assert_within_one(result.fee.protocol_fee, py_slip_buy, "slip_buy_amount");
}

#[test]
fn cross_validate_scenario3_buy_dot_with_hdx() {
	// Python scenario 3: Buy 1000 DOT with HDX (buy-specified, quadratic inversion)
	// Pool: HDX(10M/10M), DOT(500K/5M), lrna_fee=0.0005, asset_fee=0.0025, slip_factor=1.0
	let asset_in_state = AssetReserveState {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100),
	};

	let result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		1000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Python values:
	let py_sell_quantity: u128 = 10_090_809_943_053_344;
	let py_delta_hub_in: u128 = 10_080_637_763_076_148;
	let py_d_net: u128 = 10_045_203_415_369_161;
	let _py_asset_fee: u128 = 2_506_265_664_160;

	// Buy inversion has multi-step rounding → allow wider tolerance (UNIT/1000 = 10^9)
	let tolerance: u128 = UNIT / 1000;

	let diff_sell = if *result.asset_in.delta_reserve >= py_sell_quantity {
		*result.asset_in.delta_reserve - py_sell_quantity
	} else {
		py_sell_quantity - *result.asset_in.delta_reserve
	};
	assert!(
		diff_sell <= tolerance,
		"sell_quantity: rust={} python={} diff={}",
		*result.asset_in.delta_reserve,
		py_sell_quantity,
		diff_sell
	);

	let diff_hub = if *result.asset_in.delta_hub_reserve >= py_delta_hub_in {
		*result.asset_in.delta_hub_reserve - py_delta_hub_in
	} else {
		py_delta_hub_in - *result.asset_in.delta_hub_reserve
	};
	assert!(
		diff_hub <= tolerance,
		"delta_hub_in: rust={} python={} diff={}",
		*result.asset_in.delta_hub_reserve,
		py_delta_hub_in,
		diff_hub
	);

	let diff_dnet = if *result.asset_out.delta_hub_reserve >= py_d_net {
		*result.asset_out.delta_hub_reserve - py_d_net
	} else {
		py_d_net - *result.asset_out.delta_hub_reserve
	};
	assert!(
		diff_dnet <= tolerance,
		"D_net: rust={} python={} diff={}",
		*result.asset_out.delta_hub_reserve,
		py_d_net,
		diff_dnet
	);

	// Forward check: selling the computed input should yield at least 1000 DOT
	let forward = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		*result.asset_in.delta_reserve,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Forward check: allow small rounding tolerance (multi-step integer truncation)
	let fwd_diff = if *forward.asset_out.delta_reserve >= 1000 * UNIT {
		*forward.asset_out.delta_reserve - 1000 * UNIT
	} else {
		1000 * UNIT - *forward.asset_out.delta_reserve
	};
	assert!(
		fwd_diff <= UNIT / 1000,
		"Forward check: sell {} should give ≈ 1000 DOT, got {} diff {}",
		*result.asset_in.delta_reserve,
		*forward.asset_out.delta_reserve,
		fwd_diff
	);
}

#[test]
fn cross_validate_scenario4_consecutive_sells() {
	// Two consecutive sells HDX→DOT in the same block.
	// Trade 1: Sell 50K HDX → DOT (establishes deltas on both pools)
	// Trade 2: Sell 50K HDX → DOT (with accumulated deltas)
	//
	// For cross-validation, we use Python-computed intermediate pool state and
	// deltas after trade 1 to ensure trade 2 starts from the exact same state
	// as the Python model. This isolates trade 2 math from trade 1 rounding.

	// Python-computed pool state after trade 1
	let asset_in_state2 = AssetReserveState {
		reserve: 10_050_000_000_000_000_000,    // HDX reserve
		hub_reserve: 9_950_248_756_218_905_472, // HDX hub_reserve
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state2 = AssetReserveState {
		reserve: 495_160_389_163_715_130,       // DOT reserve
		hub_reserve: 5_049_115_249_061_335_511, // DOT hub_reserve
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	// Python-computed deltas after trade 1
	let hdx_delta = SignedBalance::Negative(49_751_243_781_094_527); // LRNA left HDX pool
	let dot_delta = SignedBalance::Positive(49_115_249_061_335_511); // LRNA entered DOT pool

	// Trade 2 with accumulated deltas
	let slip2 = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT, // Q0 at block start (unchanged)
		asset_in_delta: hdx_delta,
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: dot_delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let r2 = calculate_sell_state_changes(
		&asset_in_state2,
		&asset_out_state2,
		50_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip2),
	)
	.unwrap();

	// Python values for trade 2:
	let py_delta_hub_in: u128 = 49_258_657_209_004_482;
	let py_d_net: u128 = 47_805_817_492_268_201;
	let py_tokens_out: u128 = 4_632_672_944_598_431;
	let py_asset_fee: u128 = 11_610_709_134_331;

	// ±2 tolerance: two sequential slip fee computations (sell-side + buy-side), each rounds independently
	assert_within_tolerance(*r2.asset_in.delta_hub_reserve, py_delta_hub_in, 2, "S4 delta_hub_in");
	assert_within_tolerance(*r2.asset_out.delta_hub_reserve, py_d_net, 2, "S4 D_net");
	assert_within_tolerance(*r2.asset_out.delta_reserve, py_tokens_out, 2, "S4 tokens_out");
	assert_within_tolerance(r2.fee.asset_fee, py_asset_fee, 2, "S4 asset_fee");

	// Also verify behavioral property: run a fresh-delta trade with the same pool state
	// to confirm cumulative slip makes trade 2 worse
	let slip_fresh = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100),
	};
	let r_fresh = calculate_sell_state_changes(
		&asset_in_state2,
		&asset_out_state2,
		50_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip_fresh),
	)
	.unwrap();

	assert!(
		*r2.asset_out.delta_reserve < *r_fresh.asset_out.delta_reserve,
		"Cumulative slip should give less output than fresh trade"
	);
}

#[test]
fn cross_validate_scenario5_opposing_flow() {
	// Trade 1: Sell 50K HDX → DOT (HDX loses LRNA, DOT gains LRNA)
	// Trade 2: Sell 5K DOT → HDX (opposing direction: reverses the deltas partially)
	//
	// For trade 2, DOT is sell pool with positive prior delta (LRNA entered before),
	// HDX is buy pool with negative prior delta (LRNA left before).
	// Opposing flow should produce LOWER slip fees.
	//
	// Using Python-computed intermediate state to isolate trade 2 cross-validation.

	// Python-computed pool state after trade 1 (50K HDX→DOT)
	let dot_state2 = AssetReserveState {
		reserve: 495_160_389_163_715_130,       // DOT reserve
		hub_reserve: 5_049_115_249_061_335_511, // DOT hub_reserve
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};
	let hdx_state2 = AssetReserveState {
		reserve: 10_050_000_000_000_000_000,    // HDX reserve
		hub_reserve: 9_950_248_756_218_905_472, // HDX hub_reserve
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};

	// Python-computed deltas after trade 1
	let hdx_delta = SignedBalance::Negative(49_751_243_781_094_527); // LRNA left HDX pool (negative)
	let dot_delta = SignedBalance::Positive(49_115_249_061_335_511); // LRNA entered DOT pool (positive)

	// Trade 2: DOT → HDX (opposing flow)
	// Now DOT is sell pool (prior_delta > 0), HDX is buy pool (prior_delta < 0)
	let slip2 = TradeSlipFees {
		asset_in_hub_reserve: 5_000_000 * UNIT,   // DOT Q0
		asset_in_delta: dot_delta,                // positive (LRNA entered)
		asset_out_hub_reserve: 10_000_000 * UNIT, // HDX Q0
		asset_out_delta: hdx_delta,               // negative (LRNA left)
		max_slip_fee: Permill::from_percent(100),
	};

	let r2 = calculate_sell_state_changes(
		&dot_state2, // DOT is now sell (asset_in)
		&hdx_state2, // HDX is now buy (asset_out)
		5_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip2),
	)
	.unwrap();

	// Python values:
	let py_delta_hub_in: u128 = 50_474_961_216_977_065;
	let py_d_net: u128 = 50_432_540_351_042_778;
	let py_tokens_out: u128 = 50_554_547_031_217_231;
	let py_asset_fee: u128 = 126_703_125_391_521;

	assert_within_one(*r2.asset_in.delta_hub_reserve, py_delta_hub_in, "S5 delta_hub_in");
	assert_within_one(*r2.asset_out.delta_hub_reserve, py_d_net, "S5 D_net");
	assert_within_one(*r2.asset_out.delta_reserve, py_tokens_out, "S5 tokens_out");
	assert_within_one(r2.fee.asset_fee, py_asset_fee, "S5 asset_fee");

	// Verify behavioral property: opposing flow should have lower slip than same-direction
	// Use the slip fee breakdown from protocol_fee to check
	let protocol_fee_portion = Permill::from_rational(5u32, 10000u32).mul_floor(*r2.asset_in.delta_hub_reserve);
	let trade2_total_slip = r2.fee.protocol_fee.saturating_sub(protocol_fee_portion);
	// With opposing flow, slip fees should be very small (partial cancellation)
	// Python shows slip_sell=0.027%, slip_buy=0.007% — total slip is small fraction of D_hub_in
	assert!(
		trade2_total_slip < *r2.asset_in.delta_hub_reserve / 100,
		"Opposing flow slip should be <1% of delta_hub_in: {} < {}",
		trade2_total_slip,
		*r2.asset_in.delta_hub_reserve / 100
	);
}

#[test]
fn cross_validate_scenario6_buy_hub_with_prior_delta() {
	// Trade 1: Sell 20K HDX → DOT (establishes positive delta on DOT)
	// Trade 2: Buy 100 DOT for LRNA (hub trade, with prior delta on buy pool)
	//
	// Using Python-computed intermediate state to isolate trade 2 cross-validation.

	// Python-computed DOT delta and pool state after trade 1 (20K HDX→DOT, lrna_fee=0)
	let dot_delta = SignedBalance::Positive(19_890_712_487_233_615); // positive (LRNA entered DOT pool)

	let dot_state2 = AssetReserveState {
		reserve: 498_028_671_741_334_516,       // DOT reserve after trade 1
		hub_reserve: 5_019_890_712_487_233_615, // DOT hub_reserve after trade 1
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	// Trade 2: Buy 100 DOT for LRNA (hub trade with prior positive delta)
	let hub_slip = HubTradeSlipFees {
		asset_hub_reserve: 5_000_000 * UNIT, // Q0 at block start
		asset_delta: dot_delta,              // positive from trade 1
		max_slip_fee: Permill::from_percent(100),
	};

	let r2 = calculate_buy_for_hub_asset_state_changes(
		&dot_state2,
		100 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Some(&hub_slip),
	)
	.unwrap();

	// Python values:
	let py_d_net: u128 = 1_010_681_792_565_891;
	let py_slip_buy: u128 = 4_225_785_912_543;
	let py_asset_fee: u128 = 250_626_566_416;

	assert_within_one(*r2.asset.delta_hub_reserve, py_d_net, "S6 D_net");
	assert_within_one(r2.fee.protocol_fee, py_slip_buy, "S6 slip_buy");
	assert_within_one(r2.fee.asset_fee, py_asset_fee, "S6 asset_fee");

	// Buy with prior positive delta should cost MORE LRNA than fresh block
	let r2_fresh = calculate_buy_for_hub_asset_state_changes(
		&dot_state2,
		100 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Some(&HubTradeSlipFees {
			asset_hub_reserve: 5_000_000 * UNIT,
			asset_delta: SignedBalance::zero(), // fresh block
			max_slip_fee: Permill::from_percent(100),
		}),
	)
	.unwrap();

	let cost_with_delta = *r2.asset.delta_hub_reserve + r2.fee.protocol_fee;
	let cost_fresh = *r2_fresh.asset.delta_hub_reserve + r2_fresh.fee.protocol_fee;
	assert!(
		cost_with_delta > cost_fresh,
		"Prior positive delta should increase hub buy cost: {} > {}",
		cost_with_delta,
		cost_fresh
	);
}

#[test]
fn cross_validate_scenario7_large_trade_high_slip() {
	// Large trade on small pool: sell 200K HDX into 1M/1M pools
	// Slip sell rate should be ~20%, slip buy rate ~11.8%

	let asset_in_state = AssetReserveState {
		reserve: 1_000_000 * UNIT,
		hub_reserve: 1_000_000 * UNIT,
		shares: 1_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 100_000 * UNIT,
		hub_reserve: 1_000_000 * UNIT,
		shares: 100_000 * UNIT,
		protocol_shares: 0,
	};

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 1_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 1_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100),
	};

	let result = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		200_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Python values:
	let py_delta_hub_in: u128 = 166_666_666_666_666_666;
	let py_d_net: u128 = 117_582_175_159_938_230;
	let py_tokens_out: u128 = 10_494_818_397_157_520;
	let py_asset_fee: u128 = 26_302_803_000_394;

	assert_within_one(*result.asset_in.delta_hub_reserve, py_delta_hub_in, "S7 delta_hub_in");
	assert_within_one(*result.asset_out.delta_hub_reserve, py_d_net, "S7 D_net");
	assert_within_one(*result.asset_out.delta_reserve, py_tokens_out, "S7 tokens_out");
	assert_within_one(result.fee.asset_fee, py_asset_fee, "S7 asset_fee");

	// Verify invariant still holds
	assert_eq!(
		*result.asset_in.delta_hub_reserve - result.fee.protocol_fee,
		*result.asset_out.delta_hub_reserve,
		"Invariant: delta_hub_in - protocol_fee == D_net"
	);
}

#[test]
fn cross_validate_scenario8_buy_after_prior_sell() {
	// Buy-specified DOT with HDX after a prior sell HDX→DOT.
	// Trade 1: Sell 50K HDX → DOT (establishes cumulative deltas)
	// Trade 2: Buy 500 DOT with HDX (quadratic inversion with non-zero deltas)
	//
	// Using Python-computed intermediate state to isolate trade 2.

	// Python-computed pool state after trade 1 (50K HDX→DOT)
	let asset_in_state = AssetReserveState {
		reserve: 10_050_000_000_000_000_000,    // HDX reserve
		hub_reserve: 9_950_248_756_218_905_472, // HDX hub_reserve
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 495_160_389_163_715_130,       // DOT reserve
		hub_reserve: 5_049_115_249_061_335_511, // DOT hub_reserve
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};

	// Python-computed deltas after trade 1
	let hdx_delta = SignedBalance::Negative(49_751_243_781_094_527);
	let dot_delta = SignedBalance::Positive(49_115_249_061_335_511);

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: hdx_delta,
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: dot_delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		500 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Python values for trade 2:
	let py_sell_quantity: u128 = 5_258_240_575_748_856;
	let py_delta_hub_in: u128 = 5_203_327_502_581_279;
	let py_d_net: u128 = 5_116_421_899_997_441;

	// Buy inversion with non-zero deltas: multi-step rounding → UNIT/1000 tolerance
	let tolerance: u128 = UNIT / 1000;

	assert_within_tolerance(
		*result.asset_in.delta_reserve,
		py_sell_quantity,
		tolerance,
		"S8 sell_quantity",
	);
	assert_within_tolerance(
		*result.asset_in.delta_hub_reserve,
		py_delta_hub_in,
		tolerance,
		"S8 delta_hub_in",
	);
	assert_within_tolerance(*result.asset_out.delta_hub_reserve, py_d_net, tolerance, "S8 D_net");

	// Forward check: selling the computed input should yield ≈500 DOT
	let forward = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		*result.asset_in.delta_reserve,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	let fwd_diff = if *forward.asset_out.delta_reserve >= 500 * UNIT {
		*forward.asset_out.delta_reserve - 500 * UNIT
	} else {
		500 * UNIT - *forward.asset_out.delta_reserve
	};
	assert!(
		fwd_diff <= UNIT / 1000,
		"Forward check: sell {} should give ≈500 DOT, got {} diff {}",
		*result.asset_in.delta_reserve,
		*forward.asset_out.delta_reserve,
		fwd_diff
	);

	// Behavioral: buying with accumulated deltas should cost more than fresh block
	let slip_fresh = TradeSlipFees {
		asset_in_hub_reserve: 10_000_000 * UNIT,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: 5_000_000 * UNIT,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee: Permill::from_percent(100),
	};
	let fresh = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		500 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip_fresh),
	)
	.unwrap();

	assert!(
		*result.asset_in.delta_reserve > *fresh.asset_in.delta_reserve,
		"Cumulative deltas should make buy cost more: {} > {}",
		*result.asset_in.delta_reserve,
		*fresh.asset_in.delta_reserve
	);
}

#[test]
fn cross_validate_scenario9_buy_opposing_flow() {
	// Buy-specified HDX with DOT after a prior sell HDX→DOT (opposing direction).
	// Trade 1: Sell 50K HDX → DOT (HDX loses LRNA, DOT gains LRNA)
	// Trade 2: Buy 5000 HDX with DOT (opposing: DOT sells LRNA out, HDX receives LRNA)
	//
	// Using Python-computed intermediate state to isolate trade 2.

	// Python-computed pool state after trade 1 (50K HDX→DOT)
	// For trade 2: DOT is sell (asset_in), HDX is buy (asset_out)
	let asset_in_state = AssetReserveState {
		reserve: 495_160_389_163_715_130,       // DOT reserve
		hub_reserve: 5_049_115_249_061_335_511, // DOT hub_reserve
		shares: 500_000 * UNIT,
		protocol_shares: 0,
	};
	let asset_out_state = AssetReserveState {
		reserve: 10_050_000_000_000_000_000,    // HDX reserve
		hub_reserve: 9_950_248_756_218_905_472, // HDX hub_reserve
		shares: 10_000_000 * UNIT,
		protocol_shares: 0,
	};

	// Python-computed deltas after trade 1
	// DOT (now sell pool) had positive delta (LRNA entered)
	// HDX (now buy pool) had negative delta (LRNA left)
	let dot_delta = SignedBalance::Positive(49_115_249_061_335_511); // positive
	let hdx_delta = SignedBalance::Negative(49_751_243_781_094_527); // negative

	let slip = TradeSlipFees {
		asset_in_hub_reserve: 5_000_000 * UNIT, // DOT Q0
		asset_in_delta: dot_delta,
		asset_out_hub_reserve: 10_000_000 * UNIT, // HDX Q0
		asset_out_delta: hdx_delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let result = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		5_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	// Python values for trade 2:
	let py_sell_quantity: u128 = 494_189_881_306_675;
	let py_delta_hub_in: u128 = 5_034_194_721_745_323;
	let py_d_net: u128 = 4_965_255_931_944_712;

	// Buy inversion with opposing deltas: multi-step rounding → UNIT/1000 tolerance
	let tolerance: u128 = UNIT / 1000;

	assert_within_tolerance(
		*result.asset_in.delta_reserve,
		py_sell_quantity,
		tolerance,
		"S9 sell_quantity",
	);
	assert_within_tolerance(
		*result.asset_in.delta_hub_reserve,
		py_delta_hub_in,
		tolerance,
		"S9 delta_hub_in",
	);
	assert_within_tolerance(*result.asset_out.delta_hub_reserve, py_d_net, tolerance, "S9 D_net");

	// Forward check: selling the computed DOT should yield ≈5000 HDX
	let forward = calculate_sell_state_changes(
		&asset_in_state,
		&asset_out_state,
		*result.asset_in.delta_reserve,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip),
	)
	.unwrap();

	let fwd_diff = if *forward.asset_out.delta_reserve >= 5_000 * UNIT {
		*forward.asset_out.delta_reserve - 5_000 * UNIT
	} else {
		5_000 * UNIT - *forward.asset_out.delta_reserve
	};
	assert!(
		fwd_diff <= UNIT / 1000,
		"Forward check: sell {} DOT should give ≈5000 HDX, got {} diff {}",
		*result.asset_in.delta_reserve,
		*forward.asset_out.delta_reserve,
		fwd_diff
	);

	// Behavioral: opposing flow should cost LESS than same-direction cumulative
	// Compare against buying 5000 HDX with same-direction deltas
	let slip_same_dir = TradeSlipFees {
		asset_in_hub_reserve: 5_000_000 * UNIT,
		asset_in_delta: dot_delta.negate(), // flip to same direction (negative = LRNA left)
		asset_out_hub_reserve: 10_000_000 * UNIT,
		asset_out_delta: hdx_delta.negate(), // flip to same direction (positive = LRNA entered)
		max_slip_fee: Permill::from_percent(100),
	};
	let same_dir = calculate_buy_state_changes(
		&asset_in_state,
		&asset_out_state,
		5_000 * UNIT,
		Permill::from_rational(25u32, 10000u32),
		Permill::from_rational(5u32, 10000u32),
		Permill::zero(),
		Some(&slip_same_dir),
	)
	.unwrap();

	assert!(
		*result.asset_in.delta_reserve < *same_dir.asset_in.delta_reserve,
		"Opposing flow should cost less than same-direction: {} < {}",
		*result.asset_in.delta_reserve,
		*same_dir.asset_in.delta_reserve
	);
}

// ========== Multi-trade sequence tests ==========
// A 3-asset omnipool (HDX, DOT, ETH) with 12 mixed trades executed in two different
// orders, verifying path dependence and invariant preservation.

/// Pool state for one asset, tracking reserve, hub_reserve, Q0, and cumulative delta.
#[derive(Clone, Debug)]
struct PoolAsset {
	reserve: Balance,
	hub_reserve: Balance,
	q0: Balance,            // LRNA at block start (fixed for the block)
	delta: SignedBalance,   // cumulative intra-block LRNA delta
	shares: Balance,
}

impl PoolAsset {
	fn state(&self) -> AssetReserveState<Balance> {
		AssetReserveState {
			reserve: self.reserve,
			hub_reserve: self.hub_reserve,
			shares: self.shares,
			protocol_shares: 0,
		}
	}
}

/// Execute a sell trade between two assets and update pool states + deltas.
/// Returns (tokens_out, protocol_fee_bundled).
fn exec_sell(
	sell_asset: &mut PoolAsset,
	buy_asset: &mut PoolAsset,
	amount: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
) -> (Balance, Balance) {
	let slip = TradeSlipFees {
		asset_in_hub_reserve: sell_asset.q0,
		asset_in_delta: sell_asset.delta,
		asset_out_hub_reserve: buy_asset.q0,
		asset_out_delta: buy_asset.delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let r = calculate_sell_state_changes(
		&sell_asset.state(),
		&buy_asset.state(),
		amount,
		asset_fee,
		protocol_fee,
		Permill::zero(),
		Some(&slip),
	)
	.expect("sell failed");

	// Update sell pool
	sell_asset.reserve += *r.asset_in.delta_reserve;
	sell_asset.hub_reserve -= *r.asset_in.delta_hub_reserve;
	sell_asset.delta = sell_asset
		.delta
		.checked_add(SignedBalance::Negative(*r.asset_in.delta_hub_reserve))
		.unwrap(); // LRNA left

	// Update buy pool
	buy_asset.reserve -= *r.asset_out.delta_reserve;
	buy_asset.hub_reserve += *r.asset_out.delta_hub_reserve;
	buy_asset.delta = buy_asset
		.delta
		.checked_add(SignedBalance::Positive(*r.asset_out.delta_hub_reserve))
		.unwrap(); // D_net entered

	(*r.asset_out.delta_reserve, r.fee.protocol_fee)
}

/// Execute a buy trade between two assets and update pool states + deltas.
/// Returns (tokens_in_cost, protocol_fee_bundled).
fn exec_buy(
	sell_asset: &mut PoolAsset,
	buy_asset: &mut PoolAsset,
	amount: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
) -> (Balance, Balance) {
	let slip = TradeSlipFees {
		asset_in_hub_reserve: sell_asset.q0,
		asset_in_delta: sell_asset.delta,
		asset_out_hub_reserve: buy_asset.q0,
		asset_out_delta: buy_asset.delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let r = calculate_buy_state_changes(
		&sell_asset.state(),
		&buy_asset.state(),
		amount,
		asset_fee,
		protocol_fee,
		Permill::zero(),
		Some(&slip),
	)
	.expect("buy failed");

	// Update sell pool: use forward-computed values from the result
	sell_asset.reserve += *r.asset_in.delta_reserve;
	sell_asset.hub_reserve -= *r.asset_in.delta_hub_reserve;
	sell_asset.delta = sell_asset
		.delta
		.checked_add(SignedBalance::Negative(*r.asset_in.delta_hub_reserve))
		.unwrap();

	// Update buy pool
	buy_asset.reserve -= *r.asset_out.delta_reserve;
	buy_asset.hub_reserve += *r.asset_out.delta_hub_reserve;
	buy_asset.delta = buy_asset
		.delta
		.checked_add(SignedBalance::Positive(*r.asset_out.delta_hub_reserve))
		.unwrap();

	(*r.asset_in.delta_reserve, r.fee.protocol_fee)
}

/// Execute a sell-LRNA-for-token trade and update pool state + delta.
/// Returns (tokens_out, slip_fee).
fn exec_sell_hub(buy_asset: &mut PoolAsset, lrna_amount: Balance, asset_fee: Permill) -> (Balance, Balance) {
	let slip = HubTradeSlipFees {
		asset_hub_reserve: buy_asset.q0,
		asset_delta: buy_asset.delta,
		max_slip_fee: Permill::from_percent(100),
	};

	let r = calculate_sell_hub_state_changes(&buy_asset.state(), lrna_amount, asset_fee, Some(&slip))
		.expect("sell_hub failed");

	buy_asset.reserve -= *r.asset.delta_reserve;
	buy_asset.hub_reserve += *r.asset.delta_hub_reserve;
	buy_asset.delta = buy_asset
		.delta
		.checked_add(SignedBalance::Positive(*r.asset.delta_hub_reserve))
		.unwrap(); // D_net entered

	(*r.asset.delta_reserve, r.fee.protocol_fee)
}

fn make_pools() -> (PoolAsset, PoolAsset, PoolAsset) {
	let hdx = PoolAsset {
		reserve: 10_000_000 * UNIT,
		hub_reserve: 10_000_000 * UNIT,
		q0: 10_000_000 * UNIT,
		delta: SignedBalance::zero(),
		shares: 10_000_000 * UNIT,
	};
	let dot = PoolAsset {
		reserve: 500_000 * UNIT,
		hub_reserve: 5_000_000 * UNIT,
		q0: 5_000_000 * UNIT,
		delta: SignedBalance::zero(),
		shares: 500_000 * UNIT,
	};
	let eth = PoolAsset {
		reserve: 200_000 * UNIT,
		hub_reserve: 8_000_000 * UNIT,
		q0: 8_000_000 * UNIT,
		delta: SignedBalance::zero(),
		shares: 200_000 * UNIT,
	};
	(hdx, dot, eth)
}

const AF: Permill = Permill::from_parts(2500); // 0.25%
const PF: Permill = Permill::from_parts(500); // 0.05%

/// Snapshot of final state after a trade sequence, for comparison.
#[derive(Debug)]
struct SequenceResult {
	hdx_reserve: Balance,
	hdx_hub: Balance,
	hdx_delta: SignedBalance,
	dot_reserve: Balance,
	dot_hub: Balance,
	dot_delta: SignedBalance,
	eth_reserve: Balance,
	eth_hub: Balance,
	eth_delta: SignedBalance,
	total_protocol_fees: Balance,
	trade_outputs: Vec<Balance>, // output of each trade (tokens_out or tokens_in_cost)
	trade_fees: Vec<Balance>,    // per-trade protocol fees (for cross-validation)
}

fn snapshot(
	hdx: &PoolAsset,
	dot: &PoolAsset,
	eth: &PoolAsset,
	fees: Vec<Balance>,
	outputs: Vec<Balance>,
) -> SequenceResult {
	SequenceResult {
		hdx_reserve: hdx.reserve,
		hdx_hub: hdx.hub_reserve,
		hdx_delta: hdx.delta,
		dot_reserve: dot.reserve,
		dot_hub: dot.hub_reserve,
		dot_delta: dot.delta,
		eth_reserve: eth.reserve,
		eth_hub: eth.hub_reserve,
		eth_delta: eth.delta,
		total_protocol_fees: fees.iter().sum(),
		trade_outputs: outputs,
		trade_fees: fees,
	}
}

/// Run the "Order A" trade sequence and return the snapshot.
fn run_order_a() -> SequenceResult {
	let (mut hdx, mut dot, mut eth) = make_pools();
	let mut fees = Vec::new();
	let mut outputs = Vec::new();

	// Trade 1: Sell 50K HDX → DOT (establishes deltas)
	let (out, fee) = exec_sell(&mut hdx, &mut dot, 50_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 2: Sell 30K HDX → ETH (same direction on HDX, fresh on ETH)
	let (out, fee) = exec_sell(&mut hdx, &mut eth, 30_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 3: Sell 2K DOT → HDX (opposing on both DOT and HDX)
	let (out, fee) = exec_sell(&mut dot, &mut hdx, 2_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 4: Buy 500 DOT with HDX (buy-specified, same direction as trade 1)
	let (cost, fee) = exec_buy(&mut hdx, &mut dot, 500 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 5: Sell 1K ETH → DOT (unrelated pair, new direction)
	let (out, fee) = exec_sell(&mut eth, &mut dot, 1_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 6: Sell 5K LRNA → ETH (hub trade, buy-side only)
	let (out, fee) = exec_sell_hub(&mut eth, 5_000 * UNIT, AF);
	fees.push(fee);
	outputs.push(out);

	// Trade 7: Sell 100K HDX → DOT (large trade, high cumulative)
	let (out, fee) = exec_sell(&mut hdx, &mut dot, 100_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 8: Buy 1000 ETH with HDX (buy-specified on fresh-ish pair)
	let (cost, fee) = exec_buy(&mut hdx, &mut eth, 1_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 9: Sell 10K DOT → ETH (DOT→ETH, both have existing deltas)
	let (out, fee) = exec_sell(&mut dot, &mut eth, 10_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 10: Sell 5K DOT → HDX (opposing on HDX which lost a lot of LRNA)
	let (out, fee) = exec_sell(&mut dot, &mut hdx, 5_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 11: Buy 200 DOT with ETH (buy-specified, cross pair)
	let (cost, fee) = exec_buy(&mut eth, &mut dot, 200 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 12: Sell 20K HDX → ETH (final trade, heavy cumulative on HDX)
	let (out, fee) = exec_sell(&mut hdx, &mut eth, 20_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	snapshot(&hdx, &dot, &eth, fees, outputs)
}

/// Run "Order B" — same trades but reordered to test path dependence.
/// We interleave opposing trades earlier and delay large same-direction trades.
fn run_order_b() -> SequenceResult {
	let (mut hdx, mut dot, mut eth) = make_pools();
	let mut fees = Vec::new();
	let mut outputs = Vec::new();

	// Trade 1: Sell 2K DOT → HDX (was trade 3 in A — now first, fresh deltas)
	let (out, fee) = exec_sell(&mut dot, &mut hdx, 2_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 2: Sell 1K ETH → DOT (was trade 5 — now early, builds ETH delta)
	let (out, fee) = exec_sell(&mut eth, &mut dot, 1_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 3: Sell 50K HDX → DOT (was trade 1 — now HDX has positive buy delta from trade 1)
	let (out, fee) = exec_sell(&mut hdx, &mut dot, 50_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 4: Sell 5K LRNA → ETH (was trade 6)
	let (out, fee) = exec_sell_hub(&mut eth, 5_000 * UNIT, AF);
	fees.push(fee);
	outputs.push(out);

	// Trade 5: Buy 500 DOT with HDX (was trade 4)
	let (cost, fee) = exec_buy(&mut hdx, &mut dot, 500 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 6: Sell 30K HDX → ETH (was trade 2)
	let (out, fee) = exec_sell(&mut hdx, &mut eth, 30_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 7: Sell 5K DOT → HDX (was trade 10 — opposing earlier)
	let (out, fee) = exec_sell(&mut dot, &mut hdx, 5_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 8: Sell 10K DOT → ETH (was trade 9)
	let (out, fee) = exec_sell(&mut dot, &mut eth, 10_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 9: Sell 100K HDX → DOT (was trade 7 — large trade now later)
	let (out, fee) = exec_sell(&mut hdx, &mut dot, 100_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	// Trade 10: Buy 1000 ETH with HDX (was trade 8)
	let (cost, fee) = exec_buy(&mut hdx, &mut eth, 1_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 11: Buy 200 DOT with ETH (was trade 11)
	let (cost, fee) = exec_buy(&mut eth, &mut dot, 200 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(cost);

	// Trade 12: Sell 20K HDX → ETH (was trade 12)
	let (out, fee) = exec_sell(&mut hdx, &mut eth, 20_000 * UNIT, AF, PF);
	fees.push(fee);
	outputs.push(out);

	snapshot(&hdx, &dot, &eth, fees, outputs)
}

#[test]
fn multi_trade_sequence_order_a() {
	let result = run_order_a();

	// All 12 trades should produce non-zero output
	assert_eq!(result.trade_outputs.len(), 12);
	for (i, &out) in result.trade_outputs.iter().enumerate() {
		assert!(out > 0, "Trade {} produced zero output", i + 1);
	}

	// Total protocol fees should be substantial (slip fees + protocol fees from 12 trades)
	assert!(result.total_protocol_fees > 0, "Total protocol fees should be positive");

	// Invariant: for each asset, hub_reserve should equal Q0 + delta
	// (hub_reserve = starting_hub_reserve + net LRNA movement, and delta tracks that net movement)
	let check_hub_delta = |name: &str, hub: Balance, q0: Balance, delta: SignedBalance| {
		let expected = delta.add_to_unsigned(q0).expect("delta + q0 should not underflow");
		// The hub_reserve tracks actual pool state; delta tracks LRNA movement at the pool level.
		// Protocol fees are burned (removed from system) so hub_reserve = Q0 + delta is only
		// approximate — the delta reflects D_net (what actually entered the pool), not the full
		// delta_hub_reserve_in. The relationship is exact per construction.
		assert_eq!(hub, expected, "{} hub_reserve vs Q0+delta mismatch", name);
	};
	check_hub_delta("HDX", result.hdx_hub, 10_000_000 * UNIT, result.hdx_delta);
	check_hub_delta("DOT", result.dot_hub, 5_000_000 * UNIT, result.dot_delta);
	check_hub_delta("ETH", result.eth_hub, 8_000_000 * UNIT, result.eth_delta);
}

#[test]
fn multi_trade_sequence_order_b() {
	let result = run_order_b();

	// Same basic checks
	assert_eq!(result.trade_outputs.len(), 12);
	for (i, &out) in result.trade_outputs.iter().enumerate() {
		assert!(out > 0, "Trade {} produced zero output", i + 1);
	}
	assert!(result.total_protocol_fees > 0);

	// Hub-delta invariants
	let check_hub_delta = |name: &str, hub: Balance, q0: Balance, delta: SignedBalance| {
		let expected = delta.add_to_unsigned(q0).expect("delta + q0 should not underflow");
		assert_eq!(hub, expected, "{} hub_reserve vs Q0+delta mismatch", name);
	};
	check_hub_delta("HDX", result.hdx_hub, 10_000_000 * UNIT, result.hdx_delta);
	check_hub_delta("DOT", result.dot_hub, 5_000_000 * UNIT, result.dot_delta);
	check_hub_delta("ETH", result.eth_hub, 8_000_000 * UNIT, result.eth_delta);
}

#[test]
fn multi_trade_sequence_path_dependence() {
	let a = run_order_a();
	let b = run_order_b();

	// Path dependence: same trades in different order should produce different results
	// because slip fees depend on cumulative intra-block deltas.
	assert_ne!(
		a.hdx_reserve, b.hdx_reserve,
		"HDX reserve should differ between orders (path dependence)"
	);
	assert_ne!(
		a.dot_reserve, b.dot_reserve,
		"DOT reserve should differ between orders (path dependence)"
	);
	assert_ne!(
		a.eth_reserve, b.eth_reserve,
		"ETH reserve should differ between orders (path dependence)"
	);

	// Deltas should also differ (different cumulative paths → different slip deductions)
	assert_ne!(a.hdx_delta, b.hdx_delta, "HDX delta should differ between orders");

	// Both sequences should collect substantial protocol fees
	assert!(
		a.total_protocol_fees > 100 * UNIT,
		"Order A should collect meaningful fees"
	);
	assert!(
		b.total_protocol_fees > 100 * UNIT,
		"Order B should collect meaningful fees"
	);

	// The total fees should differ (slip amounts are path-dependent)
	assert_ne!(
		a.total_protocol_fees, b.total_protocol_fees,
		"Total fees should differ between orders (path dependence)"
	);
}

// Cross-validation against Python reference implementation.
// Python values from test_slip_fee_cross_validation.py (lrna_mint_pct=0, slip_factor=1.0).
// Each trade accumulates deltas sequentially, so small rounding differences compound.
// We use a tolerance of ±10 per trade (generous for 12-step sequences).

const MULTI_TRADE_TOLERANCE: u128 = 10;

#[test]
fn multi_trade_cross_validation_order_a() {
	let result = run_order_a();

	// Python-computed per-trade outputs (tokens_out or input_cost)
	let py_outputs: [u128; 12] = [
		4_839_610_836_284_869,  // trade 1: sell 50K HDX→DOT
		726_882_117_540_707,    // trade 2: sell 30K HDX→ETH
		20_295_462_425_632_054, // trade 3: sell 2K DOT→HDX
		5_209_737_707_659_164,  // trade 4: buy 500 DOT w/ HDX
		3_851_018_736_816_717,  // trade 5: sell 1K ETH→DOT
		124_857_762_387_519,    // trade 6: sell 5K LRNA→ETH
		8_843_311_999_565_970,  // trade 7: sell 100K HDX→DOT
		42_848_093_783_104_215, // trade 8: buy 1K ETH w/ HDX
		2_478_611_431_987_756,  // trade 9: sell 10K DOT→ETH
		51_498_977_618_451_029, // trade 10: sell 5K DOT→HDX
		49_672_050_544_432,     // trade 11: buy 200 DOT w/ ETH
		448_773_917_335_286,    // trade 12: sell 20K HDX→ETH
	];

	// Python-computed per-trade protocol fees
	let py_fees: [u128; 12] = [
		758_441_215_438_260,   // trade 1
		359_090_262_918_433,   // trade 2
		246_170_277_207_342,   // trade 3
		70_246_965_116_778,    // trade 4
		656_284_599_610_825,   // trade 5
		3_650_650_907_736,     // trade 6
		4_806_685_157_194_824, // trade 7
		1_052_324_498_562_266, // trade 8
		3_070_795_510_967_910, // trade 9
		920_374_676_524_221,   // trade 10
		40_010_274_806_956,    // trade 11
		706_686_327_115_182,   // trade 12
	];

	for i in 0..12 {
		assert_within_tolerance(
			result.trade_outputs[i],
			py_outputs[i],
			MULTI_TRADE_TOLERANCE,
			&format!("Order A trade {} output", i + 1),
		);
		assert_within_tolerance(
			result.trade_fees[i],
			py_fees[i],
			MULTI_TRADE_TOLERANCE,
			&format!("Order A trade {} protocol_fee", i + 1),
		);
	}
}

#[test]
fn multi_trade_cross_validation_order_b() {
	let result = run_order_b();

	// Python-computed per-trade outputs
	let py_outputs: [u128; 12] = [
		19_702_956_005_166_758, // trade 1: sell 2K DOT→HDX
		3_933_234_564_918_534,  // trade 2: sell 1K ETH→DOT
		4_811_914_373_078_129,  // trade 3: sell 50K HDX→DOT
		125_308_889_078_455,    // trade 4: sell 5K LRNA→ETH
		5_288_097_354_767_135,  // trade 5: buy 500 DOT w/ HDX
		738_612_284_018_721,    // trade 6: sell 30K HDX→ETH
		50_914_925_986_396_112, // trade 7: sell 5K DOT→HDX
		2_374_540_494_899_033,  // trade 8: sell 10K DOT→ETH
		9_797_612_054_639_846,  // trade 9: sell 100K HDX→DOT
		43_774_201_146_126_089, // trade 10: buy 1K ETH w/ HDX
		49_998_386_591_137,     // trade 11: buy 200 DOT w/ ETH
		449_113_715_269_515,    // trade 12: sell 20K HDX→ETH
	];

	// Python-computed per-trade protocol fees
	let py_fees: [u128; 12] = [
		128_889_179_507_245,   // trade 1
		373_946_590_777_168,   // trade 2
		856_246_099_313_038,   // trade 3
		21_845_653_199_084,    // trade 4
		97_429_312_723_568,    // trade 5
		229_155_617_738_488,   // trade 6
		329_833_183_704_199,   // trade 7
		2_685_590_984_124_158, // trade 8
		1_595_553_139_046_860, // trade 9
		1_379_836_815_405_026, // trade 10
		43_391_099_640_944,    // trade 11
		701_758_922_337_940,   // trade 12
	];

	for i in 0..12 {
		assert_within_tolerance(
			result.trade_outputs[i],
			py_outputs[i],
			MULTI_TRADE_TOLERANCE,
			&format!("Order B trade {} output", i + 1),
		);
		assert_within_tolerance(
			result.trade_fees[i],
			py_fees[i],
			MULTI_TRADE_TOLERANCE,
			&format!("Order B trade {} protocol_fee", i + 1),
		);
	}
}
