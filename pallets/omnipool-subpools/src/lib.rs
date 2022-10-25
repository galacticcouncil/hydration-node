#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

use frame_support::pallet_prelude::*;
use sp_runtime::FixedU128;
use sp_std::prelude::*;

pub use pallet::*;

pub type Balance = u128;

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetDetail {
	pub(crate) price: FixedU128,
	pub(crate) shares: Balance,
	pub(crate) hub_reserve: Balance,
	pub(crate) share_tokens: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pallet_omnipool::types::Tradability;
	use pallet_stableswap::types::AssetLiquidity;
	use sp_runtime::{ArithmeticError, Permill};

	#[pallet::pallet]
	#[pallet::generate_store(pub (crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_omnipool::Config + pallet_stableswap::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The origin which can create a new pool
		type CreatePoolOrigin: EnsureOrigin<Self::Origin>;
	}

	/// Assets migrated from Omnipool to a subpool
	#[pallet::storage]
	#[pallet::getter(fn migrated_assets)]
	pub(super) type MigratedAssets<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		<T as pallet_omnipool::Config>::AssetId,
		(<T as pallet_stableswap::Config>::AssetId, AssetDetail),
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub (crate) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId:
			Into<<T as pallet_stableswap::Config>::AssetId> + From<<T as pallet_stableswap::Config>::AssetId>,
	{
		///
		///
		/// Limit to 2 assets.
		///
		#[pallet::weight(0)]
		pub fn create_subpool(
			origin: OriginFor<T>,
			asset_a: <T as pallet_omnipool::Config>::AssetId,
			asset_b: <T as pallet_omnipool::Config>::AssetId,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		) -> DispatchResult {
			<T as Config>::CreatePoolOrigin::ensure_origin(origin.clone())?;

			// Load state - return AssetNotFound if it does not exist
			let asset_state_a = pallet_omnipool::Pallet::<T>::load_asset_state(asset_a)?;
			let asset_state_b = pallet_omnipool::Pallet::<T>::load_asset_state(asset_a)?;

			// Create new subpool
			let pool_id = pallet_stableswap::Pallet::<T>::do_create_pool(
				&[asset_a.into(), asset_b.into()],
				amplification,
				trade_fee,
				withdraw_fee,
			)?;
			let omnipool_account = pallet_omnipool::Pallet::<T>::protocol_account();

			// Move liquidity from omnipool account to subpool
			pallet_stableswap::Pallet::<T>::move_liquidity_to_pool(
				&omnipool_account,
				pool_id,
				&[
					AssetLiquidity::<<T as pallet_stableswap::Config>::AssetId> {
						asset_id: asset_a.into(),
						amount: asset_state_a.reserve,
					},
					AssetLiquidity::<<T as pallet_stableswap::Config>::AssetId> {
						asset_id: asset_b.into(),
						amount: asset_state_b.reserve,
					},
				],
			)?;

			// Deposit pool shares to omnipool account
			let hub_reserve = asset_state_a
				.hub_reserve
				.checked_add(asset_state_b.hub_reserve)
				.ok_or(ArithmeticError::Overflow)?;
			let protocol_shares = asset_state_a
				.protocol_shares
				.checked_add(asset_state_b.protocol_shares)
				.ok_or(ArithmeticError::Overflow)?;
			let cap = asset_state_a
				.cap
				.checked_add(asset_state_b.cap)
				.ok_or(ArithmeticError::Overflow)?;
			let shares = hub_reserve;
			pallet_stableswap::Pallet::<T>::deposit_shares(&omnipool_account, pool_id, shares)?;

			// Remove tokens from omnipool
			pallet_omnipool::Pallet::<T>::remove_asset(asset_a)?;
			pallet_omnipool::Pallet::<T>::remove_asset(asset_b)?;

			// Add Share token to omnipool as another asset - LRNA is Qi + Qj
			pallet_omnipool::Pallet::<T>::add_asset(
				pool_id.into(),
				hub_reserve,
				shares,
				protocol_shares,
				cap,
				Tradability::default(),
			)?;

			// Remember some stuff to be able to update LP positions later on
			let asset_a_details = AssetDetail {
				price: Default::default(), //TODO: correct price
				shares: asset_state_a.shares,
				hub_reserve: asset_state_a.hub_reserve,
				share_tokens: asset_state_a.hub_reserve,
			};
			let asset_b_details = AssetDetail {
				price: Default::default(), //TODO: correct price
				shares: asset_state_b.shares,
				hub_reserve: asset_state_b.hub_reserve,
				share_tokens: asset_state_b.hub_reserve,
			};

			MigratedAssets::<T>::insert(asset_a, (pool_id, asset_a_details));
			MigratedAssets::<T>::insert(asset_b, (pool_id, asset_b_details));

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn migrate_asset_to_subpool(
			origin: OriginFor<T>,
			pool_id: <T as pallet_stableswap::Config>::AssetId,
			asset_id: <T as pallet_omnipool::Config>::AssetId,
		) -> DispatchResult {
			<T as Config>::CreatePoolOrigin::ensure_origin(origin.clone())?;

			// Load state - return AssetNotFound if it does not exist
			let asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_id)?;

			let subpool_pool_state = pallet_omnipool::Pallet::<T>::load_asset_state(pool_id.into())?;

			let omnipool_account = pallet_omnipool::Pallet::<T>::protocol_account();

			// Add token to subpool
			// this might require moving from one pool account to another - depends on how AccountIdFor is implemented!
			pallet_stableswap::Pallet::<T>::add_asset_to_existing_pool(pool_id, asset_id.into())?;

			// Move liquidity from omnipool account to subpool
			pallet_stableswap::Pallet::<T>::move_liquidity_to_pool(
				&omnipool_account,
				pool_id,
				&[AssetLiquidity::<<T as pallet_stableswap::Config>::AssetId> {
					asset_id: asset_id.into(),
					amount: asset_state.reserve,
				}],
			)?;

			// Remove token from omnipool
			pallet_omnipool::Pallet::<T>::remove_asset(asset_id)?;

			let delta_q = asset_state.hub_reserve;
			let delta_ps = asset_state.protocol_shares;
			let delta_s = asset_state.hub_reserve * subpool_pool_state.shares / subpool_pool_state.hub_reserve;
			let delta_u = asset_state.hub_reserve * subpool_pool_state.reserve / subpool_pool_state.hub_reserve;

			pallet_omnipool::Pallet::<T>::update_asset_state(
				pool_id.into(),
				delta_q,
				delta_s,
				delta_ps,
				asset_state.cap,
			)?;

			pallet_stableswap::Pallet::<T>::deposit_shares(&omnipool_account, pool_id.into(), delta_u)?;

			// Remember some stuff to be able to update LP positions later on
			let asset_details = AssetDetail {
				price: Default::default(), //TODO: correct price
				shares: asset_state.shares,
				hub_reserve: delta_q,
				share_tokens: delta_u,
			};

			MigratedAssets::<T>::insert(asset_id, (pool_id, asset_details));

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			asset_id: <T as pallet_omnipool::Config>::AssetId,
			amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			// Figure out where is the asset - isopool or subpool
			// if supbpool - do add liquidity to subpool and then call omnipool's add_liquidity with shares to mint position
			// if isopool - call omnipool::add_liquidity

			if let Some((pool_id, _)) = MigratedAssets::<T>::get(&asset_id) {
				let shares = pallet_stableswap::Pallet::<T>::do_add_liquidity(
					&who,
					pool_id,
					&[AssetLiquidity {
						asset_id: asset_id.into(),
						amount,
					}],
				)?;
				pallet_omnipool::Pallet::<T>::add_liquidity(origin, pool_id.into(), shares)
			} else {
				pallet_omnipool::Pallet::<T>::add_liquidity(origin, asset_id, amount)
			}
		}

		#[pallet::weight(0)]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			asset_id: <T as pallet_omnipool::Config>::AssetId,
			position_id: Option<T::PositionInstanceId>,
			share_amount: Balance,
		) -> DispatchResult {
			<T as Config>::CreatePoolOrigin::ensure_origin(origin.clone())?;

			// Figure out where is the asset - isopools or subpool
			// - if subpool and position provided, update it first to current and follow the spec
			// - if subpool but no position, only sahres -> call stableswap::remove_liquid
			// - if isopool and position provided -> call omnipool::remove_liquid
			// - if isopool but no position -> Error

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: <T as pallet_omnipool::Config>::AssetId,
			asset_out: <T as pallet_omnipool::Config>::AssetId,
			amount: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			// Figure out where each asset is - isopool or subpool
			// - if both in isopool - call omnipool sell
			// - if both in same subpool - call stableswap::sell
			// - if both in different subpool - handle here according to spec
			// - if mixed - handle here according to spec

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: <T as pallet_omnipool::Config>::AssetId,
			asset_in: <T as pallet_omnipool::Config>::AssetId,
			amount: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			// Figure out where each asset is - isopool or subpool
			// - if both in isopool - call omnipool buy
			// - if both in same subpool - call stableswap buy
			// - if both in different subpool - handle here according to spec
			// - if mixed - handle here according to spec

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}
