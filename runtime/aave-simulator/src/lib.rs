#![cfg_attr(not(feature = "std"), no_std)]

use codec::Decode;
use codec::Encode;
use core::marker::PhantomData;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AmmSimulator, SimulatorError, TradeResult};
use hydradx_traits::evm::CallResult;
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::evm::EVM;
use hydradx_traits::router::PoolType;
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::Price;

//NOTE: This is tmp. dummy impl. of aave simulator that always trade 1:1 and doesn't do any checks.
pub struct AaveSimulator<Evm, ErcMapping>(PhantomData<(Evm, ErcMapping)>);

#[derive(Clone, Debug, Default, Encode, Decode)]
pub struct Snapshot {}

impl<Evm, ErcMapping> AmmSimulator for AaveSimulator<Evm, ErcMapping>
where
	Evm: EVM<CallResult>,
	ErcMapping: Erc20Mapping<AssetId>,
{
	type Snapshot = Snapshot;

	fn snapshot() -> Self::Snapshot {
		Snapshot {}
	}

	fn pool_type() -> PoolType<AssetId> {
		PoolType::Aave
	}

	fn simulate_buy(
		_asset_in: AssetId,
		_asset_out: AssetId,
		amount_out: Balance,
		_max_amount_in: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		Ok((
			snapshot.clone(),
			TradeResult {
				amount_in: amount_out,
				amount_out,
			},
		))
	}

	fn simulate_sell(
		_asset_in: AssetId,
		_asset_out: AssetId,
		amount_in: Balance,
		_min_amount_out: Balance,
		snapshot: &Self::Snapshot,
	) -> Result<(Self::Snapshot, TradeResult), SimulatorError> {
		Ok((
			snapshot.clone(),
			TradeResult {
				amount_in,
				amount_out: amount_in,
			},
		))
	}

	fn get_spot_price(
		_asset_in: primitives::AssetId,
		_asset_out: primitives::AssetId,
		_snapshot: &Self::Snapshot,
	) -> Result<Price, SimulatorError> {
		Ok(Ratio { n: 1, d: 1 })
	}
}
