//! This is temporaty implementation of simulators' `DataProvider` using runtime.
//! This should be removed when we'll move solver from runtime to node.

use core::marker::PhantomData;
use frame_support::traits::Get;
use hydradx_traits::fee::GetDynamicFee;
use ice_support::AssetId;
use ice_support::Balance;
use orml_traits::MultiCurrency;
use sp_runtime::Permill;
use sp_std::vec;
use sp_std::vec::Vec;

use amm_simulator::omnipool::DataProvider as OmnipoolDataProvider;
use pallet_omnipool::types::AssetState;

/// Whether governance has taken this target out of the solver's routing.
fn is_excluded(target: ice_support::RoutingTarget) -> bool {
	pallet_ice::SolverRouting::<crate::Runtime>::get(target) == Some(ice_support::RoutingState::Excluded)
}

pub struct Omnipool<T>(PhantomData<T>);

impl<T: pallet_omnipool::Config<AssetId = AssetId>> OmnipoolDataProvider for Omnipool<T> {
	type AccountId = T::AccountId;

	fn protocol_account() -> Self::AccountId {
		pallet_omnipool::Pallet::<T>::protocol_account()
	}

	/// Assets excluded by governance are dropped here rather than filtered later,
	/// so they never enter the snapshot at all — no pool edge, no spot price, and
	/// route discovery simply routes around them.
	fn assets() -> impl Iterator<Item = (AssetId, AssetState<Balance>)> {
		pallet_omnipool::pallet::Assets::<T>::iter()
			.filter(|(asset_id, _)| !is_excluded(ice_support::RoutingTarget::OmnipoolAsset(*asset_id)))
	}

	fn free_balance(currncy_id: AssetId, who: &Self::AccountId) -> Balance {
		T::Currency::free_balance(currncy_id, who)
	}

	fn fee(key: (AssetId, Balance)) -> (Permill, Permill) {
		T::Fee::get(key)
	}

	fn hub_asset_id() -> AssetId {
		T::HubAssetId::get()
	}

	fn min_trading_limit() -> Balance {
		T::MinimumTradingLimit::get()
	}

	fn max_in_ratio() -> Balance {
		T::MaxInRatio::get()
	}

	fn max_out_ratio() -> Balance {
		T::MaxOutRatio::get()
	}

	fn slip_fee() -> Option<pallet_omnipool::types::SlipFeeConfig> {
		pallet_omnipool::pallet::SlipFee::<T>::get()
	}
}

use amm_simulator::stableswap::DataProvider as StableswapDataProvider;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_stableswap::types::PoolInfo;
use pallet_stableswap::types::PoolPegInfo;
use pallet_stableswap::types::PoolSnapshot;

pub struct Stableswap<T>(PhantomData<T>);

impl<T: pallet_stableswap::Config<AssetId = AssetId>> StableswapDataProvider for Stableswap<T> {
	type BlockNumber = BlockNumberFor<T>;

	/// Excluded pools are dropped here, so they never reach the snapshot and route
	/// discovery simply routes around them.
	fn pools() -> impl Iterator<Item = (AssetId, PoolInfo<AssetId, Self::BlockNumber>)> {
		pallet_stableswap::pallet::Pools::<T>::iter()
			.filter(|(pool_id, _)| !is_excluded(ice_support::RoutingTarget::StableswapPool(*pool_id)))
	}

	fn pool_pegs(pool_id: AssetId) -> Option<PoolPegInfo<Self::BlockNumber, AssetId>> {
		pallet_stableswap::pallet::PoolPegs::<T>::get(pool_id)
	}

	fn create_snapshot(pool_id: AssetId) -> Option<PoolSnapshot<AssetId>> {
		pallet_stableswap::Pallet::<T>::create_snapshot(pool_id)
	}

	fn min_trading_limit() -> Balance {
		T::MinTradingLimit::get()
	}
}

