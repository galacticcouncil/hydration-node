#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use crate::types::{Balance, CollateralInfo};
use evm::{ExitReason, ExitSucceed};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::traits::ExistenceRequirement;
use frame_support::traits::{fungibles::Mutate, ExistenceRequirement};
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_stableswap::types::{AssetReserve, PoolSnapshot};
use sp_core::Get;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::ArithmeticError;
use sp_runtime::DispatchError;
use sp_runtime::RuntimeDebug;

pub mod math;
pub mod traits;
pub mod types;

#[cfg(feature = "std")]
use sp_std::vec::Vec;

// Generate the ERC20 function selectors
#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum ERC20Function {
	Mint = "mint(address,uint256)",
	Burn = "burn(uint256)",
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::math::PegType;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use pallet_stableswap::types::AssetReserve;
	use sp_core::{H256, U256};
	use sp_runtime::{
		traits::{AccountIdConversion, CheckedSub, Convert, Zero},
		ArithmeticError, Perbill, Permill,
	};
	use sp_std::prelude::*;

	// EVM imports

	// Type for EVM call result
	type CallResult = (ExitReason, Vec<u8>);

	/// HSM account id identifier
	pub const HSM_IDENTIFIER: &[u8] = b"hsm/acct";

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_stableswap::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset ID of Hollar
		#[pallet::constant]
		type HollarId: Get<Self::AssetId>;

		/// Pallet ID to determine HSM account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// GHO contract address - EVM address of GHO token contract
		#[pallet::constant]
		type GhoContractAddress: Get<EvmAddress>;

		/// Currency - fungible tokens trait to access token transfers
		type Currency: Mutate<Self::AccountId, Balance = Balance, AssetId = Self::AssetId>;

		/// EVM handler
		type Evm: EVM<CallResult>;

		/// EVM address converter
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// The gas limit for the execution of EVM calls
		#[pallet::constant]
		type GasLimit: Get<u64>;

		/// Receiver account that receives all collateral balance of HSM account when asset is removed from the list
		#[pallet::constant]
		type Receiver: Get<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// List of approved assets that Hollar can be purchased with
	#[pallet::storage]
	#[pallet::getter(fn collaterals)]
	pub type Collaterals<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, CollateralInfo<T::AssetId>>;

	/// List of assets that HSM holds as collateral
	#[pallet::storage]
	#[pallet::getter(fn collateral_holdings)]
	pub type CollateralHoldings<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, Balance, ValueQuery>;

	/// Amount of Hollar bought with an asset in single block
	#[pallet::storage]
	#[pallet::getter(fn hollar_amount_received)]
	pub type HollarAmountReceived<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new collateral asset was added
		CollateralAdded {
			asset_id: T::AssetId,
			pool_id: T::AssetId,
			purchase_fee: Permill,
			max_buy_price_coefficient: Permill,
			buy_back_fee: Permill,
			b: Perbill,
		},
		/// A collateral asset was removed
		CollateralRemoved { asset_id: T::AssetId, amount: Balance },
		/// A collateral asset was updated
		CollateralUpdated {
			asset_id: T::AssetId,
			purchase_fee: Option<Permill>,
			max_buy_price_coefficient: Option<Permill>,
			buy_back_fee: Option<Permill>,
			b: Option<Perbill>,
		},
		/// Sell executed
		SellExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
		},
		/// Buy executed
		BuyExecuted {
			who: T::AccountId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset is not approved as collateral
		AssetNotApproved,
		/// Asset is already approved as collateral
		AssetAlreadyApproved,
		/// Another asset from the same pool is already approved
		PoolAlreadyHasCollateral,
		/// Invalid asset pair, must be Hollar and approved collateral
		InvalidAssetPair,
		/// Max buy price exceeded
		MaxBuyPriceExceeded,
		/// Max buy back amount in single block exceeded
		MaxBuyBackExceeded,
		/// Max holding amount for collateral exceeded
		MaxHoldingExceeded,
		/// Slippage limit exceeded
		SlippageLimitExceeded,
		/// Invalid EVM contract interaction
		InvalidEVMInteraction,
		/// Decimal retrieval failed
		DecimalRetrievalFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			// Clear the Hollar Amount Received storage on finalize
			<HollarAmountReceived<T>>::clear(u32::MAX, None);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new collateral asset
		///
		/// Parameters:
		/// - `asset_id`: Asset ID to be added as collateral
		/// - `pool_id`: Pool ID where this asset belongs
		/// - `purchase_fee`: Fee applied when buying Hollar with this asset
		/// - `max_buy_price_coefficient`: Maximum buy price coefficient for HSM to buy back Hollar
		/// - `buy_back_fee`: Fee applied when buying back Hollar
		/// - `b`: Parameter that controls how quickly HSM can buy Hollar with this asset
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn add_collateral_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			pool_id: T::AssetId,
			purchase_fee: Permill,
			max_buy_price_coefficient: Permill,
			buy_back_fee: Permill,
			b: Perbill,
			max_in_holding: Option<Balance>,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				!Collaterals::<T>::contains_key(asset_id),
				Error::<T>::AssetAlreadyApproved
			);

			// Check if there's already an asset from the same pool
			for (_, info) in Collaterals::<T>::iter() {
				ensure!(info.pool_id != pool_id, Error::<T>::PoolAlreadyHasCollateral);
			}

			let collateral_info = CollateralInfo {
				pool_id,
				purchase_fee,
				max_buy_price_coefficient,
				b,
				buy_back_fee,
				max_in_holding,
			};

			Collaterals::<T>::insert(asset_id, collateral_info);

			Self::deposit_event(Event::<T>::CollateralAdded {
				asset_id,
				pool_id,
				purchase_fee,
				max_buy_price_coefficient,
				buy_back_fee,
				b,
			});

			Ok(())
		}

		/// Remove a collateral asset
		///
		/// Parameters:
		/// - `asset_id`: Asset ID to remove from collaterals
		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn remove_collateral_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(Collaterals::<T>::contains_key(asset_id), Error::<T>::AssetNotApproved);

			// Get the current holding balance
			let amount = CollateralHoldings::<T>::get(asset_id);

			// Transfer all holdings to the receiver account
			if !amount.is_zero() {
				<T as Config>::Currency::transfer(
					asset_id,
					&Self::account_id(),
					&T::Receiver::get(),
					amount,
			Preservation::Expendable,
				)?;
			}

			// Remove from storages
			Collaterals::<T>::remove(asset_id);
			CollateralHoldings::<T>::remove(asset_id);

			Self::deposit_event(Event::<T>::CollateralRemoved { asset_id, amount });

			Ok(())
		}

		/// Update collateral asset parameters
		///
		/// Parameters:
		/// - `asset_id`: Asset ID to update
		/// - `purchase_fee`: New purchase fee (optional)
		/// - `max_buy_price_coefficient`: New max buy price coefficient (optional)
		/// - `buy_back_fee`: New buy back fee (optional)
		/// - `b`: New b parameter (optional)
		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn update_collateral_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			purchase_fee: Option<Permill>,
			max_buy_price_coefficient: Option<Permill>,
			buy_back_fee: Option<Permill>,
			b: Option<Perbill>,
			max_in_holding: Option<Option<Balance>>,
		) -> DispatchResult {
			ensure_root(origin)?;

			Collaterals::<T>::try_mutate(asset_id, |maybe_info| -> DispatchResult {
				let info = maybe_info.as_mut().ok_or(Error::<T>::AssetNotApproved)?;

				if let Some(fee) = purchase_fee {
					info.purchase_fee = fee;
				}

				if let Some(coefficient) = max_buy_price_coefficient {
					info.max_buy_price_coefficient = coefficient;
				}

				if let Some(fee) = buy_back_fee {
					info.buy_back_fee = fee;
				}

				if let Some(param_b) = b {
					info.b = param_b;
				}

				if let Some(holding) = max_in_holding {
					info.max_in_holding = holding;
				}

				Ok(())
			})?;

			Self::deposit_event(Event::<T>::CollateralUpdated {
				asset_id,
				purchase_fee,
				max_buy_price_coefficient,
				buy_back_fee,
				b,
			});

			Ok(())
		}

		/// Sell asset to HSM
		///
		/// Either selling Hollar back to HSM or selling collateral asset in exchange of Hollar
		///
		/// Parameters:
		/// - `asset_in`: Asset being sold
		/// - `asset_out`: Asset being received
		/// - `amount_in`: Amount of asset_in to sell
		/// - `slippage_limit`: Minimum amount out for slippage protection
		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			slippage_limit: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let hollar_id = T::HollarId::get();

			// Ensure it's a pair of Hollar and collateral asset
			ensure!(
				(asset_in == hollar_id && Self::is_collateral(asset_out))
					|| (asset_out == hollar_id && Self::is_collateral(asset_in)),
				Error::<T>::InvalidAssetPair
			);

			let amount_out = if asset_in == hollar_id {
				// Selling Hollar to get collateral
				Self::do_sell_hollar(&who, asset_out, amount_in)?
			} else {
				// Selling collateral to get Hollar
				Self::do_sell_collateral(&who, asset_in, amount_in)?
			};

			// Check slippage
			ensure!(amount_out >= slippage_limit, Error::<T>::SlippageLimitExceeded);

			Self::deposit_event(Event::<T>::SellExecuted {
				who: who.clone(),
				asset_in,
				asset_out,
				amount_in,
				amount_out,
			});

			Ok(())
		}

		/// Buy asset from HSM
		///
		/// Either buying Hollar from HSM or buying collateral asset with Hollar
		///
		/// Parameters:
		/// - `asset_in`: Asset being sold
		/// - `asset_out`: Asset being bought
		/// - `amount_out`: Amount of asset_out to buy
		/// - `slippage_limit`: Maximum amount in for slippage protection
		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn buy(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_out: Balance,
			slippage_limit: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let hollar_id = T::HollarId::get();

			// Ensure it's a pair of Hollar and collateral asset
			ensure!(
				(asset_in == hollar_id && Self::is_collateral(asset_out))
					|| (asset_out == hollar_id && Self::is_collateral(asset_in)),
				Error::<T>::InvalidAssetPair
			);

			let amount_in = if asset_out == hollar_id {
				// Buying Hollar with collateral
				Self::do_buy_hollar(&who, asset_in, amount_out)?
			} else {
				// Buying collateral with Hollar
				Self::do_buy_collateral(&who, asset_out, amount_out)?
			};

			// Check slippage
			ensure!(amount_in <= slippage_limit, Error::<T>::SlippageLimitExceeded);

			Self::deposit_event(Event::<T>::BuyExecuted {
				who: who.clone(),
				asset_in,
				asset_out,
				amount_in,
				amount_out,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Get the account ID of the HSM
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Check if an asset is an approved collateral
	pub fn is_collateral(asset_id: T::AssetId) -> bool {
		Collaterals::<T>::contains_key(asset_id)
	}

	/// Get pool data
	fn get_pool_data(
		pool_id: T::AssetId,
		collateral_asset: T::AssetId,
	) -> Result<(usize, usize, Vec<Balance>, Vec<u8>, Vec<(Balance, Balance)>), DispatchError> {
		let hollar_id = T::HollarId::get();

		// Try to get the pool snapshot first
		let Some(pool_snapshot) = pallet_stableswap::Pallet::<T>::initial_pool_snapshot(pool_id) else {
			return Err(pallet_stableswap::Error::<T>::PoolNotFound.into());
		};
		// Find Hollar and collateral asset positions in the pool
		let hollar_pos = pool_snapshot
			.assets
			.iter()
			.position(|&asset| asset == hollar_id)
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		let collateral_pos = pool_snapshot
			.assets
			.iter()
			.position(|&asset| asset == collateral_asset)
			.ok_or(pallet_stableswap::Error::<T>::AssetNotInPool)?;

		return Ok((
			hollar_pos,
			collateral_pos,
			pool_snapshot.reserves,
			pool_snapshot.decimals,
			pool_snapshot.pegs.into_inner(),
		));
	}

	/// Selling Hollar to get collateral asset
	fn do_sell_hollar(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		hollar_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		// Get pool data
		let (hollar_pos, collateral_pos, reserves, decimals, pegs) = Self::get_pool_data(pool_id, collateral_asset)?;

		// Get decimals
		let hollar_decimals = decimals[hollar_pos];
		let collateral_decimals = decimals[collateral_pos];

		// Scale Hollar amount to 18 decimals for calculation
		let hollar_amount_scaled = crate::math::scale_to_18_decimals(hollar_amount, hollar_decimals)?;

		// Get reserves and pegs
		let hollar_reserve = reserves[hollar_pos];
		let collateral_reserve = reserves[collateral_pos];
		let peg = pegs[collateral_pos];

		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;

		// 2. Calculate how much Hollar can HSM buy back in a single block
		let buyback_limit = crate::math::calculate_buyback_limit(imbalance, collateral_info.b);

		// Check if the requested amount exceeds the buyback limit
		ensure!(
			HollarAmountReceived::<T>::get(collateral_asset).saturating_add(hollar_amount_scaled) <= buyback_limit,
			Error::<T>::MaxBuyBackExceeded
		);

		// 3. Calculate execution price by simulating a swap
		let input_amount = pallet_stableswap::Pallet::<T>::calculate_in_given_out(
			pool_id,
			collateral_asset,
			T::HollarId::get(),
			hollar_amount,
			false, // don't persist peg
		)?
		.0;

		let execution_price = input_amount
			.checked_div(hollar_amount)
			.ok_or(ArithmeticError::DivisionByZero)?;

		// 4. Calculate final buy price with fee
		let buy_price = crate::math::calculate_buy_price_with_fee(execution_price, collateral_info.buy_back_fee)?;

		// 5. Calculate amount of collateral to receive
		let collateral_amount = crate::math::calculate_collateral_amount(hollar_amount_scaled, buy_price)?;

		// 6. Calculate max price
		let max_price = crate::math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);

		// Check if price exceeds max price
		ensure!(buy_price <= max_price, Error::<T>::MaxBuyPriceExceeded);

		// 7. Check max holding limit if configured
		if let Some(max_holding) = collateral_info.max_in_holding {
			let current_holding = CollateralHoldings::<T>::get(collateral_asset);
			ensure!(
				current_holding.saturating_add(collateral_amount) <= max_holding,
				Error::<T>::MaxHoldingExceeded
			);
		}

		// Scale collateral amount back to its original decimals
		let collateral_amount = crate::math::scale_from_18_decimals(collateral_amount, collateral_decimals)?;

		// Execute the swap
		// 1. Transfer Hollar from user to HSM
		<T as Config>::Currency::transfer(
			T::HollarId::get(),
			who,
			&Self::account_id(),
			hollar_amount,
					Preservation::Expendable,
		)?;

		// 2. Burn Hollar by calling GHO contract
		Self::burn_hollar(hollar_amount)?;

		// 3. Transfer collateral from HSM to user
		<T as Config>::Currency::transfer(
			collateral_asset,
			&Self::account_id(),
			who,
			collateral_amount,
			Preservation::Preserve,
		)?;

		// 4. Update HSM holdings
		CollateralHoldings::<T>::mutate(collateral_asset, |balance| {
			*balance = balance.saturating_sub(collateral_amount);
		});

		// 5. Update Hollar amount received in this block
		HollarAmountReceived::<T>::mutate(collateral_asset, |amount| {
			*amount = amount.saturating_add(hollar_amount_scaled);
		});

		Ok(collateral_amount)
	}

	/// Selling collateral asset to get Hollar
	fn do_sell_collateral(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		collateral_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		// Get pool data
		let (hollar_pos, collateral_pos, _, decimals, pegs) = Self::get_pool_data(pool_id, collateral_asset)?;

		// Get decimals
		let hollar_decimals = decimals[hollar_pos];
		let collateral_decimals = decimals[collateral_pos];

		// Scale collateral amount to 18 decimals for calculation
		let collateral_amount_scaled = crate::math::scale_to_18_decimals(collateral_amount, collateral_decimals)?;

		// Get the peg for this asset
		let peg = pegs[collateral_pos];

		// Calculate purchase price
		let purchase_price = crate::math::calculate_purchase_price(peg, collateral_info.purchase_fee);

		// Calculate Hollar amount to mint
		let hollar_amount = crate::math::calculate_hollar_amount(collateral_amount_scaled, purchase_price)?;

		// Scale Hollar amount back to its decimals
		let hollar_amount = crate::math::scale_from_18_decimals(hollar_amount, hollar_decimals)?;

		// Execute the swap
		// 1. Transfer collateral from user to HSM
		<T as Config>::Currency::transfer(
			collateral_asset,
			who,
			&Self::account_id(),
			collateral_amount,
			Preservation::Expendable,
		)?;

		// 2. Mint Hollar by calling GHO contract
		Self::mint_hollar(who, hollar_amount)?;

		// 3. Update HSM holdings
		CollateralHoldings::<T>::mutate(collateral_asset, |balance| {
			*balance = balance.saturating_add(collateral_amount);
		});

		Ok(hollar_amount)
	}

	/// Buying Hollar using collateral asset
	fn do_buy_hollar(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		hollar_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		// Get pool data
		let (hollar_pos, collateral_pos, reserves, decimals, pegs) = Self::get_pool_data(pool_id, collateral_asset)?;

		// Get decimals
		let hollar_decimals = decimals[hollar_pos];
		let collateral_decimals = decimals[collateral_pos];

		// Scale Hollar amount to 18 decimals for calculation
		let hollar_amount_scaled = crate::math::scale_to_18_decimals(hollar_amount, hollar_decimals)?;

		// Get reserves and pegs
		let hollar_reserve = reserves[hollar_pos];
		let collateral_reserve = reserves[collateral_pos];
		let peg = pegs[collateral_pos];

		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;

		// 2. Calculate how much Hollar can HSM mint in a single block
		let buyback_limit = crate::math::calculate_buyback_limit(imbalance, collateral_info.b);

		// Check if the requested amount exceeds the buyback limit
		ensure!(hollar_amount_scaled < buyback_limit, Error::<T>::MaxBuyBackExceeded);

		// 3. Calculate execution price by simulating a swap
		let input_amount = pallet_stableswap::Pallet::<T>::calculate_in_given_out(
			pool_id,
			collateral_asset,
			T::HollarId::get(),
			hollar_amount,
			false, // don't persist peg
		)?
		.0;

		let execution_price = input_amount
			.checked_div(hollar_amount)
			.ok_or(ArithmeticError::DivisionByZero)?;

		// 4. Calculate final purchase price with fee
		let purchase_price = crate::math::calculate_purchase_price(peg, collateral_info.purchase_fee);

		// 5. Calculate amount of collateral needed
		let collateral_amount = crate::math::calculate_collateral_amount(hollar_amount_scaled, purchase_price)?;

		// Scale collateral amount back to its original decimals
		let collateral_amount = crate::math::scale_from_18_decimals(collateral_amount, collateral_decimals)?;

		// Check user has enough collateral
		ensure!(
			<T as Config>::Currency::free_balance(collateral_asset, who) >= collateral_amount,
			pallet_stableswap::Error::<T>::InsufficientBalance
		);

		// Execute the swap
		// 1. Transfer collateral from user to HSM
		<T as Config>::Currency::transfer(
			collateral_asset,
			who,
			&Self::account_id(),
			collateral_amount,
			Preservation::Expendable,
		)?;

		// 2. Mint Hollar by calling GHO contract
		Self::mint_hollar(who, hollar_amount)?;

		// 3. Update HSM holdings
		CollateralHoldings::<T>::mutate(collateral_asset, |balance| {
			*balance = balance.saturating_add(collateral_amount);
		});

		Ok(hollar_amount)
	}

	/// Buying collateral asset using Hollar
	fn do_buy_collateral(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		collateral_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		// Get pool data
		let (hollar_pos, collateral_pos, reserves, decimals, pegs) = Self::get_pool_data(pool_id, collateral_asset)?;

		// Get decimals
		let hollar_decimals = decimals[hollar_pos];
		let collateral_decimals = decimals[collateral_pos];

		// Scale collateral amount to 18 decimals for calculation
		let collateral_amount_scaled = crate::math::scale_to_18_decimals(collateral_amount, collateral_decimals)?;

		// Get reserves and pegs
		let hollar_reserve = reserves[hollar_pos];
		let collateral_reserve = reserves[collateral_pos];
		let peg = pegs[collateral_pos];

		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;

		// 2. Calculate how much Hollar can HSM buy back in a single block
		let buyback_limit = crate::math::calculate_buyback_limit(imbalance, collateral_info.b);

		// 3. Calculate execution price by simulating a swap
		let hollar_amount = pallet_stableswap::Pallet::<T>::calculate_out_given_in(
			pool_id,
			collateral_asset,
			T::HollarId::get(),
			collateral_amount,
			false, // Don't persist peg
		)?
		.0;

		let execution_price = hollar_amount
			.checked_div(collateral_amount)
			.ok_or(ArithmeticError::DivisionByZero)?;

		// 4. Calculate final buy price with fee
		let buy_price = crate::math::calculate_buy_price_with_fee(execution_price, collateral_info.buy_back_fee)?;

		// 5. Calculate amount of Hollar to pay
		let hollar_amount_to_pay = crate::math::calculate_hollar_amount(collateral_amount_scaled, buy_price)?;

		// Scale Hollar amount back to its decimals
		let hollar_amount_to_pay = crate::math::scale_from_18_decimals(hollar_amount_to_pay, hollar_decimals)?;

		// Check if the requested amount exceeds the buyback limit
		let hollar_amount_scaled = crate::math::scale_to_18_decimals(hollar_amount_to_pay, hollar_decimals)?;
		ensure!(buyback_limit > hollar_amount_scaled, Error::<T>::MaxBuyBackExceeded);

		// 6. Calculate max price
		let max_price = crate::math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);

		// Check if price exceeds max price
		ensure!(buy_price <= max_price, Error::<T>::MaxBuyPriceExceeded);

		// Check HSM has enough collateral
		let current_holding = CollateralHoldings::<T>::get(collateral_asset);
		ensure!(
			current_holding >= collateral_amount,
			pallet_stableswap::Error::<T>::InsufficientBalance
		);

		// Execute the swap
		// 1. Transfer collateral from user to HSM
		<T as Config>::Currency::transfer(
			collateral_asset,
			who,
			&Self::account_id(),
			collateral_amount,
			Preservation::Expendable,
		)?;

		// 2. Mint Hollar by calling GHO contract
		Self::mint_hollar(who, hollar_amount_to_pay)?;

		// 3. Update HSM holdings
		CollateralHoldings::<T>::mutate(collateral_asset, |balance| {
			*balance = balance.saturating_add(collateral_amount);
		});

		Ok(hollar_amount_to_pay)
	}

	/// Mint Hollar by calling the GHO token contract
	fn mint_hollar(who: &T::AccountId, amount: Balance) -> DispatchResult {
		let contract = T::GhoContractAddress::get();
		let pallet_address = T::EvmAccounts::evm_address(&Self::account_id());

		// Create the context for the EVM call
		let context = CallContext::new_call(contract, pallet_address);

		// Encode the mint function call with recipient and amount
		let recipient_evm = T::EvmAccounts::evm_address(who);
		let mut data = Into::<u32>::into(ERC20Function::Mint).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(recipient_evm).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());

		// Execute the EVM call
		let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());

		// Check if the call was successful
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: "hsm", "Mint Hollar EVM execution failed. Reason: {:?}", value);
			return Err(Error::<T>::InvalidEVMInteraction.into());
		}

		Ok(())
	}

	/// Burn Hollar by calling the GHO token contract
	fn burn_hollar(amount: Balance) -> DispatchResult {
		let contract = T::GhoContractAddress::get();
		let pallet_address = T::EvmAccounts::evm_address(&Self::account_id());

		// Create the context for the EVM call
		let context = CallContext::new_call(contract, pallet_address);

		// Encode the burn function call with amount
		let mut data = Into::<u32>::into(ERC20Function::Burn).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());

		// Execute the EVM call
		let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());

		// Check if the call was successful
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: "hsm", "Burn Hollar EVM execution failed. Reason: {:?}", value);
			return Err(Error::<T>::InvalidEVMInteraction.into());
		}

		Ok(())
	}
}
