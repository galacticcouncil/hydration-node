use crate::omnipool::types::BalanceUpdate::{Decrease, Increase};
use crate::omnipool::types::{
	AssetReserveState, AssetStateChange, HubTradeStateChange, LiquidityStateChange, Position, TradeFee,
	TradeStateChange,
};
use crate::types::Balance;
use crate::MathError::Overflow;
use crate::{to_balance, to_u256};
use num_traits::{CheckedDiv, CheckedMul, CheckedSub, One, Zero};
use primitive_types::U256;
use sp_arithmetic::traits::Saturating;
use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};
use sp_std::ops::{Div, Sub};

#[inline]
fn amount_without_fee(amount: Balance, fee: Permill) -> Option<Balance> {
	Some(Permill::from_percent(100).checked_sub(&fee)?.mul_floor(amount))
}

/// Calculate delta changes of a sell trade given current state of asset in and out.
pub fn calculate_sell_state_changes(
	asset_in_state: &AssetReserveState<Balance>,
	asset_out_state: &AssetReserveState<Balance>,
	amount: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
	m: Permill,
) -> Option<TradeStateChange<Balance>> {
	let (in_hub_reserve, in_reserve, in_amount) = to_u256!(asset_in_state.hub_reserve, asset_in_state.reserve, amount);

	let delta_hub_reserve_in = in_amount
		.checked_mul(in_hub_reserve)
		.and_then(|v| v.checked_div(in_reserve.checked_add(in_amount)?))?;

	let delta_hub_reserve_in = to_balance!(delta_hub_reserve_in).ok()?;

	let protocol_fee_amount = protocol_fee.mul_floor(delta_hub_reserve_in);

	let delta_hub_reserve_out = delta_hub_reserve_in.checked_sub(protocol_fee_amount)?;

	let (out_reserve_hp, out_hub_reserve_hp, delta_hub_reserve_out_hp) = to_u256!(
		asset_out_state.reserve,
		asset_out_state.hub_reserve,
		delta_hub_reserve_out
	);

	let delta_reserve_out_hp = out_reserve_hp
		.checked_mul(delta_hub_reserve_out_hp)
		.and_then(|v| v.checked_div(out_hub_reserve_hp.checked_add(delta_hub_reserve_out_hp)?))?;

	let amount_out = to_balance!(delta_reserve_out_hp).ok()?;
	let delta_reserve_out = amount_without_fee(amount_out, asset_fee)?;

	let asset_fee_amount = amount_out.saturating_sub(delta_reserve_out);

	//mint amount for asset out
	let delta_out_m = asset_fee.mul_floor(
		to_balance!(out_hub_reserve_hp
			.checked_add(delta_hub_reserve_out_hp)?
			.checked_mul(delta_hub_reserve_out_hp)?
			.checked_div(out_hub_reserve_hp)?)
		.ok()?,
	);

	// burn part of protocol fee and rest is to be transferred to treasury or buybacks
	// note that we dont need to include burned amount anywhere, as it is already part of delta_hub_reserve_in value.
	// we only to need to include extra_protocol_fee when the deltas are calculated, as it used to be done for hdx hub amount.
	let burned_protocol_fee = m.mul_floor(protocol_fee_amount);
	let extra_protocol_fee = protocol_fee_amount.checked_sub(burned_protocol_fee)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Decrease(delta_hub_reserve_in),
			..Default::default()
		},
		asset_out: AssetStateChange {
			delta_reserve: Decrease(delta_reserve_out),
			delta_hub_reserve: Increase(delta_hub_reserve_out),
			extra_hub_reserve_amount: Increase(delta_out_m),
			..Default::default()
		},
		extra_protocol_fee_amount: extra_protocol_fee,
		fee: TradeFee {
			asset_fee: asset_fee_amount,
			protocol_fee: protocol_fee_amount,
		},
	})
}

