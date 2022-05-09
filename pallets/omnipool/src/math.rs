use crate::types::BalanceUpdate::{Decrease, Increase};
use crate::types::{
	AssetStateChange, Balance, BalanceUpdate, HubTradeStateChange, LiquidityStateChange, Position, SimpleImbalance,
	TradeStateChange,
};
use crate::{AssetState, FixedU128, Price};
use primitive_types::U256;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Zero};
use sp_runtime::FixedPointNumber;
use sp_std::cmp::{min, Ordering};
use sp_std::default::Default;

#[macro_export]
macro_rules! to_u256 {
    ($($x:expr),+) => (
        {($(U256::from($x)),+)}
    );
}

#[macro_export]
macro_rules! to_balance {
	($x:expr) => {
		Balance::try_from($x).ok()
	};
}

#[inline]
fn amount_without_fee(amount: Balance, fee: FixedU128) -> Option<Balance> {
	let fee_amount = fee.checked_mul_int(amount)?;
	amount.checked_sub(fee_amount)
}

/// Calculate delta changes of a sell trade given current state of asset in and out.
pub(crate) fn calculate_sell_state_changes(
	asset_in_state: &AssetState<Balance>,
	asset_out_state: &AssetState<Balance>,
	amount: Balance,
	asset_fee: FixedU128,
	protocol_fee: FixedU128,
	imbalance: &SimpleImbalance<Balance>,
) -> Option<TradeStateChange<Balance>> {
	let (in_hub_reserve, in_reserve, in_amount) = to_u256!(asset_in_state.hub_reserve, asset_in_state.reserve, amount);

	let delta_hub_reserve_in = in_amount
		.checked_mul(in_hub_reserve)
		.and_then(|v| v.checked_div(in_reserve.checked_add(in_amount)?))?;

	let delta_hub_reserve_in = to_balance!(delta_hub_reserve_in)?;

	let delta_hub_reserve_out = amount_without_fee(delta_hub_reserve_in, protocol_fee)?;

	let (out_reserve_hp, out_hub_reserve_hp, delta_hub_reserve_out_hp) = to_u256!(
		asset_out_state.reserve,
		asset_out_state.hub_reserve,
		delta_hub_reserve_out
	);

	let delta_reserve_out = out_reserve_hp
		.checked_mul(delta_hub_reserve_out_hp)
		.and_then(|v| v.checked_div(out_hub_reserve_hp.checked_add(delta_hub_reserve_out_hp)?))?;

	let delta_reserve_out = amount_without_fee(to_balance!(delta_reserve_out)?, asset_fee)?;

	// Fee accounting
	let protocol_fee_amount = protocol_fee.checked_mul_int(delta_hub_reserve_in)?;

	let delta_imbalance = min(protocol_fee_amount, imbalance.value);

	let hdx_fee_amount = protocol_fee_amount.checked_sub(delta_imbalance)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Decrease(delta_hub_reserve_in),
			..Default::default()
		},
		asset_out: AssetStateChange {
			delta_reserve: Decrease(delta_reserve_out),
			delta_hub_reserve: Increase(delta_hub_reserve_out),
			..Default::default()
		},
		delta_imbalance: BalanceUpdate::Decrease(delta_imbalance),
		hdx_hub_amount: hdx_fee_amount,
	})
}

/// Calculate delta changes of a sell where asset_in is Hub Asset
pub(crate) fn calculate_sell_hub_state_changes(
	asset_out_state: &AssetState<Balance>,
	hub_asset_amount: Balance,
	asset_fee: FixedU128,
) -> Option<HubTradeStateChange<Balance>> {
	let (reserve_hp, hub_reserve_hp, amount_hp) =
		to_u256!(asset_out_state.reserve, asset_out_state.hub_reserve, hub_asset_amount);

	let delta_reserve_out = reserve_hp * amount_hp / (hub_reserve_hp + amount_hp);

	let delta_reserve_out = to_balance!(delta_reserve_out)?;

	let delta_reserve_out = amount_without_fee(delta_reserve_out, asset_fee)?;

	let hub_imbalance = amount_hp * hub_reserve_hp / (hub_reserve_hp + amount_hp);
	let hub_imbalance = to_balance!(hub_imbalance)?;

	// Negative
	let delta_imbalance = amount_without_fee(hub_imbalance, asset_fee).and_then(|v| v.checked_add(hub_asset_amount))?;

	Some(HubTradeStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(delta_reserve_out),
			delta_hub_reserve: Increase(hub_asset_amount),
			..Default::default()
		},
		delta_imbalance: Decrease(delta_imbalance),
	})
}

