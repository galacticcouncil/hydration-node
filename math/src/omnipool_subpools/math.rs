#![allow(clippy::too_many_arguments)]

use crate::omnipool::types::{AssetReserveState, AssetStateChange, BalanceUpdate, Position};
use crate::omnipool_subpools::types::MigrationDetails;
use crate::support::rational::{round_to_rational, Rounding};

use crate::support::traits::{CheckedDivInner, CheckedMulInner, CheckedMulInto};
use crate::types::Balance;

pub fn convert_position(position: Position<Balance>, details: MigrationDetails) -> Option<Position<Balance>> {
	let shares = position
		.shares
		.checked_mul_into(&details.hub_reserve)?
		.checked_div_inner(&details.shares)?
		.try_into()
		.ok()?;

	let amount = position
		.amount
		.checked_mul_into(&details.share_tokens)?
		.checked_div_inner(&details.shares)?
		.try_into()
		.ok()?;

	let nominator = position.price.0.checked_mul_into(&details.price.1)?;
	let denom = position.price.1.checked_mul_into(&details.price.0)?;

	Some(Position {
		shares,
		amount,
		price: round_to_rational((nominator, denom), Rounding::Nearest),
	})
}

pub fn create_subpool_initial_state(
	asset_state_a: &AssetReserveState<Balance>,
	asset_state_b: &AssetReserveState<Balance>,
) -> Option<AssetReserveState<Balance>> {
	let hub_reserve = asset_state_a.hub_reserve.checked_add(asset_state_b.hub_reserve)?;

	let protocol_shares = recalculate_protocol_shares(
		asset_state_a.hub_reserve,
		asset_state_a.shares,
		asset_state_a.protocol_shares,
	)?
	.checked_add(recalculate_protocol_shares(
		asset_state_b.hub_reserve,
		asset_state_b.shares,
		asset_state_b.protocol_shares,
	)?)?;

	let shares = hub_reserve;
	let reserve = shares;

	Some(AssetReserveState {
		reserve,
		hub_reserve,
		shares,
		protocol_shares,
	})
}

pub fn calculate_asset_migration_details(
	asset_state: &AssetReserveState<Balance>,
	subpool_state: Option<&AssetReserveState<Balance>>,
	share_issuance: Balance,
) -> Option<(MigrationDetails, Option<AssetStateChange<Balance>>)> {
	if let Some(subpool_state) = subpool_state {
		let p1 = subpool_state
			.shares
			.checked_mul_into(&asset_state.hub_reserve)?
			.checked_div_inner(&subpool_state.hub_reserve)?;
		let p2 = p1
			.checked_mul_inner(&asset_state.protocol_shares)?
			.checked_div_inner(&asset_state.shares)?;
		let delta_ps = p2.try_into().ok()?;

		let delta_s = asset_state
			.hub_reserve
			.checked_mul_into(&subpool_state.shares)?
			.checked_div_inner(&subpool_state.hub_reserve)?
			.try_into()
			.ok()?;

		let delta_u = asset_state
			.hub_reserve
			.checked_mul_into(&share_issuance)?
			.checked_div_inner(&subpool_state.hub_reserve)?
			.try_into()
			.ok()?;

		// price = asset price * share_issuance / pool shares
		// price = (hub reserve / reserve ) * share issuance / pool shares
		// price = hub*issuance / reserve * pool shares
		let price_denom = asset_state.reserve.checked_mul_into(&subpool_state.shares)?;

		let price_num = asset_state.hub_reserve.checked_mul_into(&share_issuance)?;

		let delta_q = asset_state.hub_reserve;

		Some((
			MigrationDetails {
				price: round_to_rational((price_num, price_denom), Rounding::Nearest),
				shares: asset_state.shares,
				hub_reserve: delta_q,
				share_tokens: delta_u,
			},
			Some(AssetStateChange {
				delta_reserve: BalanceUpdate::Increase(delta_u),
				delta_hub_reserve: BalanceUpdate::Increase(delta_q),
				delta_shares: BalanceUpdate::Increase(delta_s),
				delta_protocol_shares: BalanceUpdate::Increase(delta_ps),
			}),
		))
	} else {
		// This case if when subpool is being created
		Some((
			MigrationDetails {
				price: asset_state.price_as_rational(),
				shares: asset_state.shares,
				hub_reserve: asset_state.hub_reserve,
				share_tokens: asset_state.hub_reserve,
			},
			None,
		))
	}
}

pub fn recalculate_protocol_shares(hub_reserve: Balance, shares: Balance, protocol_shares: Balance) -> Option<Balance> {
	hub_reserve
		.checked_mul_into(&protocol_shares)?
		.checked_div_inner(&shares)?
		.try_into()
		.ok()
}
