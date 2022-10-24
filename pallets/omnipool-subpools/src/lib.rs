#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::DispatchResult;
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::Permill;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub (crate) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId: Into<<T as pallet_stableswap::Config>::AssetId>,
	{
		#[pallet::weight(0)]
		pub fn create_subpool(
			origin: OriginFor<T>,
			asset_id: <T as pallet_omnipool::Config>::AssetId,
			amplification: u16,
			trade_fee: Permill,
			withdraw_fee: Permill,
		) -> DispatchResult {
			<T as Config>::CreatePoolOrigin::ensure_origin(origin.clone())?;

			// Load state - return AssetNotFound if it does not exist
			let _asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_id)?;

			// Create new subpool
			pallet_stableswap::Pallet::<T>::create_pool(
				origin,
				vec![asset_id.into()],
				amplification,
				trade_fee,
				withdraw_fee,
			)?;

			// Move liquidity from omnipool account to subpool

			// Remove token from omnipool

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn add_token_to_subpool(
			origin: OriginFor<T>,
			_pool_id: <T as pallet_stableswap::Config>::AssetId,
			asset_id: <T as pallet_omnipool::Config>::AssetId,
		) -> DispatchResult {
			<T as Config>::CreatePoolOrigin::ensure_origin(origin.clone())?;

			// Load state - return AssetNotFound if it does not exist
			let _asset_state = pallet_omnipool::Pallet::<T>::load_asset_state(asset_id)?;

			// Add token to subpool
			// this might require moving from one pool account to another - depends on how AccountIdFor is implemented!

			// Move liquidity from omnipool account to subpool

			// Remove token from omnipool

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}