/// Calculate delta changes of a buy trade where asset_in is Hub Asset
pub(crate) fn calculate_buy_for_hub_asset_state_changes(
	asset_out_state: &AssetState<Balance>,
	asset_out_amount: Balance,
	asset_fee: FixedU128,
) -> Option<HubTradeStateChange<Balance>> {
	let hub_denominator = amount_without_fee(asset_out_state.reserve, asset_fee)?.checked_sub(asset_out_amount)?;

	let (hub_reserve_hp, amount_hp, hub_denominator_hp) =
		to_u256!(asset_out_state.hub_reserve, asset_out_amount, hub_denominator);

	let delta_hub_reserve_hp = (hub_reserve_hp * amount_hp / hub_denominator_hp) + 1;

	let hub_imbalance = delta_hub_reserve_hp * hub_reserve_hp / (hub_reserve_hp + delta_hub_reserve_hp);

	let delta_hub_reserve = to_balance!(delta_hub_reserve_hp)?;
	let hub_imbalance = to_balance!(hub_imbalance)?;

	// Negative
	let delta_imbalance =
		amount_without_fee(hub_imbalance, asset_fee).and_then(|v| v.checked_add(delta_hub_reserve))?;

	Some(HubTradeStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(asset_out_amount),
			delta_hub_reserve: Increase(delta_hub_reserve),
			..Default::default()
		},
		delta_imbalance: Decrease(delta_imbalance),
	})
}

/// Calculate delta changes of a buy trade given current state of asset in and out
pub(crate) fn calculate_buy_state_changes(
	asset_in_state: &AssetState<Balance>,
	asset_out_state: &AssetState<Balance>,
	amount: Balance,
	asset_fee: FixedU128,
	protocol_fee: FixedU128,
	imbalance: &SimpleImbalance<Balance>,
) -> Option<TradeStateChange<Balance>> {
	let reserve_no_fee = amount_without_fee(asset_out_state.reserve, asset_fee)?;
	let (out_hub_reserve, out_reserve_no_fee, out_amount) =
		to_u256!(asset_out_state.hub_reserve, reserve_no_fee, amount);

	let delta_hub_reserve_out = out_hub_reserve
		.checked_mul(out_amount)
		.and_then(|v| v.checked_div(out_reserve_no_fee.checked_sub(out_amount)?))?;

	let delta_hub_reserve_out = to_balance!(delta_hub_reserve_out)?;
	let delta_hub_reserve_out = delta_hub_reserve_out.checked_add(Balance::one())?;

	// Negative
	let delta_hub_reserve_in: Balance = FixedU128::from_inner(delta_hub_reserve_out)
		.checked_div(&FixedU128::from(1).checked_sub(&protocol_fee)?)?
		.into_inner();

	let (delta_hub_reserve_in_hp, in_hub_reserve_hp, in_reserve_hp) =
		to_u256!(delta_hub_reserve_in, asset_in_state.hub_reserve, asset_in_state.reserve);

	let delta_reserve_in = in_reserve_hp
		.checked_mul(delta_hub_reserve_in_hp)
		.and_then(|v| v.checked_div(in_hub_reserve_hp.checked_sub(delta_hub_reserve_in_hp)?))?;

	let delta_reserve_in = to_balance!(delta_reserve_in)?;
	let delta_reserve_in = delta_reserve_in.checked_add(Balance::one())?;

	// Fee accounting and imbalance
	let protocol_fee_amount = protocol_fee.checked_mul_int(delta_hub_reserve_in)?;
	let delta_imbalance = min(protocol_fee_amount, imbalance.value);

	let hdx_fee_amount = protocol_fee_amount.checked_sub(delta_imbalance)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(delta_reserve_in),
			delta_hub_reserve: Decrease(delta_hub_reserve_in),
			..Default::default()
		},
		asset_out: AssetStateChange {
			delta_reserve: Decrease(amount),
			delta_hub_reserve: Increase(delta_hub_reserve_out),
			..Default::default()
		},
		delta_imbalance: BalanceUpdate::Decrease(delta_imbalance),
		hdx_hub_amount: hdx_fee_amount,
	})
}

