// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

//#[cfg(any(feature = "runtime-benchmarks", test))]
//mod benchmarks;

#[cfg(test)]
mod tests;

pub mod migration;
//pub mod weights;

use frame_support::{
	ensure,
	pallet_prelude::{DispatchError, DispatchResult},
	sp_runtime::traits::{AccountIdConversion, Zero},
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate, Transfer},
		Get,
	},
	PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use hydradx_traits::liquidity_mining::{GlobalFarmId, Mutate as LiquidityMiningMutate, YieldFarmId};
use orml_traits::MultiCurrency;
use pallet_liquidity_mining::{FarmMultiplier, LoyaltyCurve};
use pallet_omnipool::{types::Position as OmniPosition, NFTCollectionIdOf};
use primitives::{Balance, ItemId as DepositId};
use sp_runtime::{ArithmeticError, FixedPointNumber, FixedU128, Perquintill};
use sp_std::vec;

pub use pallet::*;
//pub use weights::WeightInfo;

type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;
type PeriodOf<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_omnipool::Config<PositionItemId = DepositId> {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Currency for transfers.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// The origin account that can create new liquidity mining program.
		type CreateOrigin: EnsureOrigin<Self::Origin>;

		/// Pallet id.
		type PalletId: Get<PalletId>;

		/// NFT collection id for liq. mining deposit nfts. Has to be within the range of reserved NFT class IDs.
		#[pallet::constant]
		type NFTCollectionId: Get<NFTCollectionIdOf<Self>>;

		/// Non fungible handling
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, ItemId = Self::PositionItemId, CollectionId = Self::CollectionId>
			+ Transfer<Self::AccountId>;

		/// Liquidity mining handler for managing liquidity mining functionalities
		type LiquidityMiningHandler: LiquidityMiningMutate<
			Self::AccountId,
			Self::AssetId,
			BlockNumberFor<Self>,
			Error = DispatchError,
			AmmPoolId = Self::AssetId,
			Balance = Balance,
			LoyaltyCurve = LoyaltyCurve,
			Period = PeriodOf<Self>,
		>;
	}

	#[pallet::storage]
	/// Map of omnipool position's ids to LM's deposit ids.
	pub(super) type OmniPositionId<T: Config> =
		StorageMap<_, Blake2_128Concat, DepositId, T::PositionItemId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New global farm was created.
		GlobalFarmCreated {
			id: GlobalFarmId,
			owner: T::AccountId,
			total_rewards: Balance,
			reward_currency: T::AssetId,
			yield_per_period: Perquintill,
			planned_yielding_periods: PeriodOf<T>,
			blocks_per_period: BlockNumberFor<T>,
			max_reward_per_period: Balance,
			min_deposit: Balance,
			price_adjustment: FixedU128,
		},

		/// Global farm's `price_adjustment` was updated.
		GlobalFarmUpdated {
			id: GlobalFarmId,
			price_adjustment: FixedU128,
		},

		/// Global farm was terminated.
		GlobalFarmTerminated {
			global_farm_id: GlobalFarmId,
			who: T::AccountId,
			reward_currency: T::AssetId,
			undistributed_rewards: Balance,
		},

		/// New yield farm was added into the farm.
		YieldFarmCreated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			multiplier: FarmMultiplier,
			loyalty_curve: Option<LoyaltyCurve>,
		},

		/// Yield farm multiplier was updated.
		YieldFarmUpdated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
			multiplier: FarmMultiplier,
		},

		/// Yield farm for `asset_id` was stopped.
		YieldFarmStopped {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
		},

		/// Yield farm for `asset_id` was resumed.
		YieldFarmResumed {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
			multiplier: FarmMultiplier,
		},

		/// Yield farm was terminated from the global farm.
		YieldFarmTerminated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
		},

		SharesDeposited {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			deposit_id: DepositId,
			asset_id: T::AssetId,
			who: T::AccountId,
			shares_amount: Balance,
			position_id: T::PositionItemId,
		},

		SharesRedeposited {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			deposit_id: DepositId,
			asset_id: T::AssetId,
			who: T::AccountId,
			shares_amount: Balance,
			position_id: T::PositionItemId,
		},

		RewardClaimed {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			who: T::AccountId,
			claimed: Balance,
			reward_currency: T::AssetId,
			deposit_id: DepositId,
		},

		SharesWithdrawn {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			who: T::AccountId,
			amount: Balance,
			deposit_id: DepositId,
		},

		DepositDestroyed {
			who: T::AccountId,
			deposit_id: DepositId,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Asset is not in omnipool
		AssetNotFound,

		/// Signed account is not owner of omnipool's position instance.
		Forbidden,

		/// `deposit_id` to `position_id` association was not fond in the storage.
		MissingLpPosition,

		CantFindDepositOwner,

		NotDepositOwner,

		ZeroClaimedRewards,

		DepositDataNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1_000)]
		pub fn create_global_farm(
			origin: OriginFor<T>,
			total_rewards: Balance,
			planned_yielding_periods: PeriodOf<T>,
			blocks_per_period: BlockNumberFor<T>,
			reward_currency: T::AssetId,
			owner: T::AccountId,
			yield_per_period: Perquintill,
			min_deposit: Balance,
			price_adjustment: FixedU128,
		) -> DispatchResult {
			<T as pallet::Config>::CreateOrigin::ensure_origin(origin)?;

			let (id, max_reward_per_period) = T::LiquidityMiningHandler::create_global_farm(
				total_rewards,
				planned_yielding_periods,
				blocks_per_period,
				<T as pallet_omnipool::Config>::HubAssetId::get(),
				reward_currency,
				owner.clone(),
				yield_per_period,
				min_deposit,
				price_adjustment,
			)?;

			Self::deposit_event(Event::GlobalFarmCreated {
				id,
				owner,
				total_rewards,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				max_reward_per_period,
				min_deposit,
				price_adjustment,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn update_global_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			price_adjustment: FixedU128,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			T::LiquidityMiningHandler::update_global_farm_price_adjustment(who, global_farm_id, price_adjustment)?;

			Self::deposit_event(Event::GlobalFarmUpdated {
				id: global_farm_id,
				price_adjustment,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn terminate_global_farm(origin: OriginFor<T>, global_farm_id: GlobalFarmId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (reward_currency, undistributed_rewards, who) =
				T::LiquidityMiningHandler::terminate_global_farm(who, global_farm_id)?;

			Self::deposit_event(Event::GlobalFarmTerminated {
				global_farm_id,
				who,
				reward_currency,
				undistributed_rewards,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn create_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			asset_id: T::AssetId,
			multiplier: FarmMultiplier,
			loyalty_curve: Option<LoyaltyCurve>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(OmnipoolPallet::<T>::exists(asset_id), Error::<T>::AssetNotFound);

			let yield_farm_id = T::LiquidityMiningHandler::create_yield_farm(
				who,
				global_farm_id,
				multiplier,
				loyalty_curve.clone(),
				asset_id,
				vec![asset_id, <T as pallet_omnipool::Config>::HubAssetId::get()],
			)?;

			Self::deposit_event(Event::YieldFarmCreated {
				global_farm_id,
				yield_farm_id,
				asset_id,
				multiplier,
				loyalty_curve,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn update_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			asset_id: T::AssetId,
			multiplier: FarmMultiplier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(OmnipoolPallet::<T>::exists(asset_id), Error::<T>::AssetNotFound);

			let yield_farm_id = T::LiquidityMiningHandler::update_yield_farm_multiplier(
				who.clone(),
				global_farm_id,
				asset_id,
				multiplier,
			)?;

			Self::deposit_event(Event::YieldFarmUpdated {
				global_farm_id,
				yield_farm_id,
				asset_id,
				multiplier,
				who,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn stop_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			asset_id: T::AssetId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			//NOTE: don't check pool existance, owner must be able to stop yield farm.
			let yield_farm_id = T::LiquidityMiningHandler::stop_yield_farm(who.clone(), global_farm_id, asset_id)?;

			Self::deposit_event(Event::YieldFarmStopped {
				global_farm_id,
				yield_farm_id,
				asset_id,
				who,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn resume_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			multiplier: FarmMultiplier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(OmnipoolPallet::<T>::exists(asset_id), Error::<T>::AssetNotFound);

			T::LiquidityMiningHandler::resume_yield_farm(
				who.clone(),
				global_farm_id,
				yield_farm_id,
				asset_id,
				multiplier,
			)?;

			Self::deposit_event(Event::<T>::YieldFarmResumed {
				global_farm_id,
				yield_farm_id,
				asset_id,
				who,
				multiplier,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn terminate_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			//NOTE: don't check XYK existance, owner must be able to termiante yield farm.
			T::LiquidityMiningHandler::terminate_yield_farm(who.clone(), global_farm_id, yield_farm_id, asset_id)?;

			Self::deposit_event(Event::YieldFarmTerminated {
				global_farm_id,
				yield_farm_id,
				asset_id,
				who,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn deposit_shares(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			position_id: T::PositionItemId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let lp_position = OmnipoolPallet::<T>::load_position(position_id, who.clone())?;

			ensure!(
				OmnipoolPallet::<T>::exists(lp_position.asset_id),
				Error::<T>::AssetNotFound
			);

			let deposit_id = T::LiquidityMiningHandler::deposit_lp_shares(
				global_farm_id,
				yield_farm_id,
				lp_position.asset_id,
				lp_position.shares,
				|_, _, _| -> Result<Balance, DispatchError> { Self::get_position_value_in_hub_asset(&lp_position) },
			)?;

			Self::lock_lp_position(position_id, deposit_id)?;
			<T as pallet::Config>::NFTHandler::mint_into(
				&<T as pallet::Config>::NFTCollectionId::get(),
				&deposit_id,
				&who,
			)?;

			Self::deposit_event(Event::SharesDeposited {
				global_farm_id,
				yield_farm_id,
				deposit_id,
				asset_id: lp_position.asset_id,
				who,
				shares_amount: lp_position.shares,
				position_id,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn redeposit_shares(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			deposit_id: DepositId,
		) -> DispatchResult {
			let owner = Self::ensure_nft_owner(origin, deposit_id)?;

			//NOTE: not tested this should never fail.
			let position_id = OmniPositionId::<T>::get(deposit_id).ok_or(Error::<T>::MissingLpPosition)?;

			//NOTE: pallet should be owner of the omnipool position at this point.
			let lp_position = OmnipoolPallet::<T>::load_position(position_id, Self::account_id())?;
			ensure!(
				OmnipoolPallet::<T>::exists(lp_position.asset_id),
				Error::<T>::AssetNotFound
			);

			T::LiquidityMiningHandler::redeposit_lp_shares(global_farm_id, yield_farm_id, deposit_id, |_, _, _| {
				Self::get_position_value_in_hub_asset(&lp_position)
			})?;

			Self::deposit_event(Event::SharesRedeposited {
				global_farm_id,
				yield_farm_id,
				deposit_id,
				asset_id: lp_position.asset_id,
				who: owner,
				shares_amount: lp_position.shares,
				position_id,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			deposit_id: DepositId,
			yield_farm_id: YieldFarmId,
		) -> DispatchResult {
			let owner = Self::ensure_nft_owner(origin, deposit_id)?;

			let (global_farm_id, reward_currency, claimed, _) =
				T::LiquidityMiningHandler::claim_rewards(owner.clone(), deposit_id, yield_farm_id)?;

			ensure!(!claimed.is_zero(), Error::<T>::ZeroClaimedRewards);

			Self::deposit_event(Event::RewardClaimed {
				global_farm_id,
				yield_farm_id,
				who: owner,
				claimed,
				reward_currency,
				deposit_id,
			});

			Ok(())
		}

		#[pallet::weight(1_000)]
		pub fn withdraw_shares(
			origin: OriginFor<T>,
			deposit_id: DepositId,
			yield_farm_id: YieldFarmId,
		) -> DispatchResult {
			let owner = Self::ensure_nft_owner(origin, deposit_id)?;

			//NOTE: not tested -this should never fail.
			let position_id = OmniPositionId::<T>::get(deposit_id).ok_or(Error::<T>::MissingLpPosition)?;
			let lp_position = OmnipoolPallet::<T>::load_position(position_id, Self::account_id())?;

			//NOTE: not tested -this should never fail.
			let global_farm_id = T::LiquidityMiningHandler::get_global_farm_id(deposit_id, yield_farm_id)
				.ok_or(Error::<T>::DepositDataNotFound)?;

			let (withdrawn_amount, claim_data, is_destroyed) = T::LiquidityMiningHandler::withdraw_lp_shares(
				owner.clone(),
				deposit_id,
				global_farm_id,
				yield_farm_id,
				lp_position.asset_id,
			)?;

			if let Some((reward_currency, claimed, _)) = claim_data {
				if !claimed.is_zero() {
					Self::deposit_event(Event::RewardClaimed {
						global_farm_id,
						yield_farm_id,
						who: owner.clone(),
						claimed,
						reward_currency,
						deposit_id,
					});
				}
			}

			if !withdrawn_amount.is_zero() {
				Self::deposit_event(Event::SharesWithdrawn {
					global_farm_id,
					yield_farm_id,
					who: owner.clone(),
					amount: withdrawn_amount,
					deposit_id,
				});
			}

			if is_destroyed {
				Self::unlock_lp_postion(deposit_id, &owner)?;
				<T as pallet::Config>::NFTHandler::burn(
					&<T as pallet::Config>::NFTCollectionId::get(),
					&deposit_id,
					Some(&owner),
				)?;

				Self::deposit_event(Event::DepositDestroyed { who: owner, deposit_id });
			}

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		//fn integrity_test() { }
	}
}

impl<T: Config> Pallet<T> {
	/// Account ID of the pot holding all the locked NFTs. This account is also owner of the NFT
	/// collection used to mint LM's NFTs.
	fn account_id() -> T::AccountId {
		<T as pallet::Config>::PalletId::get().into_account_truncating()
	}

	/// This function transfers omnipool's position NFT to LM's account and saves deposit to
	/// omnipool's position pairing to storage.
	fn lock_lp_position(position_id: T::PositionItemId, deposit_id: DepositId) -> Result<(), DispatchError> {
		<T as pallet::Config>::NFTHandler::transfer(
			&<T as pallet_omnipool::Config>::NFTCollectionId::get(),
			&position_id,
			&Self::account_id(),
		)?;

		//Map `deposit_id` to `position_id` so we know which position to unlock when deposit is destroyed.
		OmniPositionId::<T>::insert(deposit_id, position_id);

		Ok(())
	}

	/// This function transfers omnipool's NFT associated with `deposit_id` to `who` and remove
	/// deposit to omnipool's position pairing from storage.
	fn unlock_lp_postion(deposit_id: DepositId, who: &T::AccountId) -> Result<(), DispatchError> {
		OmniPositionId::<T>::try_mutate_exists(deposit_id, |maybe_position_id| -> DispatchResult {
			//NOTE: this should never fail
			//TODO: move thi error to InconsistentState errors
			let lp_position_id = maybe_position_id.as_mut().ok_or(Error::<T>::MissingLpPosition)?;

			<T as pallet::Config>::NFTHandler::transfer(
				&<T as pallet_omnipool::Config>::NFTCollectionId::get(),
				lp_position_id,
				who,
			)?;

			//NOTE: storage clean up
			*maybe_position_id = None;

			Ok(())
		})
	}

	/// This function returns value of omnipool's lp postion in [`LRNA`].
	fn get_position_value_in_hub_asset(
		lp_position: &OmniPosition<Balance, T::AssetId>,
	) -> Result<Balance, DispatchError> {
		let state = OmnipoolPallet::<T>::load_asset_state(lp_position.asset_id)?;

		state
			.price()
			.ok_or(ArithmeticError::DivisionByZero)?
			.checked_mul_int(lp_position.amount)
			.ok_or(ArithmeticError::Overflow.into())
	}

	fn ensure_nft_owner(origin: OriginFor<T>, deposit_id: DepositId) -> Result<T::AccountId, DispatchError> {
		let who = ensure_signed(origin)?;

		let nft_owner =
			<T as pallet::Config>::NFTHandler::owner(&<T as pallet::Config>::NFTCollectionId::get(), &deposit_id)
				.ok_or(Error::<T>::CantFindDepositOwner)?;

		ensure!(nft_owner == who, Error::<T>::NotDepositOwner);

		Ok(who)
	}
}
