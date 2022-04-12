use crate::types::BalanceUpdate::{Decrease, Increase};
use crate::types::{BalanceUpdate, SimpleImbalance};
use crate::{AssetState, Config, FixedU128};
use sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One, Zero};
use sp_runtime::FixedPointNumber;
use sp_std::default::Default;
use std::cmp::min;

#[derive(Default, Copy, Clone)]
pub(super) struct AssetStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) delta_reserve: BalanceUpdate<Balance>,
	pub(crate) delta_hub_reserve: BalanceUpdate<Balance>,
	pub(crate) delta_shares: BalanceUpdate<Balance>,
	pub(crate) delta_protocol_shares: BalanceUpdate<Balance>,
	pub(crate) delta_tvl: BalanceUpdate<Balance>,
}

#[derive(Default, Copy, Clone)]
pub(super) struct TradeStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) asset_in: AssetStateChange<Balance>,
	pub(crate) asset_out: AssetStateChange<Balance>,
	pub(crate) delta_imbalance: BalanceUpdate<Balance>,
	pub(crate) hdx_hub_amount: Balance,
}

#[derive(Default, Copy, Clone)]
pub(super) struct LiquidityStateChange<Balance>
where
	Balance: Default + Copy,
{
	pub(crate) asset: AssetStateChange<Balance>,
	pub(crate) delta_imbalance: BalanceUpdate<Balance>,
	pub(crate) delta_position_reserve: BalanceUpdate<Balance>,
	pub(crate) lp_hub_amount: Balance,
}

pub(crate) fn calculate_sell_state_changes<T: Config>(
	asset_in_state: &AssetState<T::Balance>,
	asset_out_state: &AssetState<T::Balance>,
	amount: T::Balance,
	asset_fee: FixedU128,
	protocol_fee: FixedU128,
	imbalance: &SimpleImbalance<T::Balance>,
) -> Option<TradeStateChange<T::Balance>> {
	let delta_hub_reserve_in = FixedU128::from((amount, (asset_in_state.reserve.checked_add(&amount)?)))
		.checked_mul_int(asset_in_state.hub_reserve)?;

	let fee_p = FixedU128::from(1).checked_sub(&protocol_fee)?;

	let delta_hub_reserve_out = fee_p.checked_mul_int(delta_hub_reserve_in)?;

	let fee_a = FixedU128::from(1).checked_sub(&asset_fee)?;

	let hub_reserve_out = asset_out_state.hub_reserve.checked_add(&delta_hub_reserve_out)?;

	let delta_reserve_out = FixedU128::from((delta_hub_reserve_out, hub_reserve_out))
		.checked_mul(&fee_a)
		.and_then(|v| v.checked_mul_int(asset_out_state.reserve))?;

	// Fee accounting
	let protocol_fee_amount = protocol_fee.checked_mul_int(delta_hub_reserve_in)?;

	// TODO: verify if this is correct min select ?! is protocol_fee_amoutn supposed to be negative ?
	let delta_imbalance = min(protocol_fee_amount, imbalance.value);

	let hdx_fee_amount = protocol_fee_amount.checked_sub(&delta_imbalance)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Decrease(delta_hub_reserve_in.checked_sub(&hdx_fee_amount)?),
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
pub(crate) fn calculate_sell_hub_state_changes<T: Config>(
	asset_out_state: &AssetState<T::Balance>,
	amount: T::Balance,
	asset_fee: FixedU128,
) -> Option<(AssetStateChange<T::Balance>, BalanceUpdate<T::Balance>)> {
	let fee_asset = FixedU128::from(1).checked_sub(&asset_fee)?;

	let hub_ratio = FixedU128::from((
		asset_out_state.hub_reserve,
		asset_out_state.hub_reserve.checked_add(&amount)?,
	));

	let delta_reserve_out = fee_asset
		.checked_mul(&FixedU128::from((
			amount,
			asset_out_state.hub_reserve.checked_add(&amount)?,
		)))?
		.checked_mul_int(asset_out_state.reserve)?;

	// Negative
	let delta_imbalance = fee_asset
		.checked_mul(&hub_ratio)?
		.checked_add(&FixedU128::one())?
		.checked_mul_int(amount)?;

	Some((
		AssetStateChange {
			delta_reserve: Decrease(delta_reserve_out),
			delta_hub_reserve: Increase(amount),
			..Default::default()
		},
		Decrease(delta_imbalance),
	))
}

pub(crate) fn calculate_buy_state_changes<T: Config>(
	asset_in_state: &AssetState<T::Balance>,
	asset_out_state: &AssetState<T::Balance>,
	amount: T::Balance,
	asset_fee: FixedU128,
	protocol_fee: FixedU128,
	imbalance: &SimpleImbalance<T::Balance>,
) -> Option<TradeStateChange<T::Balance>> {
	// Positive
	let fee_asset = FixedU128::from(1).checked_sub(&asset_fee)?;
	let fee_protocol = FixedU128::from(1).checked_sub(&protocol_fee)?;

	let delta_hub_reserve_out = FixedU128::from((
		amount,
		fee_asset
			.checked_mul_int(asset_out_state.reserve)?
			.checked_sub(&amount)?,
	))
	.checked_mul_int(asset_out_state.hub_reserve)?;

	// Negative
	let delta_hub_reserve_in: T::Balance = FixedU128::from_inner(delta_hub_reserve_out.into())
		.checked_div(&fee_protocol)?
		.into_inner()
		.into();

	// Positive
	let delta_reserve_in = FixedU128::from((
		delta_hub_reserve_in,
		asset_in_state.hub_reserve.checked_sub(&delta_hub_reserve_in)?,
	))
	.checked_mul_int(asset_in_state.reserve)?;

	// Fee accounting and imbalance
	let protocol_fee_amount = protocol_fee.checked_mul_int(delta_hub_reserve_in)?;
	let delta_imbalance = min(protocol_fee_amount, imbalance.value);

	let hdx_fee_amount = protocol_fee_amount.checked_sub(&delta_imbalance)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(delta_reserve_in),
			delta_hub_reserve: Decrease(delta_hub_reserve_in.checked_sub(&hdx_fee_amount)?),
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

pub(crate) fn calculate_add_liquidity_state_changes<T: Config>(
	asset_state: &AssetState<T::Balance>,
	amount: T::Balance,
) -> Option<LiquidityStateChange<T::Balance>> {
	let delta_hub_reserve = asset_state.price().checked_mul_int(amount)?;

	let new_reserve = asset_state.reserve.checked_add(&amount)?;

	let new_shares = FixedU128::from((asset_state.shares, asset_state.reserve)).checked_mul_int(new_reserve)?;

	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Increase(delta_hub_reserve),
			delta_shares: Increase(new_shares.checked_sub(&asset_state.shares)?),
			..Default::default()
		},
		delta_imbalance: BalanceUpdate::Decrease(amount),
		..Default::default()
	})
}