/// Calculate delta changes of a sell where asset_in is Hub Asset
pub fn calculate_sell_hub_state_changes(
	asset_out_state: &AssetReserveState<Balance>,
	hub_asset_amount: Balance,
	asset_fee: Permill,
) -> Option<HubTradeStateChange<Balance>> {
	let (reserve_hp, hub_reserve_hp, amount_hp) =
		to_u256!(asset_out_state.reserve, asset_out_state.hub_reserve, hub_asset_amount);

	let delta_reserve_out_hp = reserve_hp
		.checked_mul(amount_hp)
		.and_then(|v| v.checked_div(hub_reserve_hp.checked_add(amount_hp)?))?;

	let amount_out = to_balance!(delta_reserve_out_hp).ok()?;
	let delta_reserve_out = amount_without_fee(amount_out, asset_fee)?;
	let asset_fee_amount = amount_out.saturating_sub(delta_reserve_out);

	// mint hub asset
	let delta_q_m = asset_fee.mul_floor(
		to_balance!(hub_reserve_hp
			.checked_add(amount_hp)?
			.checked_mul(amount_hp)?
			.checked_div(hub_reserve_hp)?)
		.ok()?,
	);

	Some(HubTradeStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(delta_reserve_out),
			delta_hub_reserve: Increase(hub_asset_amount),
			extra_hub_reserve_amount: Increase(delta_q_m),
			..Default::default()
		},
		fee: TradeFee {
			asset_fee: asset_fee_amount,
			..Default::default()
		},
	})
}

#[inline]
pub(crate) fn calculate_fee_amount_for_buy(fee: Permill, amount: Balance) -> Balance {
	if fee.is_zero() {
		return Balance::zero();
	}
	if fee == Permill::one() {
		return amount;
	}

	let (numerator, denominator) = (fee.deconstruct() as u128, 1_000_000u128);
	// Already handled but just in case, so div is safe safe. this is 100%
	if numerator == denominator {
		return amount;
	}
	// Round up
	numerator
		.saturating_mul(amount)
		.div(denominator.saturating_sub(numerator))
		.saturating_add(Balance::one())
}

/// Calculate delta changes of a buy trade where asset_in is Hub Asset
pub fn calculate_buy_for_hub_asset_state_changes(
	asset_out_state: &AssetReserveState<Balance>,
	asset_out_amount: Balance,
	asset_fee: Permill,
) -> Option<HubTradeStateChange<Balance>> {
	let reserve_no_fee = amount_without_fee(asset_out_state.reserve, asset_fee)?;
	let hub_denominator = reserve_no_fee.checked_sub(asset_out_amount)?;

	let (hub_reserve_hp, amount_hp, hub_denominator_hp) =
		to_u256!(asset_out_state.hub_reserve, asset_out_amount, hub_denominator);

	let delta_hub_reserve_hp = hub_reserve_hp.checked_mul(amount_hp).and_then(|v| {
		v.checked_div(hub_denominator_hp)
			.and_then(|v| v.checked_add(U256::one()))
	})?;

	let delta_hub_reserve = to_balance!(delta_hub_reserve_hp).ok()?;
	let fee_amount = calculate_fee_amount_for_buy(asset_fee, asset_out_amount);

	// mint hub asset
	let n = asset_fee.mul_floor(
		to_balance!(hub_reserve_hp
			.checked_add(delta_hub_reserve_hp)?
			.checked_mul(amount_hp)?)
		.ok()?,
	);
	let delta_q_m = n.checked_div(hub_denominator)?;

	Some(HubTradeStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(asset_out_amount),
			delta_hub_reserve: Increase(delta_hub_reserve),
			extra_hub_reserve_amount: Increase(delta_q_m),
			..Default::default()
		},
		fee: TradeFee {
			asset_fee: fee_amount,
			..Default::default()
		},
	})
}

