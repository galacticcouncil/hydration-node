#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use crate::types::{Balance, CoefficientRatio, CollateralInfo};
use ethabi::ethereum_types::BigEndianHash;
use evm::{ExitReason, ExitSucceed};
use frame_support::dispatch::DispatchResult;
use frame_support::ensure;
use frame_support::pallet_prelude::IsType;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_system::offchain::SendTransactionTypes;
use frame_system::offchain::SendUnsignedTransaction;
use frame_system::offchain::SubmitTransaction;
use frame_system::pallet_prelude::BlockNumberFor;
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use num_traits::Zero;
use pallet_stableswap::types::PoolSnapshot;
use sp_core::offchain::Duration;
use sp_core::Get;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::offchain::storage_lock::Time;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::DispatchError;
use sp_runtime::Permill;
use sp_runtime::RuntimeDebug;
use sp_runtime::{
	offchain::storage_lock::StorageLock,
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
};
use sp_runtime::{AccountId32, Saturating};
use sp_runtime::{ArithmeticError, Rounding};

pub mod math;
pub mod traits;
pub mod types;

#[cfg(test)]
pub mod tests;

// Generate the ERC20 function selectors
#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum ERC20Function {
	Mint = "mint(address,uint256)",
	Burn = "burn(uint256)",
}

/// Unsigned transaction priority for arbitrage
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

