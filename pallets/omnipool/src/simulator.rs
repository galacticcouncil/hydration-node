use crate::types::{AssetReserveState, Balance, Tradability};
use crate::{Assets, Config, Pallet};
use codec::{Decode, Encode};
use frame_support::traits::Get;
use hydra_dx_math::support::rational::{round_to_rational, Rounding};
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::fee::GetDynamicFee;
use hydradx_traits::router::PoolType;
use orml_traits::MultiCurrency;
use primitive_types::U256;
use sp_runtime::{traits::Zero, Permill};
use sp_std::collections::btree_map::BTreeMap;

/// Snapshot of Omnipool state for simulation purposes.
///
/// Contains all asset states needed to simulate trades without
/// accessing chain storage.
#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct OmnipoolSnapshot {
	/// Asset states: AssetId -> AssetReserveState
	pub assets: BTreeMap<u32, AssetReserveState<Balance>>,
	/// Asset fees: AssetId -> (asset_fee, protocol_fee)
	/// Stored separately to avoid changing AssetReserveState type
	pub fees: BTreeMap<u32, (Permill, Permill)>,
	/// Hub asset id
	pub hub_asset_id: u32,
	/// Minimum trading limit
	pub min_trading_limit: Balance,
	/// Max in ratio
	pub max_in_ratio: Balance,
	/// Max out ratio
	pub max_out_ratio: Balance,
}

impl OmnipoolSnapshot {
	pub fn get_asset(&self, asset_id: u32) -> Option<&AssetReserveState<Balance>> {
		self.assets.get(&asset_id)
	}

	pub fn get_fees(&self, asset_id: u32) -> (Permill, Permill) {
		self.fees
			.get(&asset_id)
			.copied()
			.unwrap_or((Permill::zero(), Permill::zero()))
	}

	pub fn with_updated_asset(mut self, asset_id: u32, state: AssetReserveState<Balance>) -> Self {
		self.assets.insert(asset_id, state);
		self
	}
}

impl<T: Config<AssetId = u32>> AmmSimulator for Pallet<T> {
	type Snapshot = OmnipoolSnapshot;

	fn pool_type() -> PoolType<u32> {
		PoolType::Omnipool
	}

	fn snapshot() -> Self::Snapshot {
		let protocol_account = Self::protocol_account();

		let mut assets: BTreeMap<u32, AssetReserveState<Balance>> = BTreeMap::new();
		let mut fees: BTreeMap<u32, (Permill, Permill)> = BTreeMap::new();

		for (asset_id, state) in Assets::<T>::iter() {
			let reserve = T::Currency::free_balance(asset_id, &protocol_account);
			let (asset_fee, protocol_fee) = T::Fee::get((asset_id, reserve));

			let reserve_state = (state, reserve).into();
			assets.insert(asset_id, reserve_state);
			fees.insert(asset_id, (asset_fee, protocol_fee));
		}

		OmnipoolSnapshot {
			assets,
			fees,
			hub_asset_id: T::HubAssetId::get(),
			min_trading_limit: T::MinimumTradingLimit::get(),
			max_in_ratio: T::MaxInRatio::get(),
			max_out_ratio: T::MaxOutRatio::get(),
		}
	}

	fn simulate_sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: Balance,
		min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if asset_in == asset_out {
			return Err(SimulatorError::Other);
		}

		if amount_in < snapshot.min_trading_limit {
			return Err(SimulatorError::TradeTooSmall);
		}

