//! This is temporaty implementation of simulators' `DataProvider` using runtime.
//! This should be removed when we'll move solver from runtime to node.

use core::marker::PhantomData;
use frame_support::traits::Get;
use hydradx_traits::fee::GetDynamicFee;
use ice_support::AssetId;
use ice_support::Balance;
use orml_traits::MultiCurrency;
use sp_runtime::Permill;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec;
use sp_std::vec::Vec;

use amm_simulator::omnipool::DataProvider as OmnipoolDataProvider;
use pallet_omnipool::types::AssetState;

/// Whether governance has taken this target out of the solver's routing.
fn is_excluded(target: ice_support::RoutingTarget) -> bool {
	pallet_ice::SolverRouting::<crate::Runtime>::get(&target) == Some(ice_support::RoutingState::Excluded)
}

/// Everything governance has opted in for one venue: the union of every `Included`
/// entry naming it, minus every `Excluded` one. Exclusion wins, so a single entry
/// vetoes one member out of a batch without the batch being rewritten.
///
/// `extract` pulls a venue's members out of a target, a batch and a single entry
/// alike, and returns `None` for a target belonging to another venue.
fn registered<T: Ord>(extract: impl Fn(&ice_support::RoutingTarget) -> Option<Vec<T>>) -> Vec<T> {
	let mut included = BTreeSet::new();
	let mut excluded = BTreeSet::new();

	for (target, state) in pallet_ice::SolverRouting::<crate::Runtime>::iter() {
		let Some(members) = extract(&target) else {
			continue;
		};
		match state {
			ice_support::RoutingState::Included => included.extend(members),
			ice_support::RoutingState::Excluded => excluded.extend(members),
		}
	}

	included.into_iter().filter(|m| !excluded.contains(m)).collect()
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

use crate::evm::executor::BalanceOf;
use crate::evm::executor::NonceIdOf;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
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

	/// Aave is opt-in: enumerating every reserve cost two EVM view calls per reserve
	/// before a snapshot could even start, and grew with each reserve listed.
	fn pairs() -> Vec<(AssetId, AssetId)> {
		registered(|target| match target {
			RoutingTarget::AaveWrap(reserve, atoken) => Some(vec![(*reserve, *atoken)]),
			RoutingTarget::AaveWraps(wraps) => Some(wraps.to_vec()),
			_ => None,
		})
	}

	fn asset_address(asset: AssetId) -> EvmAddress {
		HydraErc20Mapping::asset_address(asset)
	}
}

use amm_simulator::uniswap_v3::DataProvider as UniswapV3DataProvider;
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
		registered(|target| match target {
			RoutingTarget::UniswapV3Pool(address) => Some(vec![*address]),
			RoutingTarget::UniswapV3Pools(addresses) => Some(addresses.to_vec()),
			_ => None,
		})
	}

	fn address_to_asset(address: EvmAddress) -> Option<AssetId> {
		HydraErc20Mapping::address_to_asset(address)
	}
}

use amm_simulator::xyk::DataProvider as XykDataProvider;

pub struct Xyk<T>(PhantomData<T>);

impl<T: pallet_xyk::Config> XykDataProvider for Xyk<T> {
	/// XYK is opt-in: only pairs governance has registered are read, and the rest of
	/// the hundreds of permissionless pools on chain are never touched. Enumerating
	/// them all cost ~13 us each in state load for pools that will never trade.
	fn pools() -> Vec<(AssetId, AssetId, Balance, Balance)> {
		registered(|target| match target {
			RoutingTarget::XykPool(asset_a, asset_b) => Some(vec![(*asset_a, *asset_b)]),
			RoutingTarget::XykPools(pools) => Some(pools.to_vec()),
			_ => None,
		})
		.into_iter()
		.filter_map(|(asset_a, asset_b)| {
			let pair_account = pallet_xyk::Pallet::<T>::pair_account_from_assets(asset_a, asset_b);
			// A registered pair that does not exist on chain is skipped rather than
			// fabricated with zero reserves. Reading the stored pair back also gives
			// the order the pool was created with, not the normalised registry order.
			let (asset_a, asset_b) = pallet_xyk::Pallet::<T>::pool_assets(&pair_account)?;
			Some((
				asset_a,
				asset_b,
				<T as pallet_xyk::Config>::Currency::free_balance(asset_a, &pair_account),
				<T as pallet_xyk::Config>::Currency::free_balance(asset_b, &pair_account),
			))
		})
		.collect()
	}

	fn exchange_fee() -> (u32, u32) {
		T::GetExchangeFee::get()
	}

	fn min_trading_limit() -> Balance {
		T::MinTradingLimit::get()
	}

	fn max_in_ratio() -> u128 {
		T::MaxInRatio::get()
	}

	fn max_out_ratio() -> u128 {
		T::MaxOutRatio::get()
	}
}