/// Offchain Worker lock
pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"hydradx/hsm/arbitrage-lock/";
/// Lock timeout in milliseconds
pub const LOCK_TIMEOUT: u64 = 5_000; // 5 seconds

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use frame_system::Origin;
	use sp_runtime::{traits::Zero, Perbill, Permill};
	use sp_std::prelude::*;
	// EVM imports

	/// HSM account id identifier
	pub const HSM_IDENTIFIER: &[u8] = b"hsm/acct";

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_stableswap::Config + SendTransactionTypes<Call<Self>>
	where
		<Self as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
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
		type Evm: EVM<crate::types::CallResult>;

		/// EVM address converter
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// The gas limit for the execution of EVM calls
		#[pallet::constant]
		type GasLimit: Get<u64>;
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
	pub enum Event<T: Config>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		/// A new collateral asset was added
		CollateralAdded {
			asset_id: T::AssetId,
			pool_id: T::AssetId,
			purchase_fee: Permill,
			max_buy_price_coefficient: CoefficientRatio,
			buy_back_fee: Permill,
			b: Perbill,
		},
		/// A collateral asset was removed
		CollateralRemoved { asset_id: T::AssetId, amount: Balance },
		/// A collateral asset was updated
		CollateralUpdated {
			asset_id: T::AssetId,
			purchase_fee: Option<Permill>,
			max_buy_price_coefficient: Option<CoefficientRatio>,
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
		/// Arbitrage executed
		ArbitrageExecuted {
			asset_id: T::AssetId,
			hollar_amount: Balance,
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
		/// No arbitrage opportunity
		NoArbitrageOpportunity,
		/// Offchain lock error
		OffchainLockError,
		/// Asset not in the pool.
		AssetNotFound,
		/// Provided pool state is invalid
		InvalidPoolState,
		/// Collateral is not empty
		CollateralNotEmpty,
		/// Asset not in the pool
		AssetNotInPool,
		/// Hollar is not in the pool
		HollarNotInPool,
		/// Insufficient collateral balance
		InsufficientCollateralBalance,
		/// This collateral asset is not accepted now.
		CollateralNotWanted,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		fn on_finalize(_n: BlockNumberFor<T>) {
			// Clear the Hollar Amount Received storage on finalize
			<HollarAmountReceived<T>>::clear(u32::MAX, None);
		}

		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// Only validators should run the offchain worker
			if sp_io::offchain::is_validator() {
				let _ = Self::process_arbitrage_opportunities(block_number);
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
					// Disallow external unsigned transactions
					return InvalidTransaction::Call.into();
				}
				TransactionSource::Local => {}   // produced by our offchain worker
				TransactionSource::InBlock => {} // included in block
			};

			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("hsm-execute-arbitrage")
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides([&provide])
					.longevity(3)
					.propagate(false)
					.build()
			};

			match call {
				Call::execute_arbitrage { collateral_asset_id } => valid_tx(b"execute_arbitrage".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
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
			max_buy_price_coefficient: CoefficientRatio,
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
			// Ensure pool exists and pool assets contains hollar
			let pool_state = Self::get_stablepool_state(pool_id)?;
			ensure!(
				pool_state.assets.contains(&T::HollarId::get()),
				Error::<T>::HollarNotInPool
			);
			// also collateral asset must be in the pool
			ensure!(pool_state.assets.contains(&asset_id), Error::<T>::AssetNotInPool);

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

			// Ensure the collateral is empty
			ensure!(amount.is_zero(), Error::<T>::CollateralNotEmpty);

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
			max_buy_price_coefficient: Option<CoefficientRatio>,
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
				Self::do_collateral_out_given_hollar_in(&who, asset_out, amount_in)?
			} else {
				// Selling collateral to get Hollar
				Self::do_hollar_out_given_collateral_in(&who, asset_in, amount_in)?
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
				Self::do_collateral_in_given_hollar_out(&who, asset_in, amount_out)?
			} else {
				// Buying collateral with Hollar
				Self::do_hollar_in_given_collateral_out(&who, asset_out, amount_out)?
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

		/// Execute arbitrage opportunity between HSM and collateral stable pool
		///
		/// This call detects and executes arbitrage opportunities by minting Hollar,
		/// swapping it for collateral on HSM, then swapping that collateral for Hollar
		/// on the stable pool, and burning the hollar at the end.
		///
		/// The call is unsigned and should only be called by the offchain worker.
		///
		/// Parameters:
		/// - `collateral_asset_id`: The ID of the collateral asset to check for arbitrage
		#[pallet::call_index(5)]
		#[pallet::weight(10_000)]
		pub fn execute_arbitrage(origin: OriginFor<T>, collateral_asset_id: T::AssetId) -> DispatchResult {
			ensure_none(origin)?;

			// Check if the asset is a valid collateral
			ensure!(Self::is_collateral(collateral_asset_id), Error::<T>::AssetNotApproved);

			// Get the collateral info
			let collateral_info = Self::collaterals(collateral_asset_id).ok_or(Error::<T>::AssetNotApproved)?;

			// Calculate the arbitrage opportunity
			let hollar_amount_to_trade = Self::calculate_arbitrage_opportunity(collateral_asset_id, &collateral_info)?;

			// If there's an opportunity, execute it
			if hollar_amount_to_trade > 0 {
				let hsm_account = Self::account_id();

				// Mint hollar
				Self::mint_hollar(&hsm_account, hollar_amount_to_trade)?;

				// Sell hollar in HSM for collateral
				let collateral_received =
					Self::do_collateral_out_given_hollar_in(&hsm_account, collateral_asset_id, hollar_amount_to_trade)?;

				// Buy hollar in the collateral stable pool
				let origin: OriginFor<T> = Origin::<T>::Signed(hsm_account.clone()).into();
				pallet_stableswap::Pallet::<T>::buy(
					origin,
					collateral_info.pool_id,
					T::HollarId::get(),
					collateral_asset_id,
					hollar_amount_to_trade,
					collateral_received,
				)?;

				// Burn the hollar
				Self::burn_hollar(hollar_amount_to_trade)?;

				// Emit event
				Self::deposit_event(Event::<T>::ArbitrageExecuted {
					asset_id: collateral_asset_id,
					hollar_amount: hollar_amount_to_trade,
				});

				Ok(())
			} else {
				Err(Error::<T>::NoArbitrageOpportunity.into())
			}
		}
	}
}

