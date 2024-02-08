// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

//! # Omnipool pallet
//!
//! Omnipool implementation
//!
//! ## Overview
//!
//! Omnipool is type of AMM where all assets are pooled together into one single pool.
//!
//! Each asset is internally paired with so called Hub Asset ( LRNA ). When a liquidity is provided, corresponding
//! amount of hub asset is minted. When a liquidity is removed, corresponding amount of hub asset is burned.
//!
//! Liquidity provider can provide any asset of their choice to the Omnipool and in return
//! they will receive pool shares for this single asset.
//!
//! The position is represented as a NFT token which stores the amount of shares distributed
//! and the price of the asset at the time of provision.
//!
//! For traders this means that they can benefit from non-fragmented liquidity.
//! They can send any token to the pool using the swap mechanism
//! and in return they will receive the token of their choice in the appropriate quantity.
//!
//! Omnipool is implemented with concrete Balance type: u128.
//!
//! ### Imbalance mechanism
//! The Imbalance mechanism is designed to stabilize the value of LRNA. By design it is a weak and passive mechanism,
//! and is specifically meant to deal with one cause of LRNA volatility: LRNA being sold back to the pool.
//!
//! Imbalance is always negative, internally represented by a special type `SimpleImbalance` which uses unsigned integer and boolean flag.
//! This was done initially because of the intention that in future imbalance can also become positive.
//!
//! ### Omnipool Hooks
//!
//! Omnipool pallet supports multiple hooks which are triggerred on certain operations:
//! - on_liquidity_changed - called when liquidity is added or removed from the pool
//! - on_trade - called when trade is executed
//! - on_trade_fee - called after successful trade with fee amount that can be taken out of the pool if needed.
//!
//! This is currently used to update on-chain oracle and in the circuit breaker.
//!
//! ## Terminology
//!
//! * **LP:**  liquidity provider
//! * **Position:**  a moment when LP added liquidity to the pool. It stores amount,shares and price at the time
//!  of provision
//! * **Hub Asset:** dedicated 'hub' token for trade executions (LRNA)
//! * **Native Asset:** governance token
//! * **Imbalance:** Tracking of hub asset imbalance.
//!
//! ## Assumptions
//!
//! Below are assumptions that must be held when using this pallet.
//!
//! * Initial liquidity of new token being added to Omnipool must be transferred manually to pool account prior to calling add_token.
//! * All tokens added to the pool must be first registered in Asset Registry.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `add_token` - Adds token to the pool. Initial liquidity must be transffered to pool account prior to calling add_token.
//! * `add_liquidity` - Adds liquidity of selected asset to the pool. Mints corresponding position NFT.
//! * `remove_liquidity` - Removes liquidity of selected position from the pool. Partial withdrawals are allowed.
//! * `sell` - Trades an asset in for asset out by selling given amount of asset in.
//! * `buy` - Trades an asset in for asset out by buying given amount of asset out.
//! * `set_asset_tradable_state` - Updates asset's tradable state with new flags. This allows/forbids asset operation such SELL,BUY,ADD or  REMOVE liquidtityy.
//! * `refund_refused_asset` - Refunds the initial liquidity amount sent to pool account prior to add_token if the token has been refused to be added.
//! * `sacrifice_position` - Destroys a position and position's shares become protocol's shares.
//! * `withdraw_protocol_liquidity` - Withdraws protocol's liquidity from the pool. Used to withdraw liquidity from sacrificed position.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::{DispatchResult, Get};
use frame_support::require_transactional;
use frame_support::PalletId;
use frame_support::{ensure, transactional};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, One};
use sp_runtime::traits::{CheckedAdd, CheckedSub, Zero};
use sp_std::ops::{Add, Sub};
use sp_std::prelude::*;

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use hydra_dx_math::omnipool::types::{AssetStateChange, BalanceUpdate, I129};
use hydradx_traits::Registry;
use orml_traits::{GetByKey, MultiCurrency};
use scale_info::TypeInfo;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128, Permill};

#[cfg(test)]
mod tests;

pub mod provider;
pub mod router_execution;
pub mod traits;
pub mod types;
pub mod weights;

use crate::traits::{AssetInfo, OmnipoolHooks};
use crate::types::{AssetReserveState, AssetState, Balance, Position, SimpleImbalance, Tradability};
pub use pallet::*;
pub use weights::WeightInfo;