use crate::evm::aave_trade_executor::AaveTradeExecutor;
use crate::evm::executor::BalanceOf;
use crate::evm::executor::NonceIdOf;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::Runtime;
use amm_simulator::aave::DataProvider as AaveDataProvider;
use evm::ExitReason;
use hydradx_traits::evm::CallResult;
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::evm::EVM;
use pallet_evm::AddressMapping;
use pallet_liquidation::BorrowingContract;
use primitives::EvmAddress;
use sp_core::U256;

pub struct Aave<T>(PhantomData<T>);

impl<T> AaveDataProvider for Aave<T>
where
	T: frame_system::Config + pallet_liquidation::Config + pallet_evm::Config + pallet_dispatcher::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
	NonceIdOf<T>: Into<T::Nonce>,
{
	fn view(context: hydradx_traits::evm::CallContext, data: Vec<u8>, gas: u64) -> (ExitReason, Vec<u8>) {
		let CallResult {
			exit_reason,
			value,
			contract: _,
			gas_used: _,
			gas_limit: _t,
		} = crate::evm::Executor::<T>::view(context, data, gas);

		(exit_reason, value)
	}

	fn borrowing_contract() -> EvmAddress {
		pallet_liquidation::BorrowingContract::<T>::get()
	}

	fn address_to_asset(address: EvmAddress) -> Option<AssetId> {
		crate::evm::precompiles::erc20_mapping::HydraErc20Mapping::address_to_asset(address)
	}

	fn pairs() -> Vec<(AssetId, AssetId)> {
		let pool = <BorrowingContract<Runtime>>::get();
		let reserves = match AaveTradeExecutor::<Runtime>::get_reserves_list(pool) {
			Ok(reserves) => reserves,
			Err(_) => return vec![],
		};
		reserves
			.into_iter()
			.filter_map(|reserve| {
				let data = AaveTradeExecutor::<Runtime>::get_reserve_data(pool, reserve).ok()?;
				let reserve_asset = HydraErc20Mapping::address_to_asset(reserve)?;
				let atoken_asset = HydraErc20Mapping::address_to_asset(data.atoken_address)?;
				Some((reserve_asset, atoken_asset))
			})
			.filter(|(reserve, atoken)| !is_excluded(ice_support::RoutingTarget::AaveWrap(*reserve, *atoken)))
			.collect()
	}
}

use amm_simulator::uniswap_v3::DataProvider as UniswapV3DataProvider;
use ice_support::RoutingState;
use ice_support::RoutingTarget;

pub struct UniswapV3<T>(PhantomData<T>);

impl<T> UniswapV3DataProvider for UniswapV3<T>
where
	T: frame_system::Config + pallet_ice::Config + pallet_evm::Config + pallet_dispatcher::Config,
	BalanceOf<T>: TryFrom<U256> + Into<U256>,
	T::AddressMapping: AddressMapping<T::AccountId>,
	pallet_evm::AccountIdOf<T>: From<T::AccountId>,
	NonceIdOf<T>: Into<T::Nonce>,
{
	fn view(context: hydradx_traits::evm::CallContext, data: Vec<u8>, gas: u64) -> (ExitReason, Vec<u8>) {
		let CallResult {
			exit_reason,
			value,
			contract: _,
			gas_used: _,
			gas_limit: _,
		} = crate::evm::Executor::<T>::view(context, data, gas);

		(exit_reason, value)
	}

	fn quoter() -> Option<EvmAddress> {
		pallet_parameters::Pallet::<crate::Runtime>::uniswap_v3_quoter()
	}

	fn pools() -> Vec<EvmAddress> {
		pallet_ice::SolverRouting::<T>::iter()
			.filter_map(|(target, state)| match (target, state) {
				(RoutingTarget::UniswapV3Pool(address), RoutingState::Included) => Some(address),
				_ => None,
			})
			.collect()
	}

	fn address_to_asset(address: EvmAddress) -> Option<AssetId> {
		HydraErc20Mapping::address_to_asset(address)
	}
}