impl<T: Config> Pallet<T>
where
	T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	/// Get the account ID of the HSM
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Check if an asset is an approved collateral
	pub fn is_collateral(asset_id: T::AssetId) -> bool {
		Collaterals::<T>::contains_key(asset_id)
	}

	fn get_stablepool_state(pool_id: T::AssetId) -> Result<PoolSnapshot<T::AssetId>, DispatchError> {
		let Some(pool_snapshot) = pallet_stableswap::Pallet::<T>::initial_pool_snapshot(pool_id) else {
			return Err(pallet_stableswap::Error::<T>::PoolNotFound.into());
		};
		Ok(pool_snapshot)
	}

	/// Selling Hollar to get collateral asset
	fn do_collateral_out_given_hollar_in(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		hollar_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		// Get pool data
		let pool_state = Self::get_stablepool_state(pool_id)?;

		let hollar_pos = pool_state
			.asset_idx(T::HollarId::get())
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_pos = pool_state
			.asset_idx(collateral_asset)
			.ok_or(Error::<T>::AssetNotFound)?;

		// just to be on the safe side
		ensure!(
			pool_state.reserves.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);
		ensure!(
			pool_state.pegs.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);

		// Get reserves and pegs
		let hollar_reserve = pool_state
			.asset_reserve_at(hollar_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_reserve = pool_state
			.asset_reserve_at(collateral_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let peg = pool_state.pegs[collateral_pos]; // hollar/collateral

		// TODO: take decimals into account
		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;

		ensure!(!imbalance.is_zero(), Error::<T>::CollateralNotWanted);

		// 2. Calculate how much Hollar can HSM buy back in a single block
		let buyback_limit = crate::math::calculate_buyback_limit(imbalance, collateral_info.b);

		// Check if the requested amount exceeds the buyback limit
		ensure!(
			HollarAmountReceived::<T>::get(collateral_asset).saturating_add(hollar_amount) <= buyback_limit,
			Error::<T>::MaxBuyBackExceeded
		);

		// 3. Calculate execution price by simulating a swap
		let input_amount = Self::simulate_in_given_out(
			pool_id,
			collateral_asset,
			T::HollarId::get(),
			hollar_amount,
			Balance::MAX,
			&pool_state,
		)?;

		let execution_price = (input_amount, hollar_amount);

		// 4. Calculate final buy price with fee
		let buy_price = crate::math::calculate_buy_price_with_fee(execution_price, collateral_info.buy_back_fee)?;

		// 5. Calculate amount of collateral to receive
		let collateral_amount =
			crate::math::calculate_collateral_amount(hollar_amount, buy_price).ok_or(ArithmeticError::Overflow)?;

		// 6. Calculate max price
		let max_price = crate::math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);

		// Check if price exceeds max price - compare the ratios
		// For (a,b) <= (c,d), we check a*d <= b*c
		let buy_price_check = buy_price.0.saturating_mul(max_price.1);
		let max_price_check = buy_price.1.saturating_mul(max_price.0);
		ensure!(buy_price_check <= max_price_check, Error::<T>::MaxBuyPriceExceeded);

		// 7. Check max holding limit if configured
		if let Some(max_holding) = collateral_info.max_in_holding {
			let current_holding = CollateralHoldings::<T>::get(collateral_asset);
			ensure!(
				current_holding.saturating_add(collateral_amount) <= max_holding,
				Error::<T>::MaxHoldingExceeded
			);
		}

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
			*amount = amount.saturating_add(hollar_amount);
		});

		Ok(collateral_amount)
	}

	/// Buying collateral asset using Hollar
	fn do_hollar_in_given_collateral_out(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		collateral_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;
		// Get pool data
		let pool_state = Self::get_stablepool_state(pool_id)?;

		let hollar_pos = pool_state
			.asset_idx(T::HollarId::get())
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_pos = pool_state
			.asset_idx(collateral_asset)
			.ok_or(Error::<T>::AssetNotFound)?;

		// just to be on the safe side
		ensure!(
			pool_state.reserves.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);
		ensure!(
			pool_state.pegs.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);

		// Get reserves and pegs
		let hollar_reserve = pool_state
			.asset_reserve_at(hollar_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_reserve = pool_state
			.asset_reserve_at(collateral_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let peg = pool_state.pegs[collateral_pos]; // hollar/collateral

		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;

		// 2. Calculate how much Hollar can HSM buy back in a single block
		let buyback_limit = crate::math::calculate_buyback_limit(imbalance, collateral_info.b);

		// 3. Calculate execution price by simulating a swap
		let hollar_amount = Self::simulate_out_given_in(
			pool_id,
			collateral_asset,
			T::HollarId::get(),
			collateral_amount,
			0,
			&pool_state,
		)?;

		// Create a PegType for execution price (hollar_amount/collateral_amount)
		let execution_price = (collateral_amount, hollar_amount);

		// 4. Calculate final buy price with fee
		let buy_price = crate::math::calculate_buy_price_with_fee(execution_price, collateral_info.buy_back_fee)?;

		// 5. Calculate amount of Hollar to pay
		let hollar_amount_to_pay =
			crate::math::calculate_hollar_amount(collateral_amount, buy_price).ok_or(ArithmeticError::Overflow)?;

		// Check if the requested amount exceeds the buyback limit
		ensure!(buyback_limit > hollar_amount_to_pay, Error::<T>::MaxBuyBackExceeded);

		// 6. Calculate max price
		let max_price = crate::math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);
		// Check if price exceeds max price - compare the ratios
		// For (a,b) <= (c,d), we check a*d <= b*c
		let buy_price_check = buy_price.0.saturating_mul(max_price.1);
		let max_price_check = buy_price.1.saturating_mul(max_price.0);
		ensure!(buy_price_check <= max_price_check, Error::<T>::MaxBuyPriceExceeded);

		// Check HSM has enough collateral
		let current_holding = CollateralHoldings::<T>::get(collateral_asset);
		ensure!(
			current_holding >= collateral_amount,
			Error::<T>::InsufficientCollateralBalance
		);

		// Execute the swap
		// 1. Transfer hollar from user to HSM
		<T as Config>::Currency::transfer(
			T::HollarId::get(),
			who,
			&Self::account_id(),
			hollar_amount_to_pay,
			Preservation::Expendable,
		)?;

		// 2. Transfer collateral from HSM to user
		<T as Config>::Currency::transfer(
			collateral_asset,
			&Self::account_id(),
			who,
			collateral_amount,
			Preservation::Expendable,
		)?;

		// 3. Burn Hollar by calling GHO contract
		Self::burn_hollar(hollar_amount_to_pay)?;

		// 3. Update HSM holdings
		CollateralHoldings::<T>::mutate(collateral_asset, |balance| {
			*balance = balance.saturating_sub(collateral_amount);
		});

		// 5. Update Hollar amount received in this block
		HollarAmountReceived::<T>::mutate(collateral_asset, |amount| {
			*amount = amount.saturating_add(hollar_amount_to_pay);
		});

		Ok(hollar_amount_to_pay)
	}

	/// Selling collateral asset to get Hollar
	fn do_hollar_out_given_collateral_in(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		collateral_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		let pool_state = Self::get_stablepool_state(pool_id)?;

		let collateral_pos = pool_state
			.asset_idx(collateral_asset)
			.ok_or(Error::<T>::AssetNotFound)?;

		// Get the peg for this asset
		// peg is  price hollar / collateral asset
		let peg = pool_state.pegs[collateral_pos];

		// Calculate purchase pice
		let purchase_price = crate::math::calculate_purchase_price(peg, collateral_info.purchase_fee);

		// Calculate Hollar amount to mint
		let hollar_amount =
			crate::math::calculate_hollar_amount(collateral_amount, purchase_price).ok_or(ArithmeticError::Overflow)?;

		// Execute the "swap"
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
	fn do_collateral_in_given_hollar_out(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		hollar_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;

		let pool_id = collateral_info.pool_id;

		let pool_state = Self::get_stablepool_state(pool_id)?;

		let collateral_pos = pool_state
			.asset_idx(collateral_asset)
			.ok_or(Error::<T>::AssetNotFound)?;

		// Get the peg for this asset
		// peg is  price hollar / collateral asset
		let peg = pool_state.pegs[collateral_pos];

		// 1. Calculate purchase price with fee
		let purchase_price = crate::math::calculate_purchase_price(peg, collateral_info.purchase_fee);

		// 2. Calculate amount of collateral needed
		let collateral_amount =
			crate::math::calculate_collateral_amount(hollar_amount, purchase_price).ok_or(ArithmeticError::Overflow)?;

		// Check user has enough collateral
		ensure!(
			<T as Config>::Currency::reducible_balance(collateral_asset, who, Preservation::Protect, Fortitude::Polite)
				>= collateral_amount,
			Error::<T>::InsufficientCollateralBalance
		);

		if let Some(max_holding) = collateral_info.max_in_holding {
			let current_holding = CollateralHoldings::<T>::get(collateral_asset);
			ensure!(
				current_holding.saturating_add(collateral_amount) <= max_holding,
				Error::<T>::MaxHoldingExceeded
			);
		}

		// Execute the "swap"
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

		Ok(collateral_amount)
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

	fn simulate_out_given_in(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		min_amount_out: Balance,
		pool_state: &PoolSnapshot<T::AssetId>,
	) -> Result<Balance, DispatchError> {
		let (amount_out, _) = pallet_stableswap::Pallet::<T>::simulate_sell(
			pool_id,
			asset_in,
			asset_out,
			amount_in,
			min_amount_out,
			Some(pool_state.clone()),
		)?;
		Ok(amount_out)
	}

	fn simulate_in_given_out(
		pool_id: T::AssetId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: Balance,
		max_amount_in: Balance,
		pool_state: &PoolSnapshot<T::AssetId>,
	) -> Result<Balance, DispatchError> {
		let (amount_in, _) = pallet_stableswap::Pallet::<T>::simulate_buy(
			pool_id,
			asset_in,
			asset_out,
			amount_out,
			max_amount_in,
			Some(pool_state.clone()),
		)?;
		Ok(amount_in)
	}

	/// Process arbitrage opportunities for all collateral assets
	fn process_arbitrage_opportunities(block_number: BlockNumberFor<T>) -> Result<(), DispatchError> {
		// Create a lock to ensure only one offchain worker runs at a time
		let lock_expiration = Duration::from_millis(LOCK_TIMEOUT);
		let mut lock = StorageLock::<'_, Time>::with_deadline(OFFCHAIN_WORKER_LOCK, lock_expiration);

		// Try to obtain the lock, if not possible, return early
		let r = if let Ok(_guard) = lock.try_lock() {
			log::debug!(
				target: "hsm::offchain_worker",
				"Processing arbitrage opportunities at block: {:?}", block_number
			);

			// Iterate over all collateral assets
			for (asset_id, _) in <Collaterals<T>>::iter() {
				// Check if the asset has collateral holdings
				if <CollateralHoldings<T>>::get(asset_id) > 0 {
					// Try to submit an unsigned transaction to execute arbitrage
					let call = Call::execute_arbitrage {
						collateral_asset_id: asset_id,
					};

					if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()) {
						log::error!(
							target: "hsm::offchain_worker",
							"Failed to submit transaction for asset {:?}: {:?}", asset_id, e
						);
					}
				}
			}

			Ok(())
		} else {
			log::debug!(
				target: "hsm::offchain_worker",
				"Another instance of the offchain worker is already running"
			);
			Err(Error::<T>::OffchainLockError.into())
		};

		r
	}

	/// Calculate if there's an arbitrage opportunity for a collateral asset
	///
	/// Returns (max_buy_amt, hollar_amount_to_trade)
	fn calculate_arbitrage_opportunity(
		collateral_asset_id: T::AssetId,
		collateral_info: &CollateralInfo<T::AssetId>,
	) -> Result<Balance, DispatchError> {
		let hollar_id = T::HollarId::get();
		let pool_id = collateral_info.pool_id;

		// Get pool data
		let pool_state = Self::get_stablepool_state(pool_id)?;

		let hollar_pos = pool_state
			.asset_idx(T::HollarId::get())
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_pos = pool_state
			.asset_idx(collateral_asset_id)
			.ok_or(Error::<T>::AssetNotFound)?;

		// just to be on the safe side
		ensure!(
			pool_state.reserves.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);
		ensure!(
			pool_state.pegs.len() > hollar_pos.max(collateral_pos),
			Error::<T>::InvalidPoolState
		);
		// Calculate I_i = (H_i - φ_i * R_i) / 2
		let collateral_reserve = pool_state
			.asset_reserve_at(collateral_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let hollar_reserve = pool_state
			.asset_reserve_at(hollar_pos)
			.ok_or(Error::<T>::AssetNotFound)?;

		let peg = pool_state.pegs[collateral_pos]; // hollar/collateral

		// TODO: take decimals into account
		// 1. Calculate imbalance
		let imbalance = crate::math::calculate_imbalance(hollar_reserve, peg, collateral_reserve)?;
		ensure!(!imbalance.is_zero(), Error::<T>::NoArbitrageOpportunity);
		let b_coefficient = collateral_info.b;
		let max_buy_amt = b_coefficient.mul_floor(imbalance);
		// If max_buy_amt is 0, there's no arbitrage opportunity
		if max_buy_amt == 0 {
			return Ok(0);
		}

		// Simulate swap to determine execution price
		// How much collateral asset we need to sell to buy max_buy_amt of Hollar
		let sell_amt = Self::simulate_in_given_out(
			pool_id,
			collateral_asset_id,
			hollar_id,
			max_buy_amt,
			Balance::MAX,
			&pool_state,
		)?;
		// Execution price is p_i = sell_amt / max_buy_amt
		let execution_price = (sell_amt, max_buy_amt);

		// Apply fee factor: buy_price = p_i / (1 - f)
		let fee = collateral_info.buy_back_fee;
		let fee_complement = Permill::from_percent(100).saturating_sub(fee);

		let exec_prica_ratio: hydra_dx_math::ratio::Ratio = execution_price.into();
		let fee_ratio: hydra_dx_math::ratio::Ratio = (fee_complement.deconstruct() as u128, 1_000_000u128).into();
		let buy_price_ratio = exec_prica_ratio.saturating_div(&fee_ratio);
		let buy_price = (buy_price_ratio.n, buy_price_ratio.d);

		let max_price = crate::math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);

		// Check if price exceeds max price - compare the ratios
		// For (a,b) <= (c,d), we check a*d <= b*c
		let buy_price_check = buy_price.0.saturating_mul(max_price.1);
		let max_price_check = buy_price.1.saturating_mul(max_price.0);
		ensure!(buy_price_check <= max_price_check, Error::<T>::MaxBuyPriceExceeded);

		// Calculate the amount of Hollar to trade
		// max_buy_amt = min(max_buy_amt, self.liquidity[tkn] / buy_price)
		let asset_holding = Self::collateral_holdings(collateral_asset_id);
		let max_holding_liquidity_amt =
			multiply_by_rational_with_rounding(asset_holding, buy_price.1, buy_price.0, Rounding::Down)
				.ok_or(ArithmeticError::Overflow)?;

		let max_buy_amt = sp_std::cmp::min(max_buy_amt, max_holding_liquidity_amt);

		// amount of hollar to trade = max(max_buy_amt - _HollarAmountReceived_, 0)
		let hollar_amount_received = Self::hollar_amount_received(collateral_asset_id);
		let hollar_amount_to_trade = max_buy_amt.saturating_sub(hollar_amount_received);

		Ok(hollar_amount_to_trade)
	}
}