/// NFT class id type of provided nft implementation
pub type NFTCollectionIdOf<T> =
	<<T as Config>::NFTHandler as Inspect<<T as frame_system::Config>::AccountId>>::CollectionId;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::traits::{AssetInfo, ExternalPriceProvider, OmnipoolHooks, ShouldAllow};
	use crate::types::{Position, Price, Tradability};
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::DefensiveOption;
	use frame_system::pallet_prelude::*;
	use hydra_dx_math::ema::EmaPrice;
	use hydra_dx_math::omnipool::types::{BalanceUpdate, I129};
	use orml_traits::GetByKey;
	use sp_runtime::ArithmeticError;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Origin that can add token, refund refused asset and withdraw protocol liquidity.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Origin that can change asset's tradability and weight.
		type TechnicalOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		/// Native Asset ID
		#[pallet::constant]
		type HdxAssetId: Get<Self::AssetId>;

		/// Hub Asset ID
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Dynamic fee support - returns (Asset Fee, Protocol Fee) for given asset
		type Fee: GetByKey<Self::AssetId, (Permill, Permill)>;

		/// Minimum withdrawal fee
		#[pallet::constant]
		type MinWithdrawalFee: Get<Permill>;

		/// Minimum trading limit
		#[pallet::constant]
		type MinimumTradingLimit: Get<Balance>;

		/// Minimum pool liquidity which can be added
		#[pallet::constant]
		type MinimumPoolLiquidity: Get<Balance>;

		/// Max fraction of asset reserve to sell in single transaction
		#[pallet::constant]
		type MaxInRatio: Get<u128>;

		/// Max fraction of asset reserve to buy in single transaction
		#[pallet::constant]
		type MaxOutRatio: Get<u128>;

		/// Position identifier type
		type PositionItemId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Collection id type
		type CollectionId: TypeInfo + MaxEncodedLen;

		/// Non fungible class id
		#[pallet::constant]
		type NFTCollectionId: Get<NFTCollectionIdOf<Self>>;

		/// Non fungible handling - mint,burn, check owner
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, ItemId = Self::PositionItemId, CollectionId = Self::CollectionId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Hooks are actions executed on add_liquidity, sell or buy.
		type OmnipoolHooks: OmnipoolHooks<
			Self::RuntimeOrigin,
			Self::AccountId,
			Self::AssetId,
			Balance,
			Error = DispatchError,
		>;

		/// Safety mechanism when adding and removing liquidity. Determines how much price can change between spot price and oracle price.
		type PriceBarrier: ShouldAllow<Self::AccountId, Self::AssetId, EmaPrice>;

		/// Oracle price provider. Provides price for given asset. Used in remove liquidity to support calculation of dynamic withdrawal fee.
		type ExternalPriceOracle: ExternalPriceProvider<Self::AssetId, EmaPrice, Error = DispatchError>;
	}

	#[pallet::storage]
	/// State of an asset in the omnipool
	#[pallet::getter(fn assets)]
	pub(super) type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, AssetState<Balance>>;

	#[pallet::storage]
	/// Imbalance of hub asset
	#[pallet::getter(fn current_imbalance)]
	pub(super) type HubAssetImbalance<T: Config> = StorageValue<_, SimpleImbalance<Balance>, ValueQuery>;

	// LRNA is only allowed to be sold
	#[pallet::type_value]
	pub fn DefaultHubAssetTradability() -> Tradability {
		Tradability::SELL
	}

	#[pallet::storage]
	/// Tradable state of hub asset.
	pub(super) type HubAssetTradability<T: Config> =
		StorageValue<_, Tradability, ValueQuery, DefaultHubAssetTradability>;

	#[pallet::storage]
	/// LP positions. Maps NFT instance id to corresponding position
	#[pallet::getter(fn positions)]
	pub(super) type Positions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PositionItemId, Position<Balance, T::AssetId>>;

	#[pallet::storage]
	#[pallet::getter(fn next_position_id)]
	/// Position ids sequencer
	pub(super) type NextPositionId<T: Config> = StorageValue<_, T::PositionItemId, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An asset was added to Omnipool
		TokenAdded {
			asset_id: T::AssetId,
			initial_amount: Balance,
			initial_price: Price,
		},
		/// An asset was removed from Omnipool
		TokenRemoved {
			asset_id: T::AssetId,
			amount: Balance,
			hub_withdrawn: Balance,
		},
		/// Liquidity of an asset was added to Omnipool.
		LiquidityAdded {
			who: T::AccountId,
			asset_id: T::AssetId,
			amount: Balance,
			position_id: T::PositionItemId,
		},
		/// Liquidity of an asset was removed from Omnipool.
		LiquidityRemoved {
			who: T::AccountId,
			position_id: T::PositionItemId,
			asset_id: T::AssetId,
			shares_removed: Balance,
			fee: FixedU128,
		},
		/// PRotocol Liquidity was removed from Omnipool.
		ProtocolLiquidityRemoved {
			who: T::AccountId,
			asset_id: T::AssetId,
			amount: Balance,
			hub_amount: Balance,
			shares_removed: Balance,
		},
		/// Sell trade executed.
		SellExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			hub_amount_in: Balance,
			hub_amount_out: Balance,
			asset_fee_amount: Balance,
			protocol_fee_amount: Balance,
		},
		/// Buy trade executed.
		BuyExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			hub_amount_in: Balance,
			hub_amount_out: Balance,
			asset_fee_amount: Balance,
			protocol_fee_amount: Balance,
		},
		/// LP Position was created and NFT instance minted.
		PositionCreated {
			position_id: T::PositionItemId,
			owner: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
			shares: Balance,
			price: Price,
		},
		/// LP Position was destroyed and NFT instance burned.
		PositionDestroyed {
			position_id: T::PositionItemId,
			owner: T::AccountId,
		},
		/// LP Position was updated.
		PositionUpdated {
			position_id: T::PositionItemId,
			owner: T::AccountId,
			asset: T::AssetId,
			amount: Balance,
			shares: Balance,
			price: Price,
		},
		/// Asset's tradable state has been updated.
		TradableStateUpdated { asset_id: T::AssetId, state: Tradability },

		/// Amount has been refunded for asset which has not been accepted to add to omnipool.
		AssetRefunded {
			asset_id: T::AssetId,
			amount: Balance,
			recipient: T::AccountId,
		},

		/// Asset's weight cap has been updated.
		AssetWeightCapUpdated { asset_id: T::AssetId, cap: Permill },
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,
		/// Asset is already in omnipool
		AssetAlreadyAdded,
		/// Asset is not in omnipool
		AssetNotFound,
		/// Failed to add token to Omnipool due to insufficient initial liquidity.
		MissingBalance,
		/// Invalid initial asset price.
		InvalidInitialAssetPrice,
		/// Slippage protection - minimum limit has not been reached.
		BuyLimitNotReached,
		/// Slippage protection - maximum limit has been exceeded.
		SellLimitExceeded,
		/// Position has not been found.
		PositionNotFound,
		/// Insufficient shares in position
		InsufficientShares,
		/// Asset is not allowed to be traded.
		NotAllowed,
		/// Signed account is not owner of position instance.
		Forbidden,
		/// Asset weight cap has been exceeded.
		AssetWeightCapExceeded,
		/// Asset is not registered in asset registry
		AssetNotRegistered,
		/// Provided liquidity is below minimum allowed limit
		InsufficientLiquidity,
		/// Traded amount is below minimum allowed limit
		InsufficientTradingAmount,
		/// Sell or buy with same asset ids is not allowed.
		SameAssetTradeNotAllowed,
		/// LRNA update after trade results in positive value.
		HubAssetUpdateError,
		/// Imbalance results in positive value.
		PositiveImbalance,
		/// Amount of shares provided cannot be 0.
		InvalidSharesAmount,
		/// Hub asset is only allowed to be sold.
		InvalidHubAssetTradableState,
		/// Asset is not allowed to be refunded.
		AssetRefundNotAllowed,
		/// Max fraction of asset to buy has been exceeded.
		MaxOutRatioExceeded,
		/// Max fraction of asset to sell has been exceeded.
		MaxInRatioExceeded,
		/// Max allowed price difference has been exceeded.
		PriceDifferenceTooHigh,
		/// Invalid oracle price - division by zero.
		InvalidOraclePrice,
		/// Failed to calculate withdrawal fee.
		InvalidWithdrawalFee,
		/// More than allowed amount of fee has been transferred.
		FeeOverdraft,
		/// Token cannot be removed from Omnipool due to shares still owned by other users.
		SharesRemaining,
		/// Token cannot be removed from Omnipool because asset is not frozen.
		AssetNotFrozen,
		/// Calculated amount out from sell trade is zero.
		ZeroAmountOut,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add new token to omnipool in quantity `amount` at price `initial_price`
		///
		/// Initial liquidity must be transferred to pool's account for this new token manually prior to calling `add_token`.
		///
		/// Initial liquidity is pool's account balance of the token.
		///
		/// Position NFT token is minted for `position_owner`.
		///
		/// Parameters:
		/// - `asset`: The identifier of the new asset added to the pool. Must be registered in Asset registry
		/// - `initial_price`: Initial price
		/// - `position_owner`: account id for which share are distributed in form on NFT
		/// - `weight_cap`: asset weight cap
		///
		/// Emits `TokenAdded` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::add_token().saturating_add(T::OmnipoolHooks::on_liquidity_changed_weight()))]
		#[transactional]
		pub fn add_token(
			origin: OriginFor<T>,
			asset: T::AssetId,
			initial_price: Price,
			weight_cap: Permill,
			position_owner: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin.clone())?;

			ensure!(!Assets::<T>::contains_key(asset), Error::<T>::AssetAlreadyAdded);

			ensure!(T::AssetRegistry::exists(asset), Error::<T>::AssetNotRegistered);

			ensure!(initial_price > FixedU128::zero(), Error::<T>::InvalidInitialAssetPrice);

			// ensure collection is created, we can simply ignore the error if it was already created.
			let _ = T::NFTHandler::create_collection(
				&T::NFTCollectionId::get(),
				&Self::protocol_account(),
				&Self::protocol_account(),
			);

			let amount = T::Currency::free_balance(asset, &Self::protocol_account());

			ensure!(
				amount >= T::MinimumPoolLiquidity::get() && amount > 0,
				Error::<T>::MissingBalance
			);

			let hub_reserve = initial_price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

			// Initial state of asset
			let state = AssetState::<Balance> {
				hub_reserve,
				shares: amount,
				protocol_shares: Balance::zero(),
				cap: FixedU128::from(weight_cap).into_inner(),
				tradable: Tradability::default(),
			};

			let lp_position = Position::<Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: amount,
				price: (initial_price.into_inner(), FixedU128::DIV),
			};

			let instance_id = Self::create_and_mint_position_instance(&position_owner)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: position_owner,
				asset,
				amount,
				shares: amount,
				price: initial_price,
			});

			let current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			let delta_imbalance = hydra_dx_math::omnipool::calculate_delta_imbalance(
				hub_reserve,
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
			)
			.ok_or(ArithmeticError::Overflow)?;

			Self::update_imbalance(BalanceUpdate::Decrease(delta_imbalance))?;

			let delta_hub_reserve = BalanceUpdate::Increase(hub_reserve);
			Self::update_hub_asset_liquidity(&delta_hub_reserve)?;

			let reserve = T::Currency::free_balance(asset, &Self::protocol_account());

			let reserve_state: AssetReserveState<_> = (state.clone(), reserve).into();
			let changes = AssetStateChange {
				delta_hub_reserve,
				delta_reserve: BalanceUpdate::Increase(reserve),
				delta_shares: BalanceUpdate::Increase(amount),
				delta_protocol_shares: BalanceUpdate::Increase(Balance::zero()),
			};
			T::OmnipoolHooks::on_liquidity_changed(
				origin,
				AssetInfo::new(asset, &AssetReserveState::default(), &reserve_state, &changes, false),
			)?;

			<Assets<T>>::insert(asset, state);

			Self::deposit_event(Event::TokenAdded {
				asset_id: asset,
				initial_amount: amount,
				initial_price,
			});

			Ok(())
		}

		/// Add liquidity of asset `asset` in quantity `amount` to Omnipool
		///
		/// `add_liquidity` adds specified asset amount to Omnipool and in exchange gives the origin
		/// corresponding shares amount in form of NFT at current price.
		///
		/// Asset's tradable state must contain ADD_LIQUIDITY flag, otherwise `NotAllowed` error is returned.
		///
		/// NFT is minted using NTFHandler which implements non-fungibles traits from frame_support.
		///
		/// Asset weight cap must be respected, otherwise `AssetWeightExceeded` error is returned.
		/// Asset weight is ratio between new HubAsset reserve and total reserve of Hub asset in Omnipool.
		///
		/// Add liquidity fails if price difference between spot price and oracle price is higher than allowed by `PriceBarrier`.
		///
		/// Parameters:
		/// - `asset`: The identifier of the new asset added to the pool. Must be already in the pool
		/// - `amount`: Amount of asset added to omnipool
		///
		/// Emits `LiquidityAdded` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity()
			.saturating_add(T::OmnipoolHooks::on_liquidity_changed_weight()
			.saturating_add(T::ExternalPriceOracle::get_price_weight()))
		)]
		#[transactional]
		pub fn add_liquidity(origin: OriginFor<T>, asset: T::AssetId, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(
				amount >= T::MinimumPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidity
			);

			ensure!(
				T::Currency::ensure_can_withdraw(asset, &who, amount).is_ok(),
				Error::<T>::InsufficientBalance
			);

			let asset_state = Self::load_asset_state(asset)?;

			ensure!(
				asset_state.tradable.contains(Tradability::ADD_LIQUIDITY),
				Error::<T>::NotAllowed
			);

			T::PriceBarrier::ensure_price(
				&who,
				T::HubAssetId::get(),
				asset,
				EmaPrice::new(asset_state.hub_reserve, asset_state.reserve),
			)
			.map_err(|_| Error::<T>::PriceDifferenceTooHigh)?;

			let current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			//
			// Calculate add liquidity state changes
			//
			let state_changes = hydra_dx_math::omnipool::calculate_add_liquidity_state_changes(
				&(&asset_state).into(),
				amount,
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
			)
			.ok_or(ArithmeticError::Overflow)?;

			let new_asset_state = asset_state
				.clone()
				.delta_update(&state_changes.asset)
				.ok_or(ArithmeticError::Overflow)?;

			let hub_reserve_ratio = FixedU128::checked_from_rational(
				new_asset_state.hub_reserve,
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account())
					.checked_add(*state_changes.asset.delta_hub_reserve)
					.ok_or(ArithmeticError::Overflow)?,
			)
			.ok_or(ArithmeticError::DivisionByZero)?;

			ensure!(
				hub_reserve_ratio <= new_asset_state.weight_cap(),
				Error::<T>::AssetWeightCapExceeded
			);

			// Create LP position with given shares
			let lp_position = Position::<Balance, T::AssetId> {
				asset_id: asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				// Note: position needs price after asset state is updated.
				price: (new_asset_state.hub_reserve, new_asset_state.reserve),
			};

			let instance_id = Self::create_and_mint_position_instance(&who)?;

			<Positions<T>>::insert(instance_id, lp_position);

			Self::deposit_event(Event::PositionCreated {
				position_id: instance_id,
				owner: who.clone(),
				asset,
				amount,
				shares: *state_changes.asset.delta_shares,
				price: new_asset_state.price().ok_or(ArithmeticError::DivisionByZero)?,
			});

			T::Currency::transfer(
				asset,
				&who,
				&Self::protocol_account(),
				*state_changes.asset.delta_reserve,
			)?;

			debug_assert_eq!(*state_changes.asset.delta_reserve, amount);

			// Callback hook info
			let info: AssetInfo<T::AssetId, Balance> =
				AssetInfo::new(asset, &asset_state, &new_asset_state, &state_changes.asset, false);

			Self::update_imbalance(state_changes.delta_imbalance)?;

			Self::update_hub_asset_liquidity(&state_changes.asset.delta_hub_reserve)?;

			Self::set_asset_state(asset, new_asset_state);

			Self::deposit_event(Event::LiquidityAdded {
				who,
				asset_id: asset,
				amount,
				position_id: instance_id,
			});

			T::OmnipoolHooks::on_liquidity_changed(origin, info)?;

			Ok(())
		}

		/// Remove liquidity of asset `asset` in quantity `amount` from Omnipool
		///
		/// `remove_liquidity` removes specified shares amount from given PositionId (NFT instance).
		///
		/// Asset's tradable state must contain REMOVE_LIQUIDITY flag, otherwise `NotAllowed` error is returned.
		///
		/// if all shares from given position are removed, position is destroyed and NFT is burned.
		///
		/// Remove liquidity fails if price difference between spot price and oracle price is higher than allowed by `PriceBarrier`.
		///
		/// Dynamic withdrawal fee is applied if withdrawal is not safe. It is calculated using spot price and external price oracle.
		/// Withdrawal is considered safe when trading is disabled.
		///
		/// Parameters:
		/// - `position_id`: The identifier of position which liquidity is removed from.
		/// - `amount`: Amount of shares removed from omnipool
		///
		/// Emits `LiquidityRemoved` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity().saturating_add(T::OmnipoolHooks::on_liquidity_changed_weight()))]
		#[transactional]
		pub fn remove_liquidity(
			origin: OriginFor<T>,
			position_id: T::PositionItemId,
			amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(amount > Balance::zero(), Error::<T>::InvalidSharesAmount);

			ensure!(
				T::NFTHandler::owner(&T::NFTCollectionId::get(), &position_id) == Some(who.clone()),
				Error::<T>::Forbidden
			);

			let position = Positions::<T>::get(position_id).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(position.shares >= amount, Error::<T>::InsufficientShares);

			let asset_id = position.asset_id;

			let asset_state = Self::load_asset_state(asset_id)?;

			ensure!(
				asset_state.tradable.contains(Tradability::REMOVE_LIQUIDITY),
				Error::<T>::NotAllowed
			);

			let safe_withdrawal = asset_state.tradable.is_safe_withdrawal();
			// Skip price check if safe withdrawal - trading disabled.
			if !safe_withdrawal {
				T::PriceBarrier::ensure_price(
					&who,
					T::HubAssetId::get(),
					asset_id,
					EmaPrice::new(asset_state.hub_reserve, asset_state.reserve),
				)
				.map_err(|_| Error::<T>::PriceDifferenceTooHigh)?;
			}
			let ext_asset_price = T::ExternalPriceOracle::get_price(T::HubAssetId::get(), asset_id)?;

			if ext_asset_price.is_zero() {
				return Err(Error::<T>::InvalidOraclePrice.into());
			}
			let withdrawal_fee = hydra_dx_math::omnipool::calculate_withdrawal_fee(
				asset_state.price().ok_or(ArithmeticError::DivisionByZero)?,
				FixedU128::checked_from_rational(ext_asset_price.n, ext_asset_price.d)
					.defensive_ok_or(Error::<T>::InvalidOraclePrice)?,
				T::MinWithdrawalFee::get(),
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			//
			// calculate state changes of remove liquidity
			//
			let state_changes = hydra_dx_math::omnipool::calculate_remove_liquidity_state_changes(
				&(&asset_state).into(),
				amount,
				&(&position).into(),
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
				withdrawal_fee,
			)
			.ok_or(ArithmeticError::Overflow)?;

			let new_asset_state = asset_state
				.clone()
				.delta_update(&state_changes.asset)
				.ok_or(ArithmeticError::Overflow)?;

			// Update position state
			let updated_position = position
				.delta_update(
					&state_changes.delta_position_reserve,
					&state_changes.delta_position_shares,
				)
				.ok_or(ArithmeticError::Overflow)?;

			T::Currency::transfer(
				asset_id,
				&Self::protocol_account(),
				&who,
				*state_changes.asset.delta_reserve,
			)?;

			Self::update_imbalance(state_changes.delta_imbalance)?;

			// burn only difference between delta hub and lp hub amount.
			Self::update_hub_asset_liquidity(
				&state_changes
					.asset
					.delta_hub_reserve
					.merge(BalanceUpdate::Increase(state_changes.lp_hub_amount))
					.ok_or(ArithmeticError::Overflow)?,
			)?;

			// LP receives some hub asset
			Self::process_hub_amount(state_changes.lp_hub_amount, &who)?;

			if updated_position.shares == Balance::zero() {
				// All liquidity removed, remove position and burn NFT instance

				<Positions<T>>::remove(position_id);
				T::NFTHandler::burn(&T::NFTCollectionId::get(), &position_id, Some(&who))?;

				Self::deposit_event(Event::PositionDestroyed {
					position_id,
					owner: who.clone(),
				});
			} else {
				Self::deposit_event(Event::PositionUpdated {
					position_id,
					owner: who.clone(),
					asset: asset_id,
					amount: updated_position.amount,
					shares: updated_position.shares,
					price: updated_position
						.price_from_rational()
						.ok_or(ArithmeticError::DivisionByZero)?,
				});

				<Positions<T>>::insert(position_id, updated_position);
			}

			// Callback hook info
			let info: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				asset_id,
				&asset_state,
				&new_asset_state,
				&state_changes.asset,
				safe_withdrawal,
			);

			Self::set_asset_state(asset_id, new_asset_state);

			Self::deposit_event(Event::LiquidityRemoved {
				who,
				position_id,
				asset_id,
				shares_removed: amount,
				fee: withdrawal_fee,
			});

			T::OmnipoolHooks::on_liquidity_changed(origin, info)?;

			Ok(())
		}

		/// Sacrifice LP position in favor of pool.
		///
		/// A position is destroyed and liquidity owned by LP becomes pool owned liquidity.
		///
		/// Only owner of position can perform this action.
		///
		/// Emits `PositionDestroyed`.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::sacrifice_position())]
		#[transactional]
		pub fn sacrifice_position(origin: OriginFor<T>, position_id: T::PositionItemId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let position = Positions::<T>::get(position_id).ok_or(Error::<T>::PositionNotFound)?;

			ensure!(
				T::NFTHandler::owner(&T::NFTCollectionId::get(), &position_id) == Some(who.clone()),
				Error::<T>::Forbidden
			);

			Assets::<T>::try_mutate(position.asset_id, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				asset_state.protocol_shares = asset_state
					.protocol_shares
					.checked_add(position.shares)
					.ok_or(ArithmeticError::Overflow)?;

				Ok(())
			})?;

			// Destroy position and burn NFT
			<Positions<T>>::remove(position_id);
			T::NFTHandler::burn(&T::NFTCollectionId::get(), &position_id, Some(&who))?;

			Self::deposit_event(Event::PositionDestroyed {
				position_id,
				owner: who,
			});

			Ok(())
		}

		/// Execute a swap of `asset_in` for `asset_out`.
		///
		/// Price is determined by the Omnipool.
		///
		/// Hub asset is traded separately.
		///
		/// Asset's tradable states must contain SELL flag for asset_in and BUY flag for asset_out, otherwise `NotAllowed` error is returned.
		///
		/// Parameters:
		/// - `asset_in`: ID of asset sold to the pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `amount`: Amount of asset sold
		/// - `min_buy_amount`: Minimum amount required to receive
		///
		/// Emits `SellExecuted` event when successful.
		///
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::sell()
			.saturating_add(T::OmnipoolHooks::on_trade_weight())
			.saturating_add(T::OmnipoolHooks::on_liquidity_changed_weight())
		)]
		#[transactional]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount: Balance,
			min_buy_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(asset_in != asset_out, Error::<T>::SameAssetTradeNotAllowed);

			ensure!(
				amount >= T::MinimumTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			ensure!(
				T::Currency::ensure_can_withdraw(asset_in, &who, amount).is_ok(),
				Error::<T>::InsufficientBalance
			);

			// Special handling when one of the asset is Hub Asset
			// Math is simplified and asset_in is actually part of asset_out state in this case
			if asset_in == T::HubAssetId::get() {
				return Self::sell_hub_asset(origin, &who, asset_out, amount, min_buy_amount);
			}

			if asset_out == T::HubAssetId::get() {
				return Self::sell_asset_for_hub_asset(&who, asset_in, amount, min_buy_amount);
			}

			let asset_in_state = Self::load_asset_state(asset_in)?;
			let asset_out_state = Self::load_asset_state(asset_out)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			ensure!(
				amount
					<= asset_in_state
						.reserve
						.checked_div(T::MaxInRatio::get())
						.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
				Error::<T>::MaxInRatioExceeded
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let (asset_fee, _) = T::Fee::get(&asset_out);
			let (_, protocol_fee) = T::Fee::get(&asset_in);

			let state_changes = hydra_dx_math::omnipool::calculate_sell_state_changes(
				&(&asset_in_state).into(),
				&(&asset_out_state).into(),
				amount,
				asset_fee,
				protocol_fee,
				current_imbalance.value,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				*state_changes.asset_out.delta_reserve > Balance::zero(),
				Error::<T>::ZeroAmountOut
			);

			ensure!(
				*state_changes.asset_out.delta_reserve >= min_buy_amount,
				Error::<T>::BuyLimitNotReached
			);

			ensure!(
				*state_changes.asset_out.delta_reserve
					<= asset_out_state
						.reserve
						.checked_div(T::MaxOutRatio::get())
						.ok_or(ArithmeticError::DivisionByZero)?, // Note: let's be safe. this can only fail if MaxOutRatio is zero.
				Error::<T>::MaxOutRatioExceeded
			);

			let new_asset_in_state = asset_in_state
				.clone()
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			let new_asset_out_state = asset_out_state
				.clone()
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			debug_assert_eq!(
				*state_changes.asset_in.delta_reserve, amount,
				"delta_reserve_in is not equal to given amount in"
			);

			T::Currency::transfer(
				asset_in,
				&who,
				&Self::protocol_account(),
				*state_changes.asset_in.delta_reserve,
			)?;
			T::Currency::transfer(
				asset_out,
				&Self::protocol_account(),
				&who,
				*state_changes.asset_out.delta_reserve,
			)?;

			// Hub liquidity update - work out difference between in and amount so only one update is needed.
			let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount))
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(ArithmeticError::Overflow)?;

			match delta_hub_asset {
				BalanceUpdate::Increase(val) if val == Balance::zero() => {
					// nothing to do if zero.
				}
				BalanceUpdate::Increase(_) => {
					// trade can only burn some. This would be a bug.
					return Err(Error::<T>::HubAssetUpdateError.into());
				}
				BalanceUpdate::Decrease(amount) => {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
			};

			// Callback hook info
			let info_in: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				asset_in,
				&asset_in_state,
				&new_asset_in_state,
				&state_changes.asset_in,
				false,
			);

			let info_out: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				asset_out,
				&asset_out_state,
				&new_asset_out_state,
				&state_changes.asset_out,
				false,
			);

			Self::update_imbalance(state_changes.delta_imbalance)?;

			Self::set_asset_state(asset_in, new_asset_in_state);
			Self::set_asset_state(asset_out, new_asset_out_state);

			T::OmnipoolHooks::on_trade(origin.clone(), info_in, info_out)?;

			Self::update_hdx_subpool_hub_asset(origin, state_changes.hdx_hub_amount)?;

			Self::process_trade_fee(&who, asset_out, state_changes.fee.asset_fee)?;

			debug_assert!(*state_changes.asset_in.delta_hub_reserve >= *state_changes.asset_out.delta_hub_reserve);
			debug_assert_eq!(
				*state_changes.asset_in.delta_hub_reserve - *state_changes.asset_out.delta_hub_reserve,
				state_changes.fee.protocol_fee
			);

			Self::deposit_event(Event::SellExecuted {
				who,
				asset_in,
				asset_out,
				amount_in: amount,
				amount_out: *state_changes.asset_out.delta_reserve,
				hub_amount_in: *state_changes.asset_in.delta_hub_reserve,
				hub_amount_out: *state_changes.asset_out.delta_hub_reserve,
				asset_fee_amount: state_changes.fee.asset_fee,
				protocol_fee_amount: state_changes.fee.protocol_fee,
			});

			Ok(())
		}

		/// Execute a swap of `asset_out` for `asset_in`.
		///
		/// Price is determined by the Omnipool.
		///
		/// Hub asset is traded separately.
		///
		/// Asset's tradable states must contain SELL flag for asset_in and BUY flag for asset_out, otherwise `NotAllowed` error is returned.
		///
		/// Parameters:
		/// - `asset_in`: ID of asset sold to the pool
		/// - `asset_out`: ID of asset bought from the pool
		/// - `amount`: Amount of asset sold
		/// - `max_sell_amount`: Maximum amount to be sold.
		///
		/// Emits `BuyExecuted` event when successful.
		///
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::buy()
			.saturating_add(T::OmnipoolHooks::on_trade_weight())
			.saturating_add(T::OmnipoolHooks::on_liquidity_changed_weight())
		)]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: T::AssetId,
			asset_in: T::AssetId,
			amount: Balance,
			max_sell_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(asset_in != asset_out, Error::<T>::SameAssetTradeNotAllowed);

			ensure!(
				amount >= T::MinimumTradingLimit::get(),
				Error::<T>::InsufficientTradingAmount
			);

			// Special handling when one of the asset is Hub Asset
			if asset_out == T::HubAssetId::get() {
				return Self::buy_hub_asset(&who, asset_in, amount, max_sell_amount);
			}

			if asset_in == T::HubAssetId::get() {
				return Self::buy_asset_for_hub_asset(origin, &who, asset_out, amount, max_sell_amount);
			}

			let asset_in_state = Self::load_asset_state(asset_in)?;
			let asset_out_state = Self::load_asset_state(asset_out)?;

			ensure!(
				Self::allow_assets(&asset_in_state, &asset_out_state),
				Error::<T>::NotAllowed
			);

			ensure!(asset_out_state.reserve >= amount, Error::<T>::InsufficientLiquidity);

			ensure!(
				amount
					<= asset_out_state
						.reserve
						.checked_div(T::MaxOutRatio::get())
						.ok_or(ArithmeticError::DivisionByZero)?, // Note: Let's be safe. this can only fail if MaxOutRatio is zero.
				Error::<T>::MaxOutRatioExceeded
			);

			let current_imbalance = <HubAssetImbalance<T>>::get();

			let (asset_fee, _) = T::Fee::get(&asset_out);
			let (_, protocol_fee) = T::Fee::get(&asset_in);
			let state_changes = hydra_dx_math::omnipool::calculate_buy_state_changes(
				&(&asset_in_state).into(),
				&(&asset_out_state).into(),
				amount,
				asset_fee,
				protocol_fee,
				current_imbalance.value,
			)
			.ok_or(ArithmeticError::Overflow)?;

			ensure!(
				T::Currency::ensure_can_withdraw(asset_in, &who, *state_changes.asset_in.delta_reserve).is_ok(),
				Error::<T>::InsufficientBalance
			);

			ensure!(
				*state_changes.asset_in.delta_reserve <= max_sell_amount,
				Error::<T>::SellLimitExceeded
			);

			ensure!(
				*state_changes.asset_in.delta_reserve
					<= asset_in_state
						.reserve
						.checked_div(T::MaxInRatio::get())
						.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
				Error::<T>::MaxInRatioExceeded
			);

			let new_asset_in_state = asset_in_state
				.clone()
				.delta_update(&state_changes.asset_in)
				.ok_or(ArithmeticError::Overflow)?;
			let new_asset_out_state = asset_out_state
				.clone()
				.delta_update(&state_changes.asset_out)
				.ok_or(ArithmeticError::Overflow)?;

			debug_assert_eq!(
				*state_changes.asset_out.delta_reserve, amount,
				"delta_reserve_out is not equal to given amount out"
			);

			T::Currency::transfer(
				asset_in,
				&who,
				&Self::protocol_account(),
				*state_changes.asset_in.delta_reserve,
			)?;
			T::Currency::transfer(
				asset_out,
				&Self::protocol_account(),
				&who,
				*state_changes.asset_out.delta_reserve,
			)?;

			// Hub liquidity update - work out difference between in and amount so only one update is needed.
			let delta_hub_asset = state_changes
				.asset_in
				.delta_hub_reserve
				.merge(
					state_changes
						.asset_out
						.delta_hub_reserve
						.merge(BalanceUpdate::Increase(state_changes.hdx_hub_amount))
						.ok_or(ArithmeticError::Overflow)?,
				)
				.ok_or(ArithmeticError::Overflow)?;

			match delta_hub_asset {
				BalanceUpdate::Increase(val) if val == Balance::zero() => {
					// nothing to do if zero.
				}
				BalanceUpdate::Increase(_) => {
					// trade can only burn some. This would be a bug.
					return Err(Error::<T>::HubAssetUpdateError.into());
				}
				BalanceUpdate::Decrease(amount) => {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				}
			};

			// Callback hook info
			let info_in: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				asset_in,
				&asset_in_state,
				&new_asset_in_state,
				&state_changes.asset_in,
				false,
			);

			let info_out: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				asset_out,
				&asset_out_state,
				&new_asset_out_state,
				&state_changes.asset_out,
				false,
			);

			Self::update_imbalance(state_changes.delta_imbalance)?;

			Self::set_asset_state(asset_in, new_asset_in_state);
			Self::set_asset_state(asset_out, new_asset_out_state);

			T::OmnipoolHooks::on_trade(origin.clone(), info_in, info_out)?;

			Self::update_hdx_subpool_hub_asset(origin, state_changes.hdx_hub_amount)?;

			Self::process_trade_fee(&who, asset_out, state_changes.fee.asset_fee)?;

			debug_assert!(*state_changes.asset_in.delta_hub_reserve >= *state_changes.asset_out.delta_hub_reserve);
			debug_assert_eq!(
				*state_changes.asset_in.delta_hub_reserve - *state_changes.asset_out.delta_hub_reserve,
				state_changes.fee.protocol_fee
			);

			Self::deposit_event(Event::BuyExecuted {
				who,
				asset_in,
				asset_out,
				amount_in: *state_changes.asset_in.delta_reserve,
				amount_out: *state_changes.asset_out.delta_reserve,
				hub_amount_in: *state_changes.asset_in.delta_hub_reserve,
				hub_amount_out: *state_changes.asset_out.delta_hub_reserve,
				asset_fee_amount: state_changes.fee.asset_fee,
				protocol_fee_amount: state_changes.fee.protocol_fee,
			});

			Ok(())
		}

		/// Update asset's tradable state.
		///
		/// Parameters:
		/// - `asset_id`: asset id
		/// - `state`: new state
		///
		/// Emits `TradableStateUpdated` event when successful.
		///
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_tradable_state())]
		#[transactional]
		pub fn set_asset_tradable_state(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			state: Tradability,
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			if asset_id == T::HubAssetId::get() {
				// Atm omnipool does not allow adding/removing liquidity of hub asset.
				// Although BUY is not supported yet, we can allow the new state to be set to SELL/BUY.
				ensure!(
					!state.contains(Tradability::ADD_LIQUIDITY) && !state.contains(Tradability::REMOVE_LIQUIDITY),
					Error::<T>::InvalidHubAssetTradableState
				);

				HubAssetTradability::<T>::mutate(|value| -> DispatchResult {
					*value = state;
					Self::deposit_event(Event::TradableStateUpdated { asset_id, state });
					Ok(())
				})
			} else {
				Assets::<T>::try_mutate(asset_id, |maybe_asset| -> DispatchResult {
					let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

					asset_state.tradable = state;
					Self::deposit_event(Event::TradableStateUpdated { asset_id, state });

					Ok(())
				})
			}
		}

		/// Refund given amount of asset to a recipient.
		///
		/// A refund is needed when a token is refused to be added to Omnipool, and initial liquidity of the asset has been already transferred to pool's account.
		///
		/// Transfer can be executed only if asset is not in Omnipool and pool's balance has sufficient amount.
		///
		/// Only `AuthorityOrigin` can perform this operation.
		///
		/// Emits `AssetRefunded`
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::refund_refused_asset())]
		#[transactional]
		pub fn refund_refused_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			amount: Balance,
			recipient: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			// Hub asset cannot be refunded
			ensure!(asset_id != T::HubAssetId::get(), Error::<T>::AssetRefundNotAllowed);

			// Make sure that asset is not in the pool
			ensure!(!Assets::<T>::contains_key(asset_id), Error::<T>::AssetAlreadyAdded);

			ensure!(
				T::Currency::ensure_can_withdraw(asset_id, &Self::protocol_account(), amount).is_ok(),
				Error::<T>::InsufficientBalance
			);

			T::Currency::transfer(asset_id, &Self::protocol_account(), &recipient, amount)?;

			Self::deposit_event(Event::AssetRefunded {
				asset_id,
				amount,
				recipient,
			});

			Ok(())
		}

		/// Update asset's weight cap
		///
		/// Parameters:
		/// - `asset_id`: asset id
		/// - `cap`: new weight cap
		///
		/// Emits `AssetWeightCapUpdated` event when successful.
		///
		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::set_asset_weight_cap())]
		#[transactional]
		pub fn set_asset_weight_cap(origin: OriginFor<T>, asset_id: T::AssetId, cap: Permill) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			Assets::<T>::try_mutate(asset_id, |maybe_asset| -> DispatchResult {
				let asset_state = maybe_asset.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				asset_state.cap = FixedU128::from(cap).into_inner();
				Self::deposit_event(Event::AssetWeightCapUpdated { asset_id, cap });

				Ok(())
			})
		}

		/// Removes protocol liquidity.
		///
		/// Protocol liquidity is liquidity from sacrificed positions. In order to remove protocol liquidity,
		/// we need the know the price of the position at the time of sacrifice. Hence this specific call.
		///
		/// Only `AuthorityOrigin` can perform this call.
		///
		/// Note that sacrifice position will be deprecated in future. There is no longer a need for that.
		///
		/// It works the same way as remove liquidity call, but position is temporary reconstructed.
		///
		#[pallet::call_index(11)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_protocol_liquidity())]
		#[transactional]
		pub fn withdraw_protocol_liquidity(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			amount: Balance,
			price: (Balance, Balance),
			dest: T::AccountId,
		) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin.clone())?;

			let asset_state = Self::load_asset_state(asset_id)?;
			ensure!(amount <= asset_state.protocol_shares, Error::<T>::InsufficientShares);

			let current_imbalance = <HubAssetImbalance<T>>::get();
			let current_hub_asset_liquidity =
				T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account());

			// dev note: as we no longer have the position details for sacrificed one, we just need to
			// construct temporary position.
			// Note that amount is ok to set to zero in this case. Although the remove liquidity calculation
			// calculates the delta for this field, it does not make any difference afterwards.
			let position = hydra_dx_math::omnipool::types::Position::<Balance> {
				amount: 0,
				price,
				shares: amount,
			};

			let state_changes = hydra_dx_math::omnipool::calculate_remove_liquidity_state_changes(
				&(&asset_state).into(),
				amount,
				&position,
				I129 {
					value: current_imbalance.value,
					negative: current_imbalance.negative,
				},
				current_hub_asset_liquidity,
				FixedU128::zero(),
			)
			.ok_or(ArithmeticError::Overflow)?;

			let mut new_asset_state = asset_state
				.clone()
				.delta_update(&state_changes.asset)
				.ok_or(ArithmeticError::Overflow)?;

			new_asset_state.protocol_shares = new_asset_state.protocol_shares.saturating_sub(amount);

			T::Currency::transfer(
				asset_id,
				&Self::protocol_account(),
				&dest,
				*state_changes.asset.delta_reserve,
			)?;

			Self::update_imbalance(state_changes.delta_imbalance)?;

			// burn only difference between delta hub and lp hub amount.
			Self::update_hub_asset_liquidity(
				&state_changes
					.asset
					.delta_hub_reserve
					.merge(BalanceUpdate::Increase(state_changes.lp_hub_amount))
					.ok_or(ArithmeticError::Overflow)?,
			)?;

			// LP receives some hub asset
			Self::process_hub_amount(state_changes.lp_hub_amount, &dest)?;

			// Callback hook info
			let info: AssetInfo<T::AssetId, Balance> =
				AssetInfo::new(asset_id, &asset_state, &new_asset_state, &state_changes.asset, true);

			Self::set_asset_state(asset_id, new_asset_state);

			Self::deposit_event(Event::ProtocolLiquidityRemoved {
				who: dest,
				asset_id,
				amount: *state_changes.asset.delta_reserve,
				hub_amount: state_changes.lp_hub_amount,
				shares_removed: amount,
			});

			T::OmnipoolHooks::on_liquidity_changed(origin, info)?;
			Ok(())
		}

		/// Removes token from Omnipool.
		///
		/// Asset's tradability must be FROZEN, otherwise `AssetNotFrozen` error is returned.
		///
		/// Remaining shares must belong to protocol, otherwise `SharesRemaining` error is returned.
		///
		/// Protocol's liquidity is transferred to the beneficiary account and hub asset amount is burned.
		///
		/// Only `AuthorityOrigin` can perform this call.
		///
		/// Emits `TokenRemoved` event when successful.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_token())]
		#[transactional]
		pub fn remove_token(origin: OriginFor<T>, asset_id: T::AssetId, beneficiary: T::AccountId) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			let asset_state = Self::load_asset_state(asset_id)?;

			// Allow only if no shares owned by LPs and asset is frozen.
			ensure!(asset_state.tradable == Tradability::FROZEN, Error::<T>::AssetNotFrozen);
			ensure!(
				asset_state.shares == asset_state.protocol_shares,
				Error::<T>::SharesRemaining
			);
			// Imbalance update
			let imbalance = <HubAssetImbalance<T>>::get();
			let hub_asset_liquidity = Self::get_hub_asset_balance_of_protocol_account();
			let delta_imbalance = hydra_dx_math::omnipool::calculate_delta_imbalance(
				asset_state.hub_reserve,
				I129 {
					value: imbalance.value,
					negative: imbalance.negative,
				},
				hub_asset_liquidity,
			)
			.ok_or(ArithmeticError::Overflow)?;
			Self::update_imbalance(BalanceUpdate::Increase(delta_imbalance))?;

			T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), asset_state.hub_reserve)?;
			T::Currency::transfer(asset_id, &Self::protocol_account(), &beneficiary, asset_state.reserve)?;
			<Assets<T>>::remove(asset_id);
			Self::deposit_event(Event::TokenRemoved {
				asset_id,
				amount: asset_state.reserve,
				hub_withdrawn: asset_state.hub_reserve,
			});
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			assert_ne!(
				T::MinimumPoolLiquidity::get(),
				Balance::zero(),
				"Minimum pool liquidity is 0."
			);
			assert_ne!(
				T::MinimumTradingLimit::get(),
				Balance::zero(),
				"Minimum trading limit is 0."
			);
			assert_ne!(T::MaxInRatio::get(), Balance::zero(), "MaxInRatio is 0.");
			assert_ne!(T::MaxOutRatio::get(), Balance::zero(), "MaxOutRatio is 0.");
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Protocol account address
	pub fn protocol_account() -> T::AccountId {
		PalletId(*b"omnipool").into_account_truncating()
	}

	/// Retrieve state of asset from the pool and its pool balance
	pub fn load_asset_state(asset_id: T::AssetId) -> Result<AssetReserveState<Balance>, DispatchError> {
		let state = <Assets<T>>::get(asset_id).ok_or(Error::<T>::AssetNotFound)?;
		let reserve = T::Currency::free_balance(asset_id, &Self::protocol_account());
		Ok((state, reserve).into())
	}

	/// Set new state of asset.
	/// This converts the new state into correct state type ( by removing the reserve)
	fn set_asset_state(asset_id: T::AssetId, new_state: AssetReserveState<Balance>) {
		<Assets<T>>::insert(asset_id, Into::<AssetState<Balance>>::into(new_state));
	}

	/// Generate an nft instance id and mint NFT into the class and instance.
	#[require_transactional]
	fn create_and_mint_position_instance(owner: &T::AccountId) -> Result<T::PositionItemId, DispatchError> {
		<NextPositionId<T>>::try_mutate(|current_value| -> Result<T::PositionItemId, DispatchError> {
			let next_position_id = *current_value;

			T::NFTHandler::mint_into(&T::NFTCollectionId::get(), &next_position_id, owner)?;

			*current_value = current_value
				.checked_add(&T::PositionItemId::one())
				.ok_or(ArithmeticError::Overflow)?;

			Ok(next_position_id)
		})
	}

	/// Update Hub asset side of HDX subpool and add given amount to hub_asset_reserve
	fn update_hdx_subpool_hub_asset(origin: T::RuntimeOrigin, hub_asset_amount: Balance) -> DispatchResult {
		if hub_asset_amount > Balance::zero() {
			let hdx_state = Self::load_asset_state(T::HdxAssetId::get())?;

			let mut native_subpool = Assets::<T>::get(T::HdxAssetId::get()).ok_or(Error::<T>::AssetNotFound)?;
			native_subpool.hub_reserve = native_subpool
				.hub_reserve
				.checked_add(hub_asset_amount)
				.ok_or(ArithmeticError::Overflow)?;
			<Assets<T>>::insert(T::HdxAssetId::get(), native_subpool);

			let updated_hdx_state = Self::load_asset_state(T::HdxAssetId::get())?;

			let delta_changes = AssetStateChange {
				delta_hub_reserve: BalanceUpdate::Increase(hub_asset_amount),
				..Default::default()
			};

			let info: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
				T::HdxAssetId::get(),
				&hdx_state,
				&updated_hdx_state,
				&delta_changes,
				false,
			);

			T::OmnipoolHooks::on_liquidity_changed(origin, info)?;
		}
		Ok(())
	}

	/// Mint or burn hub asset amount
	#[require_transactional]
	fn update_hub_asset_liquidity(delta_amount: &BalanceUpdate<Balance>) -> DispatchResult {
		match delta_amount {
			BalanceUpdate::Increase(amount) => {
				T::Currency::deposit(T::HubAssetId::get(), &Self::protocol_account(), *amount)
			}
			BalanceUpdate::Decrease(amount) => {
				T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), *amount)
			}
		}
	}

	/// Update imbalance with given delta_imbalance - increase or decrease.
	/// It cannot result in imbalance being > 0.
	fn update_imbalance(delta_imbalance: BalanceUpdate<Balance>) -> DispatchResult {
		<HubAssetImbalance<T>>::try_mutate(|current_imbalance| -> DispatchResult {
			*current_imbalance = match delta_imbalance {
				BalanceUpdate::Decrease(amount) => (*current_imbalance).sub(amount).ok_or(ArithmeticError::Overflow)?,
				BalanceUpdate::Increase(amount) => (*current_imbalance).add(amount).ok_or(ArithmeticError::Overflow)?,
			};

			ensure!(current_imbalance.negative, Error::<T>::PositiveImbalance);

			Ok(())
		})
	}

	/// Check if assets can be traded - asset_in must be allowed to be sold and asset_out allowed to be bought.
	fn allow_assets(asset_in: &AssetReserveState<Balance>, asset_out: &AssetReserveState<Balance>) -> bool {
		asset_in.tradable.contains(Tradability::SELL) && asset_out.tradable.contains(Tradability::BUY)
	}

	/// Swap hub asset for asset_out.
	/// Special handling of sell trade where asset in is Hub Asset.
	fn sell_hub_asset(
		origin: T::RuntimeOrigin,
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: Balance,
		limit: Balance,
	) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::SELL),
			Error::<T>::NotAllowed
		);

		let asset_state = Self::load_asset_state(asset_out)?;

		ensure!(asset_state.tradable.contains(Tradability::BUY), Error::<T>::NotAllowed);
		ensure!(
			amount
				<= asset_state
					.hub_reserve
					.checked_div(T::MaxInRatio::get())
					.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
			Error::<T>::MaxInRatioExceeded
		);

		let current_imbalance = <HubAssetImbalance<T>>::get();

		let current_hub_asset_liquidity = Self::get_hub_asset_balance_of_protocol_account();

		let (asset_fee, _) = T::Fee::get(&asset_out);

		let state_changes = hydra_dx_math::omnipool::calculate_sell_hub_state_changes(
			&(&asset_state).into(),
			amount,
			asset_fee,
			I129 {
				value: current_imbalance.value,
				negative: current_imbalance.negative,
			},
			current_hub_asset_liquidity,
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(
			*state_changes.asset.delta_reserve >= limit,
			Error::<T>::BuyLimitNotReached
		);

		ensure!(
			*state_changes.asset.delta_reserve
				<= asset_state
					.reserve
					.checked_div(T::MaxOutRatio::get())
					.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
			Error::<T>::MaxOutRatioExceeded
		);

		let new_asset_out_state = asset_state
			.clone()
			.delta_update(&state_changes.asset)
			.ok_or(ArithmeticError::Overflow)?;

		// Token updates
		T::Currency::transfer(
			T::HubAssetId::get(),
			who,
			&Self::protocol_account(),
			*state_changes.asset.delta_hub_reserve,
		)?;
		T::Currency::transfer(
			asset_out,
			&Self::protocol_account(),
			who,
			*state_changes.asset.delta_reserve,
		)?;

		let info: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
			asset_out,
			&asset_state,
			&new_asset_out_state,
			&state_changes.asset,
			false,
		);

		Self::update_imbalance(state_changes.delta_imbalance)?;

		Self::set_asset_state(asset_out, new_asset_out_state);

		Self::process_trade_fee(who, asset_out, state_changes.fee.asset_fee)?;

		Self::deposit_event(Event::SellExecuted {
			who: who.clone(),
			asset_in: T::HubAssetId::get(),
			asset_out,
			amount_in: *state_changes.asset.delta_hub_reserve,
			amount_out: *state_changes.asset.delta_reserve,
			hub_amount_in: 0,
			hub_amount_out: 0,
			asset_fee_amount: state_changes.fee.asset_fee,
			protocol_fee_amount: state_changes.fee.protocol_fee,
		});

		T::OmnipoolHooks::on_hub_asset_trade(origin, info)?;

		Ok(())
	}

	/// Swap asset for Hub Asset
	/// Special handling of buy trade where asset in is Hub Asset.
	fn buy_asset_for_hub_asset(
		origin: T::RuntimeOrigin,
		who: &T::AccountId,
		asset_out: T::AssetId,
		amount: Balance,
		limit: Balance,
	) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::SELL),
			Error::<T>::NotAllowed
		);

		let asset_state = Self::load_asset_state(asset_out)?;

		ensure!(asset_state.tradable.contains(Tradability::BUY), Error::<T>::NotAllowed);

		ensure!(
			amount
				<= asset_state
					.reserve
					.checked_div(T::MaxOutRatio::get())
					.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
			Error::<T>::MaxOutRatioExceeded
		);

		let current_imbalance = <HubAssetImbalance<T>>::get();

		let current_hub_asset_liquidity = Self::get_hub_asset_balance_of_protocol_account();

		let (asset_fee, _) = T::Fee::get(&asset_out);

		let state_changes = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
			&(&asset_state).into(),
			amount,
			asset_fee,
			I129 {
				value: current_imbalance.value,
				negative: current_imbalance.negative,
			},
			current_hub_asset_liquidity,
		)
		.ok_or(ArithmeticError::Overflow)?;

		ensure!(
			*state_changes.asset.delta_hub_reserve <= limit,
			Error::<T>::SellLimitExceeded
		);

		ensure!(
			*state_changes.asset.delta_hub_reserve
				<= asset_state
					.hub_reserve
					.checked_div(T::MaxInRatio::get())
					.ok_or(ArithmeticError::DivisionByZero)?, // Note: this can only fail if MaxInRatio is zero.
			Error::<T>::MaxInRatioExceeded
		);

		let new_asset_out_state = asset_state
			.clone()
			.delta_update(&state_changes.asset)
			.ok_or(ArithmeticError::Overflow)?;

		T::Currency::transfer(
			T::HubAssetId::get(),
			who,
			&Self::protocol_account(),
			*state_changes.asset.delta_hub_reserve,
		)?;
		T::Currency::transfer(
			asset_out,
			&Self::protocol_account(),
			who,
			*state_changes.asset.delta_reserve,
		)?;

		let info: AssetInfo<T::AssetId, Balance> = AssetInfo::new(
			asset_out,
			&asset_state,
			&new_asset_out_state,
			&state_changes.asset,
			false,
		);

		Self::update_imbalance(state_changes.delta_imbalance)?;

		Self::set_asset_state(asset_out, new_asset_out_state);

		Self::process_trade_fee(who, asset_out, state_changes.fee.asset_fee)?;

		Self::deposit_event(Event::BuyExecuted {
			who: who.clone(),
			asset_in: T::HubAssetId::get(),
			asset_out,
			amount_in: *state_changes.asset.delta_hub_reserve,
			amount_out: *state_changes.asset.delta_reserve,
			hub_amount_in: 0,
			hub_amount_out: 0,
			asset_fee_amount: state_changes.fee.asset_fee,
			protocol_fee_amount: state_changes.fee.protocol_fee,
		});

		T::OmnipoolHooks::on_hub_asset_trade(origin, info)?;

		Ok(())
	}

	/// Buy hub asset from the pool
	/// Special handling of buy trade where asset out is Hub Asset.
	fn buy_hub_asset(_who: &T::AccountId, _asset_in: T::AssetId, _amount: Balance, _limit: Balance) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		// Note: Currently not allowed at all, neither math is done for this case
		// this is already ready when hub asset will be allowed to be bought from the pool

		Err(Error::<T>::NotAllowed.into())
	}

	/// Swap asset for Hub Asset
	/// Special handling of sell trade where asset out is Hub Asset.
	fn sell_asset_for_hub_asset(
		_who: &T::AccountId,
		_asset_in: T::AssetId,
		_amount: Balance,
		_limit: Balance,
	) -> DispatchResult {
		ensure!(
			HubAssetTradability::<T>::get().contains(Tradability::BUY),
			Error::<T>::NotAllowed
		);

		// Note: Currently not allowed at all, neither math is done for this case
		// this is already ready when hub asset will be allowed to be bought from the pool

		Err(Error::<T>::NotAllowed.into())
	}

	/// Get hub asset balance of protocol account
	fn get_hub_asset_balance_of_protocol_account() -> Balance {
		T::Currency::free_balance(T::HubAssetId::get(), &Self::protocol_account())
	}

	/// Remove asset from list of Omnipool assets.
	/// No events emitted.
	pub fn remove_asset(asset_id: T::AssetId) -> DispatchResult {
		<Assets<T>>::remove(asset_id);
		Ok(())
	}

	/// Insert or update position with given position data.
	pub fn set_position(position_id: T::PositionItemId, position: &Position<Balance, T::AssetId>) -> DispatchResult {
		<Positions<T>>::insert(position_id, position);
		Ok(())
	}

	/// Add new asset to list of Omnipool assets.
	/// No events emitted.
	pub fn add_asset(asset_id: T::AssetId, state: AssetState<Balance>) -> DispatchResult {
		ensure!(!Assets::<T>::contains_key(asset_id), Error::<T>::AssetAlreadyAdded);
		ensure!(T::AssetRegistry::exists(asset_id), Error::<T>::AssetNotRegistered);

		<Assets<T>>::insert(asset_id, state);

		Ok(())
	}

	/// Load state of an asset and update it with given delta changes.
	pub fn update_asset_state(asset_id: T::AssetId, delta: AssetStateChange<Balance>) -> DispatchResult {
		let state = Self::load_asset_state(asset_id)?;
		let updated_state = state.delta_update(&delta).ok_or(ArithmeticError::Overflow)?;
		Self::set_asset_state(asset_id, updated_state);

		Ok(())
	}

	/// Load position and check its owner
	/// Returns Forbidden if not position owner
	pub fn load_position(
		position_id: T::PositionItemId,
		owner: T::AccountId,
	) -> Result<Position<Balance, T::AssetId>, DispatchError> {
		ensure!(
			T::NFTHandler::owner(&T::NFTCollectionId::get(), &position_id) == Some(owner),
			Error::<T>::Forbidden
		);

		Positions::<T>::get(position_id).ok_or_else(|| Error::<T>::PositionNotFound.into())
	}

	pub fn is_hub_asset_allowed(operation: Tradability) -> bool {
		HubAssetTradability::<T>::get().contains(operation)
	}

	/// Returns `true` if `asset` exists in the omnipool or `false`
	pub fn exists(asset: T::AssetId) -> bool {
		Assets::<T>::contains_key(asset)
	}

	/// Calls `on_trade_fee` hook and ensures that no more than the fee amount is transferred.
	fn process_trade_fee(trader: &T::AccountId, asset: T::AssetId, amount: Balance) -> DispatchResult {
		let account = Self::protocol_account();
		let original_asset_reserve = T::Currency::free_balance(asset, &account);

		// Let's give little bit less to process. Subtracting one due to potential rounding errors
		let allowed_amount = amount.saturating_sub(Balance::one());
		let used = T::OmnipoolHooks::on_trade_fee(account.clone(), trader.clone(), asset, allowed_amount)?;
		let asset_reserve = T::Currency::free_balance(asset, &account);
		let diff = original_asset_reserve.saturating_sub(asset_reserve);
		ensure!(diff <= allowed_amount, Error::<T>::FeeOverdraft);
		ensure!(diff == used, Error::<T>::FeeOverdraft);
		Ok(())
	}

	pub fn process_hub_amount(amount: Balance, dest: &T::AccountId) -> DispatchResult {
		if amount > Balance::zero() {
			// If transfers fails and the amount is less than ED, it failed due to ED limit, so we simply burn it
			if let Err(e) = T::Currency::transfer(T::HubAssetId::get(), &Self::protocol_account(), dest, amount) {
				if amount < 400_000_000u128 {
					T::Currency::withdraw(T::HubAssetId::get(), &Self::protocol_account(), amount)?;
				} else {
					return Err(e);
				}
			}
		}
		Ok(())
	}
}