/// Calculate delta changes of add liqudiity given current asset state
pub(crate) fn calculate_add_liquidity_state_changes(
	asset_state: &AssetState<Balance>,
	amount: Balance,
	stable_asset: (Balance, Balance),
) -> Option<LiquidityStateChange<Balance>> {
	let delta_hub_reserve = asset_state.price().checked_mul_int(amount)?;

	let new_reserve = asset_state.reserve.checked_add(amount)?;

	let new_shares =
		FixedU128::checked_from_rational(asset_state.shares, asset_state.reserve)?.checked_mul_int(new_reserve)?;

	let adjusted_asset_tvl = FixedU128::checked_from_rational(stable_asset.0, stable_asset.1)?
		.checked_mul_int(asset_state.hub_reserve.checked_add(delta_hub_reserve)?)?;

	let delta_tvl = match adjusted_asset_tvl.cmp(&asset_state.tvl) {
		Ordering::Greater => BalanceUpdate::Increase(adjusted_asset_tvl.checked_sub(asset_state.tvl)?),
		Ordering::Less => BalanceUpdate::Decrease(asset_state.tvl.checked_sub(adjusted_asset_tvl)?),
		Ordering::Equal => BalanceUpdate::Increase(Balance::zero()),
	};

	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Increase(delta_hub_reserve),
			delta_shares: Increase(new_shares.checked_sub(asset_state.shares)?),
			delta_tvl,
			..Default::default()
		},
		delta_imbalance: BalanceUpdate::Decrease(amount),
		..Default::default()
	})
}

/// Calculate delta changes of rmove liqudiity given current asset state and position from which liquidity should be removed.
pub(crate) fn calculate_remove_liquidity_state_changes<AssetId>(
	asset_state: &AssetState<Balance>,
	shares_removed: Balance,
	position: &Position<Balance, AssetId>,
	stable_asset: (Balance, Balance),
) -> Option<LiquidityStateChange<Balance>> {
	let current_shares = asset_state.shares;
	let current_reserve = asset_state.reserve;
	let current_hub_reserve = asset_state.hub_reserve;

	let current_price = asset_state.price();
	let position_price = Price::from_inner(position.price);

	// Protocol shares update
	let delta_b = if current_price < position_price {
		let sum = current_price.checked_add(&position_price)?;
		let sub = position_price.checked_sub(&current_price)?;

		sub.checked_div(&sum).and_then(|v| v.checked_mul_int(shares_removed))?
	} else {
		Balance::zero()
	};

	let delta_shares = shares_removed.checked_sub(delta_b)?;

	let delta_reserve =
		FixedU128::checked_from_rational(current_reserve, current_shares)?.checked_mul_int(delta_shares)?;

	let delta_hub_reserve =
		FixedU128::checked_from_rational(delta_reserve, current_reserve)?.checked_mul_int(current_hub_reserve)?;

	let hub_transferred = if current_price > position_price {
		// LP receives some hub asset

		// delta_q_a = -pi * ( 2pi / (pi + pa) * delta_s_a / Si * Ri + delta_r_a )
		// note: delta_s_a is < 0

		let price_sum = current_price.checked_add(&position_price)?;

		let double_current_price = current_price.checked_mul(&FixedU128::from(2))?;

		let p1 = double_current_price.checked_div(&price_sum)?;

		let p2 = FixedU128::checked_from_rational(shares_removed, current_shares)?;

		let p3 = p1.checked_mul(&p2).and_then(|v| v.checked_mul_int(current_reserve))?;

		current_price.checked_mul_int(p3.checked_sub(delta_reserve)?)?
	} else {
		Balance::zero()
	};
	let delta_r_position =
		FixedU128::checked_from_rational(shares_removed, position.shares)?.checked_mul_int(position.amount)?;

	let adjusted_asset_tvl = FixedU128::checked_from_rational(stable_asset.0, stable_asset.1)?
		.checked_mul_int(asset_state.hub_reserve.checked_sub(delta_hub_reserve)?)?;

	let delta_tvl = match adjusted_asset_tvl.cmp(&asset_state.tvl) {
		Ordering::Greater => BalanceUpdate::Increase(adjusted_asset_tvl.checked_sub(asset_state.tvl)?),
		Ordering::Less => BalanceUpdate::Decrease(asset_state.tvl.checked_sub(adjusted_asset_tvl)?),
		Ordering::Equal => BalanceUpdate::Increase(Balance::zero()),
	};

	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(delta_reserve),
			delta_hub_reserve: Decrease(delta_hub_reserve),
			delta_shares: Decrease(delta_shares),
			delta_protocol_shares: Increase(delta_b),
			delta_tvl,
		},
		delta_imbalance: Increase(delta_reserve),
		lp_hub_amount: hub_transferred,
		delta_position_reserve: Decrease(delta_r_position),
		delta_position_shares: Decrease(shares_removed),
	})
}