		// Hub asset not allowed
		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			return Err(SimulatorError::Other);
		}

		let asset_in_state = snapshot.get_asset(asset_in).ok_or(SimulatorError::AssetNotFound)?;
		let asset_out_state = snapshot.get_asset(asset_out).ok_or(SimulatorError::AssetNotFound)?;

		// Check tradability
		if !asset_in_state.tradable.contains(Tradability::SELL) {
			return Err(SimulatorError::Other);
		}
		if !asset_out_state.tradable.contains(Tradability::BUY) {
			return Err(SimulatorError::Other);
		}

		if amount_in
			> asset_in_state
				.reserve
				.checked_div(snapshot.max_in_ratio)
				.ok_or(SimulatorError::MathError)?
		{
			return Err(SimulatorError::TradeTooLarge);
		}

		let (asset_fee, _) = snapshot.get_fees(asset_out);
		let (_, protocol_fee) = snapshot.get_fees(asset_in);
		let withdraw_fee = Permill::from_percent(0); // Not used in trades

		let state_changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
			&asset_in_state.into(),
			&asset_out_state.into(),
			amount_in,
			asset_fee,
			protocol_fee,
			withdraw_fee,
		)
		.ok_or(SimulatorError::MathError)?;

		let amount_out = *state_changes.asset_out.delta_reserve;

		if amount_out == Balance::zero() {
			return Err(SimulatorError::InsufficientLiquidity);
		}

		if amount_out < min_amount_out {
			return Err(SimulatorError::LimitNotMet);
		}

		if amount_out
			> asset_out_state
				.reserve
				.checked_div(snapshot.max_out_ratio)
				.ok_or(SimulatorError::MathError)?
		{
			return Err(SimulatorError::TradeTooLarge);
		}

		let new_asset_in_state = apply_state_changes(asset_in_state, &state_changes.asset_in)?;
		let new_asset_out_state = apply_state_changes(asset_out_state, &state_changes.asset_out)?;

		let new_snapshot = snapshot
			.clone()
			.with_updated_asset(asset_in, new_asset_in_state)
			.with_updated_asset(asset_out, new_asset_out_state);

		Ok((new_snapshot, TradeResult::new(amount_in, amount_out)))
	}

	fn simulate_buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: Balance,
		max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		if asset_in == asset_out {
			return Err(SimulatorError::Other);
		}

		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			return Err(SimulatorError::Other);
		}

		let asset_in_state = snapshot.get_asset(asset_in).ok_or(SimulatorError::AssetNotFound)?;
		let asset_out_state = snapshot.get_asset(asset_out).ok_or(SimulatorError::AssetNotFound)?;

		if !asset_in_state.tradable.contains(Tradability::SELL) {
			return Err(SimulatorError::Other);
		}
		if !asset_out_state.tradable.contains(Tradability::BUY) {
			return Err(SimulatorError::Other);
		}

		let (asset_fee, _) = snapshot.get_fees(asset_out);
		let (_, protocol_fee) = snapshot.get_fees(asset_in);
		let withdraw_fee = Permill::from_percent(0); // Not used in trades

		let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
			&asset_in_state.into(),
			&asset_out_state.into(),
			amount_out,
			asset_fee,
			protocol_fee,
			withdraw_fee,
		)
		.ok_or(SimulatorError::MathError)?;

		let amount_in = *state_changes.asset_in.delta_reserve;

		if amount_in > max_amount_in {
			return Err(SimulatorError::LimitNotMet);
		}

		if amount_in < snapshot.min_trading_limit {
			return Err(SimulatorError::TradeTooSmall);
		}

		if amount_in
			> asset_in_state
				.reserve
				.checked_div(snapshot.max_in_ratio)
				.ok_or(SimulatorError::MathError)?
		{
			return Err(SimulatorError::TradeTooLarge);
		}

		if amount_out
			> asset_out_state
				.reserve
				.checked_div(snapshot.max_out_ratio)
				.ok_or(SimulatorError::MathError)?
		{
			return Err(SimulatorError::TradeTooLarge);
		}

		let new_asset_in_state = apply_state_changes(asset_in_state, &state_changes.asset_in)?;
		let new_asset_out_state = apply_state_changes(asset_out_state, &state_changes.asset_out)?;

		let new_snapshot = snapshot
			.clone()
			.with_updated_asset(asset_in, new_asset_in_state)
			.with_updated_asset(asset_out, new_asset_out_state);

		Ok((new_snapshot, TradeResult::new(amount_in, amount_out)))
	}

	fn get_spot_price(asset_in: u32, asset_out: u32, snapshot: &Self::Snapshot) -> Result<Ratio, SimulatorError> {
		if asset_in == snapshot.hub_asset_id {
			// Price of hub asset in terms of asset_out
			// hub_price = reserve_out / hub_reserve_out
			let state_out = snapshot.get_asset(asset_out).ok_or(SimulatorError::AssetNotFound)?;
			Ok(Ratio::new(state_out.reserve, state_out.hub_reserve))
		} else if asset_out == snapshot.hub_asset_id {
			// Price of asset_in in terms of hub asset
			// price = hub_reserve_in / reserve_in
			let state_in = snapshot.get_asset(asset_in).ok_or(SimulatorError::AssetNotFound)?;
			Ok(Ratio::new(state_in.hub_reserve, state_in.reserve))
		} else {
			// Cross-rate: price of asset_in in terms of asset_out
			// price = (hub_reserve_in / reserve_in) / (hub_reserve_out / reserve_out)
			//       = (hub_reserve_in * reserve_out) / (reserve_in * hub_reserve_out)
			let state_in = snapshot.get_asset(asset_in).ok_or(SimulatorError::AssetNotFound)?;
			let state_out = snapshot.get_asset(asset_out).ok_or(SimulatorError::AssetNotFound)?;

			// Use U256 to avoid overflow in multiplication
			let n = U256::from(state_in.hub_reserve) * U256::from(state_out.reserve);
			let d = U256::from(state_in.reserve) * U256::from(state_out.hub_reserve);

			let (n, d) = round_to_rational((n, d), Rounding::Nearest);
			Ok(Ratio::new(n, d))
		}
	}

	fn can_trade(asset_in: u32, asset_out: u32, snapshot: &Self::Snapshot) -> Option<PoolType<u32>> {
		// Hub asset trades are not supported directly
		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			return None;
		}

		// Both assets must be in the omnipool
		let has_in = snapshot.assets.contains_key(&asset_in);
		let has_out = snapshot.assets.contains_key(&asset_out);

		if has_in && has_out {
			Some(PoolType::Omnipool)
		} else {
			None
		}
	}
}