/// Calculate delta changes of a buy trade given current state of asset in and out
pub fn calculate_buy_state_changes(
	asset_in_state: &AssetReserveState<Balance>,
	asset_out_state: &AssetReserveState<Balance>,
	amount: Balance,
	asset_fee: Permill,
	protocol_fee: Permill,
	m: Permill,
) -> Option<TradeStateChange<Balance>> {
	let reserve_no_fee = amount_without_fee(asset_out_state.reserve, asset_fee)?;
	let (out_hub_reserve_hp, out_reserve_no_fee_hp, out_amount_hp) =
		to_u256!(asset_out_state.hub_reserve, reserve_no_fee, amount);

	let delta_hub_reserve_out_hp = out_hub_reserve_hp
		.checked_mul(out_amount_hp)
		.and_then(|v| v.checked_div(out_reserve_no_fee_hp.checked_sub(out_amount_hp)?))?;

	//TODO: verify weith colin - this make sell and buy exact amounts
	//let delta_hub_reserve_out_hp = delta_hub_reserve_out_hp.checked_add(U256::one())?;

	let delta_hub_reserve_out = to_balance!(delta_hub_reserve_out_hp).ok()?;
	let delta_hub_reserve_out = delta_hub_reserve_out.checked_add(Balance::one())?;

	// Negative
	let delta_hub_reserve_in: Balance = FixedU128::from_inner(delta_hub_reserve_out)
		.checked_div(&Permill::from_percent(100).sub(protocol_fee).into())?
		.into_inner();

	if delta_hub_reserve_in >= asset_in_state.hub_reserve {
		return None;
	}

	let (delta_hub_reserve_in_hp, in_hub_reserve_hp, in_reserve_hp) =
		to_u256!(delta_hub_reserve_in, asset_in_state.hub_reserve, asset_in_state.reserve);

	let delta_reserve_in = in_reserve_hp
		.checked_mul(delta_hub_reserve_in_hp)
		.and_then(|v| v.checked_div(in_hub_reserve_hp.checked_sub(delta_hub_reserve_in_hp)?))?;

	let delta_reserve_in = to_balance!(delta_reserve_in).ok()?;
	let delta_reserve_in = delta_reserve_in.checked_add(Balance::one())?;

	let asset_fee_amount = calculate_fee_amount_for_buy(asset_fee, amount);
	let protocol_fee_amount = protocol_fee.mul_floor(delta_hub_reserve_in);

	// mint amount for asset out
	let delta_hub_reserve_out_hp = to_u256!(delta_hub_reserve_out);
	let delta_out_m = asset_fee.mul_floor(
		to_balance!(out_hub_reserve_hp
			.checked_add(delta_hub_reserve_out_hp)?
			.checked_mul(delta_hub_reserve_out_hp)?
			.checked_div(out_hub_reserve_hp)?)
		.ok()?,
	);

	// Protocol fee burn and transfer
	let burned_protocol_fee = m.mul_floor(protocol_fee_amount);
	let extra_protocol_fee = protocol_fee_amount.checked_sub(burned_protocol_fee)?;

	Some(TradeStateChange {
		asset_in: AssetStateChange {
			delta_reserve: Increase(delta_reserve_in),
			delta_hub_reserve: Decrease(delta_hub_reserve_in),
			..Default::default()
		},
		asset_out: AssetStateChange {
			delta_reserve: Decrease(amount),
			delta_hub_reserve: Increase(delta_hub_reserve_out),
			extra_hub_reserve_amount: Increase(delta_out_m),
			..Default::default()
		},
		extra_protocol_fee_amount: extra_protocol_fee,
		fee: TradeFee {
			asset_fee: asset_fee_amount,
			protocol_fee: protocol_fee_amount,
		},
	})
}

/// Calculate delta changes of add liqudiity given current asset state
pub fn calculate_add_liquidity_state_changes(
	asset_state: &AssetReserveState<Balance>,
	amount: Balance,
) -> Option<LiquidityStateChange<Balance>> {
	let delta_hub_reserve = asset_state.price()?.checked_mul_int(amount)?;

	let (amount_hp, shares_hp, reserve_hp) = to_u256!(amount, asset_state.shares, asset_state.reserve);

	let delta_shares_hp = shares_hp
		.checked_mul(amount_hp)
		.and_then(|v| v.checked_div(reserve_hp))?;

	let delta_shares = to_balance!(delta_shares_hp).ok()?;

	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Increase(amount),
			delta_hub_reserve: Increase(delta_hub_reserve),
			delta_shares: Increase(delta_shares),
			..Default::default()
		},
		..Default::default()
	})
}

/// Calculate withdrawal fee given current spot price and oracle price.
pub fn calculate_withdrawal_fee(
	spot_price: FixedU128,
	oracle_price: FixedU128,
	min_withdrawal_fee: Permill,
) -> FixedU128 {
	let price_diff = if oracle_price <= spot_price {
		spot_price.saturating_sub(oracle_price)
	} else {
		oracle_price.saturating_sub(spot_price)
	};

	let min_fee: FixedU128 = min_withdrawal_fee.into();
	debug_assert!(min_fee <= FixedU128::one());

	if oracle_price.is_zero() {
		return min_fee;
	}

	price_diff.div(oracle_price).clamp(min_fee, FixedU128::one())
}

