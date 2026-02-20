//! This is temporaty implementation of simulators' `DataProvider` using runtime.
//! This should be removed when we'll move solver from runtime to node.

use core::marker::PhantomData;
use frame_support::traits::Get;
use hydradx_traits::fee::GetDynamicFee;
use ice_support::AssetId;
use ice_support::Balance;
use orml_traits::MultiCurrency;
use sp_runtime::Permill;
use sp_std::vec::Vec;

use amm_simulator::omnipool::DataProvider as OmnipoolDataProvider;
use pallet_omnipool::types::AssetState;

pub struct Omnipool<T>(PhantomData<T>);

impl<T: pallet_omnipool::Config<AssetId = AssetId>> OmnipoolDataProvider for Omnipool<T> {
	type AccountId = T::AccountId;

	fn protocol_account() -> Self::AccountId {
		pallet_omnipool::Pallet::<T>::protocol_account()
	}

	fn assets() -> impl Iterator<Item = (AssetId, AssetState<Balance>)> {
		pallet_omnipool::pallet::Assets::<T>::iter()
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
}

use amm_simulator::stableswap::DataProvider as StableswapDataProvider;
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_stableswap::types::PoolInfo;
use pallet_stableswap::types::PoolPegInfo;
use pallet_stableswap::types::PoolSnapshot;

pub struct Stableswap<T>(PhantomData<T>);

impl<T: pallet_stableswap::Config<AssetId = AssetId>> StableswapDataProvider for Stableswap<T> {
	type BlockNumber = BlockNumberFor<T>;

	fn pools() -> impl Iterator<Item = (AssetId, PoolInfo<AssetId, Self::BlockNumber>)> {
		pallet_stableswap::pallet::Pools::<T>::iter()
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

use crate::evm::executor::BalanceOf;
use crate::evm::executor::NonceIdOf;
use amm_simulator::aave::DataProvider as AaveDataProvider;
use evm::ExitReason;
use hydradx_traits::evm::CallResult;
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::evm::EVM;
use pallet_evm::AddressMapping;
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
	T::AddressMapping: AddressMapping<T::AccountId>,
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
}