fn apply_state_changes(
	current: &AssetReserveState<Balance>,
	changes: &hydra_dx_math::omnipool::types::AssetStateChange<Balance>,
) -> Result<AssetReserveState<Balance>, SimulatorError> {
	use hydra_dx_math::omnipool::types::BalanceUpdate;

	let new_reserve = match &changes.delta_reserve {
		BalanceUpdate::Increase(delta) => current.reserve.checked_add(*delta),
		BalanceUpdate::Decrease(delta) => current.reserve.checked_sub(*delta),
	}
	.ok_or(SimulatorError::MathError)?;

	let new_hub_reserve = match &changes.delta_hub_reserve {
		BalanceUpdate::Increase(delta) => current.hub_reserve.checked_add(*delta),
		BalanceUpdate::Decrease(delta) => current.hub_reserve.checked_sub(*delta),
	}
	.ok_or(SimulatorError::MathError)?;

	let new_shares = match &changes.delta_shares {
		BalanceUpdate::Increase(delta) => current.shares.checked_add(*delta),
		BalanceUpdate::Decrease(delta) => current.shares.checked_sub(*delta),
	}
	.ok_or(SimulatorError::MathError)?;

	let new_protocol_shares = match &changes.delta_protocol_shares {
		BalanceUpdate::Increase(delta) => current.protocol_shares.checked_add(*delta),
		BalanceUpdate::Decrease(delta) => current.protocol_shares.checked_sub(*delta),
	}
	.ok_or(SimulatorError::MathError)?;

	Ok(AssetReserveState {
		reserve: new_reserve,
		hub_reserve: new_hub_reserve,
		shares: new_shares,
		protocol_shares: new_protocol_shares,
		cap: current.cap,
		tradable: current.tradable,
	})
}