pub(crate) fn calculate_remove_liquidity_state_changes<T: Config>(
	asset_state: &AssetState<T::Balance>,
	shares_removed: T::Balance,
	position_price: FixedU128,
) -> Option<LiquidityStateChange<T::Balance>> {
	let current_shares = asset_state.shares;
	let current_reserve = asset_state.reserve;
	let current_hub_reserve = asset_state.hub_reserve;

	let current_price = asset_state.price();

	// Protocol shares update
	let delta_b = if current_price < position_price {
		let sum = current_price.checked_add(&position_price)?;
		let sub = position_price.checked_sub(&current_price)?;

		sub.checked_div(&sum).and_then(|v| v.checked_mul_int(shares_removed))?
	} else {
		T::Balance::zero()
	};

	let delta_shares = shares_removed.checked_sub(&delta_b)?;

	let delta_reserve = FixedU128::from((current_reserve, current_shares)).checked_mul_int(delta_shares)?;

	let delta_hub_reserve = FixedU128::from((delta_reserve, current_reserve)).checked_mul_int(current_hub_reserve)?;

	let hub_transferred = if current_price > position_price {
		// LP receives some hub asset

		// delta_q_a = -pi * ( 2pi / (pi + pa) * delta_s_a / Si * Ri + delta_r_a )
		// note: delta_s_a is < 0

		let price_sum = current_price.checked_add(&position_price)?;

		let double_current_price = current_price.checked_mul(&FixedU128::from(2))?;

		let p1 = double_current_price.checked_div(&price_sum)?;

		let p2 = FixedU128::from((shares_removed, current_shares));

		let p3 = p1.checked_mul(&p2).and_then(|v| v.checked_mul_int(current_reserve))?;

		current_price.checked_mul_int(p3.checked_sub(&delta_reserve)?)?
	} else {
		T::Balance::zero()
	};
	let delta_r_position =
		FixedU128::from((asset_state.reserve, asset_state.shares)).checked_mul_int(shares_removed)?;
	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(delta_reserve),
			delta_hub_reserve: Decrease(delta_hub_reserve),
			delta_shares: Decrease(delta_shares),
			delta_protocol_shares: Decrease(delta_b),
			..Default::default()
		},
		delta_imbalance: Increase(delta_reserve),
		lp_hub_amount: hub_transferred,
		delta_position_reserve: Decrease(delta_r_position),
	})
}

// THe following module will be eventually moved into the math crate.
pub mod hydradx_math {

	#[allow(unused)]
	fn calculate_out_given_in<Balance: Default>() -> Balance {
		Balance::default()
	}

	#[allow(unused)]
	fn calculate_in_given_out<Balance: Default>() -> Balance {
		Balance::default()
	}

	#[allow(unused)]
	fn calculate_shares_given_liquidity_in<Balance: Default>() -> Balance {
		Balance::default()
	}
}