/// Calculate delta changes of remove liqudiity given current asset state and position from which liquidity should be removed.
pub fn calculate_remove_liquidity_state_changes(
	asset_state: &AssetReserveState<Balance>,
	shares_removed: Balance,
	position: &Position<Balance>,
	withdrawal_fee: FixedU128,
) -> Option<LiquidityStateChange<Balance>> {
	let current_shares = asset_state.shares;
	let current_reserve = asset_state.reserve;
	let current_hub_reserve = asset_state.hub_reserve;

	let current_price = asset_state.price()?;
	let position_price = position.price()?;

	let (
		current_reserve_hp,
		current_hub_reserve_hp,
		current_shares_hp,
		shares_removed_hp,
		position_amount_hp,
		position_shares_hp,
	) = to_u256!(
		current_reserve,
		current_hub_reserve,
		current_shares,
		shares_removed,
		position.amount,
		position.shares
	);

	let p_x_r = U256::from(position_price.checked_mul_int(current_reserve)?).checked_add(U256::one())?;

	// Protocol shares update
	let delta_b_hp = if current_price < position_price {
		let numer = p_x_r
			.checked_sub(current_hub_reserve_hp)?
			.checked_mul(shares_removed_hp)?;
		let denom = p_x_r.checked_add(current_hub_reserve_hp)?;
		numer.checked_div(denom)?.checked_add(U256::one())? // round up
	} else {
		U256::from(Balance::zero())
	};

	let delta_shares_hp = shares_removed_hp.checked_sub(delta_b_hp)?;

	let delta_reserve_hp = current_reserve_hp
		.checked_mul(delta_shares_hp)
		.and_then(|v| v.checked_div(current_shares_hp))?;
	let delta_hub_reserve_hp = delta_reserve_hp
		.checked_mul(current_hub_reserve_hp)
		.and_then(|v| v.checked_div(current_reserve_hp))?;

	let delta_position_amount_hp = shares_removed_hp
		.checked_mul(position_amount_hp)
		.and_then(|v| v.checked_div(position_shares_hp))?;

	let delta_reserve = to_balance!(delta_reserve_hp).ok()?;
	let delta_hub_reserve = to_balance!(delta_hub_reserve_hp).ok()?;
	let delta_position_amount = to_balance!(delta_position_amount_hp).ok()?;
	let delta_shares = to_balance!(delta_shares_hp).ok()?;
	let delta_b = to_balance!(delta_b_hp).ok()?;

	let hub_transferred = if current_price > position_price {
		// LP receives some hub asset

		// delta_q_a = -pi * ( 2pi / (pi + pa) * delta_s_a / Si * Ri + delta_r_a )
		// note: delta_s_a is < 0

		let sub = current_hub_reserve_hp.checked_sub(p_x_r)?;
		let sum = current_hub_reserve_hp.checked_add(p_x_r)?;
		let div1 = current_hub_reserve_hp.checked_mul(sub)?.checked_div(sum)?;
		to_balance!(div1.checked_mul(delta_shares_hp)?.checked_div(current_shares_hp)?).ok()?
	} else {
		Balance::zero()
	};

	let fee_complement = FixedU128::one().saturating_sub(withdrawal_fee);

	// Apply withdrawal fee
	let delta_reserve = fee_complement.checked_mul_int(delta_reserve)?;
	let delta_hub_reserve = fee_complement.checked_mul_int(delta_hub_reserve)?;
	let hub_transferred = fee_complement.checked_mul_int(hub_transferred)?;

	Some(LiquidityStateChange {
		asset: AssetStateChange {
			delta_reserve: Decrease(delta_reserve),
			delta_hub_reserve: Decrease(delta_hub_reserve),
			delta_shares: Decrease(delta_shares),
			delta_protocol_shares: Increase(delta_b),
			..Default::default()
		},
		lp_hub_amount: hub_transferred,
		delta_position_reserve: Decrease(delta_position_amount),
		delta_position_shares: Decrease(shares_removed),
	})
}

pub fn calculate_tvl(hub_reserve: Balance, stable_asset: (Balance, Balance)) -> Option<Balance> {
	let (hub_reserve_hp, stable_reserve_hp, stable_hub_reserve_hp) =
		to_u256!(hub_reserve, stable_asset.0, stable_asset.1);

	let tvl = hub_reserve_hp
		.checked_mul(stable_reserve_hp)
		.and_then(|v| v.checked_div(stable_hub_reserve_hp))?;

	to_balance!(tvl).ok()
}

/// Calculate spot price between two omnipool assets, with incorporating the fee
///
/// Returns price of asset_in denominated in asset_out (asset_out/asset_in)
///
/// - `asset_a` - selling asset reserve
/// - `asset_b` - buying asset reserve
/// - `fee` - protocol fee and asset fee in a tuple
///
/// NOTE: If you want price with LRNA, use `calculate_lrna_spot_sprice`
///
pub fn calculate_spot_price(
	asset_a: &AssetReserveState<Balance>,
	asset_b: &AssetReserveState<Balance>,
	fee: Option<(Permill, Permill)>,
) -> Option<FixedU128> {
	let price_a = FixedU128::checked_from_rational(asset_a.hub_reserve, asset_a.reserve)?;
	let price_b = FixedU128::checked_from_rational(asset_b.reserve, asset_b.hub_reserve)?;
	let spot_price_without_fee = price_a.checked_mul(&price_b)?;

	if let Some((protocol_fee, asset_fee)) = fee {
		let protocol_fee_multipiler = Permill::from_percent(100).checked_sub(&protocol_fee)?;
		let protocol_fee_multiplier =
			FixedU128::checked_from_rational(protocol_fee_multipiler.deconstruct() as u128, 1_000_000)?;

		let asset_fee_multiplier = Permill::from_percent(100).checked_sub(&asset_fee)?;
		let asset_fee_multiplier =
			FixedU128::checked_from_rational(asset_fee_multiplier.deconstruct() as u128, 1_000_000)?;

		// Both protocol fee and asset fee reduce the asset_out amount received, both making the B/A price smaller
		// So we decrease the spot price with multiplying by (1-protocol_fee)*(1-asset_fee) to reflect correct amount out after the fee deduction
		let spot_price_with_fee = spot_price_without_fee
			.checked_mul(&protocol_fee_multiplier)?
			.checked_mul(&asset_fee_multiplier)?;

		return Some(spot_price_with_fee);
	}

	Some(spot_price_without_fee)
}

/// Calculate LRNA spot price
///
/// Returns price of LRNA denominated in asset (asset/LRNA)
///
/// - `asset` - selling asset reserve
/// - `fee` - asset fee
///
pub fn calculate_lrna_spot_price(asset: &AssetReserveState<Balance>, fee: Option<Permill>) -> Option<FixedU128> {
	let spot_price_without_fee = FixedU128::checked_from_rational(asset.reserve, asset.hub_reserve)?;

	if let Some(asset_fee) = fee {
		let asset_fee_multiplier = Permill::from_percent(100).checked_sub(&asset_fee)?;
		let asset_fee_multiplier =
			FixedU128::checked_from_rational(asset_fee_multiplier.deconstruct() as u128, 1_000_000)?;

		// No protocol fee involved when LRNA is sold
		// Fee is taken from TKN asset out, making the TKN/LRNA price smaller
		// We multiple by (1-asset_fee) to reflect correct amount out after the fee deduction
		let spot_price_with_fee = spot_price_without_fee.checked_mul(&asset_fee_multiplier)?;

		return Some(spot_price_with_fee);
	}

	Some(spot_price_without_fee)
}

pub fn calculate_cap_difference(
	asset: &AssetReserveState<Balance>,
	asset_cap: u128,
	total_hub_reserve: Balance,
) -> Option<Balance> {
	let weight_cap = FixedU128::from_inner(asset_cap);
	let max_allowed = weight_cap.checked_mul_int(total_hub_reserve)?;
	let p = FixedU128::checked_from_rational(asset.hub_reserve, max_allowed)?;
	if p > FixedU128::one() {
		Some(0)
	} else {
		FixedU128::one().checked_sub(&p)?.checked_mul_int(asset.reserve)
	}
}

pub fn calculate_tvl_cap_difference(
	asset: &AssetReserveState<Balance>,
	stable_asset: &AssetReserveState<Balance>,
	tvl_cap: Balance,
	total_hub_reserve: Balance,
) -> Option<Balance> {
	let (tvl, stable_hub_reserve, stable_reserve, total_hub_reserve, asset_reserve, asset_hub_reserve) = to_u256!(
		tvl_cap,
		stable_asset.hub_reserve,
		stable_asset.reserve,
		total_hub_reserve,
		asset.reserve,
		asset.hub_reserve
	);
	let max_hub_reserve = tvl.checked_mul(stable_hub_reserve)?.checked_div(stable_reserve)?;

	if max_hub_reserve < total_hub_reserve {
		return Some(0);
	}

	let delta_q = max_hub_reserve.checked_sub(total_hub_reserve)?;

	let amount = delta_q.checked_mul(asset_reserve)?.checked_div(asset_hub_reserve)?;

	to_balance!(amount).ok()
}

/// Verify if cap does or does exceed asset's weight cap.
pub fn verify_asset_cap(
	asset: &AssetReserveState<Balance>,
	asset_cap: u128,
	hub_amount: Balance,
	total_hub_reserve: Balance,
) -> Option<bool> {
	let weight_cap = FixedU128::from_inner(asset_cap);
	let weight = FixedU128::checked_from_rational(
		asset.hub_reserve.checked_add(hub_amount)?,
		total_hub_reserve.checked_add(hub_amount)?,
	)?;
	Some(weight <= weight_cap)
}
