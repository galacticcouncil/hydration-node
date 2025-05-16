#![cfg_attr(not(feature = "std"), no_std)]

//! # Hollar Stability Mechanism (HSM) Pallet
//!
//! ## Overview
//!
//! The HSM pallet implements a stability mechanism for the Hollar stablecoin within the HydraDX ecosystem.
//! It provides the infrastructure to maintain Hollar's peg by managing collateral assets and
//! facilitating the buying and selling of Hollar against these collaterals.
//!
//! The mechanism works by:
//! - Managing approved collateral assets for Hollar
//! - Handling minting and burning of Hollar through integration with the GHO ERC20 token contract
//! - Providing buy/sell functionality for users to exchange Hollar against collateral assets
//! - Executing arbitrage opportunities to maintain price stability via offchain workers
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! * `add_collateral_asset` - Add a new collateral asset for Hollar.
//! * `remove_collateral_asset` - Remove a collateral asset from the approved list.
//! * `update_collateral_asset` - Update parameters for an existing collateral asset.
//! * `sell` - Sell Hollar in exchange for collateral, or sell collateral for Hollar.
//! * `buy` - Buy Hollar with collateral, or buy collateral with Hollar.
//! * `execute_arbitrage` - Execute arbitrage opportunity between HSM and collateral stable pool (called by offchain worker).

pub use pallet::*;

use crate::types::{Balance, CoefficientRatio, CollateralInfo, Price};
pub use crate::weights::WeightInfo;
use ethabi::ethereum_types::BigEndianHash;
use evm::{ExitReason, ExitSucceed};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	require_transactional,
	traits::{
		fungibles::{Inspect, Mutate},
		tokens::Preservation,
	},
	PalletId,
};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
	Origin,
};
use hex_literal::hex;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::{
	evm::{CallContext, InspectEvmAccounts, EVM},
	registry::BoundErc20,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_stableswap::types::PoolSnapshot;
use sp_core::{offchain::Duration, Get, H256, U256};
use sp_runtime::{
	helpers_128bit::multiply_by_rational_with_rounding,
	offchain::storage_lock::{StorageLock, Time},
	traits::{AccountIdConversion, Zero},
	transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction},
	AccountId32, ArithmeticError, DispatchError, Perbill, Permill, Rounding, RuntimeDebug, Saturating,
};

mod math;
pub mod traits;
pub mod types;

#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks;
pub mod weights;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum ERC20Function {
	Mint = "mint(address,uint256)",
	Burn = "burn(uint256)",
}

/// Max number of approved assets.
/// The reason to have it is easier accounting for weight.
/// And since approved collateral asset must be in a pool with Hollar,
/// and there can be only one asset from a single pool -3 should be enough.
/// That means that we would have to have 3 pools with Hollar and some different assets.
pub const MAX_COLLATERALS: u32 = 10;

/// Unsigned transaction priority for arbitrage
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

/// Offchain Worker lock
pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"hydradx/hsm/arbitrage-lock/";
/// Lock timeout in milliseconds
pub const LOCK_TIMEOUT: u64 = 5_000; // 5 seconds

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use pallet_broadcast::types::Asset;
	use pallet_evm::GasWeightMapping;
	use sp_std::vec;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_stableswap::Config + pallet_broadcast::Config + SendTransactionTypes<Call<Self>>
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

		/// Origin that can manage collateral assets
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// GHO contract address - EVM address of GHO token contract
		type GhoContractAddress: BoundErc20<AssetId = Self::AssetId>;

		/// Currency - fungible tokens trait to access token transfers
		type Currency: Mutate<Self::AccountId, Balance = Balance, AssetId = Self::AssetId>;

		/// EVM handler
		type Evm: EVM<types::CallResult>;

		/// EVM address converter
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// The gas limit for the execution of EVM calls
		#[pallet::constant]
		type GasLimit: Get<u64>;

		/// Gas to Weight conversion.
		type GasWeightMapping: GasWeightMapping;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: traits::BenchmarkHelper<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// List of approved assets that Hollar can be purchased with
	///
	/// This storage maps asset IDs to their collateral configuration information.
	/// Only assets in this map can be used to mint or redeem Hollar through HSM.
	/// Each collateral has specific parameters controlling its usage in the HSM mechanism.
	#[pallet::storage]
	#[pallet::getter(fn collaterals)]
	pub type Collaterals<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, CollateralInfo<T::AssetId>>;

	/// Amount of Hollar bought with an asset in a single block
	///
	/// This storage tracks how much Hollar has been bought back by HSM for each collateral
	/// asset within the current block. This is used to enforce rate limiting on Hollar redemptions.
	/// Values are reset to zero at the end of each block in on_finalize.
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
		///
		/// Parameters:
		/// - `asset_id`: The ID of the asset added as collateral
		/// - `pool_id`: The StableSwap pool ID where this asset belongs
		/// - `purchase_fee`: Fee applied when buying Hollar with this asset
		/// - `max_buy_price_coefficient`: Maximum buy price coefficient for HSM to buy back Hollar
		/// - `buy_back_fee`: Fee applied when buying back Hollar
		/// - `buyback_rate`: Parameter that controls how quickly HSM can buy Hollar with this asset
		CollateralAdded {
			asset_id: T::AssetId,
			pool_id: T::AssetId,
			purchase_fee: Permill,
			max_buy_price_coefficient: CoefficientRatio,
			buy_back_fee: Permill,
			buyback_rate: Perbill,
		},
		/// A collateral asset was removed
		///
		/// Parameters:
		/// - `asset_id`: The ID of the asset removed from collaterals
		/// - `amount`: The amount of the asset that was returned (should be zero)
		CollateralRemoved { asset_id: T::AssetId, amount: Balance },
		/// A collateral asset was updated
		///
		/// Parameters:
		/// - `asset_id`: The ID of the updated collateral asset
		/// - `purchase_fee`: New purchase fee if updated (None if not changed)
		/// - `max_buy_price_coefficient`: New max buy price coefficient if updated (None if not changed)
		/// - `buy_back_fee`: New buy back fee if updated (None if not changed)
		/// - `buyback_rate`: New buyback rate if updated (None if not changed)
		CollateralUpdated {
			asset_id: T::AssetId,
			purchase_fee: Option<Permill>,
			max_buy_price_coefficient: Option<CoefficientRatio>,
			buy_back_fee: Option<Permill>,
			buyback_rate: Option<Perbill>,
		},
		/// Arbitrage executed successfully
		///
		/// Parameters:
		/// - `asset_id`: The collateral asset used in the arbitrage
		/// - `hollar_amount`: Amount of Hollar that was included in the arbitrage operation
		ArbitrageExecuted {
			asset_id: T::AssetId,
			hollar_amount: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset is not approved as collateral
		///
		/// The operation attempted to use an asset that is not registered as an approved collateral.
		AssetNotApproved,
		/// Asset is already approved as collateral
		///
		/// Attempted to add an asset that is already registered as a collateral.
		AssetAlreadyApproved,
		/// Another asset from the same pool is already approved
		///
		/// Only one asset from each StableSwap pool can be used as collateral.
		PoolAlreadyHasCollateral,
		/// Invalid asset pair, must be Hollar and approved collateral
		///
		/// The asset pair for buy/sell operations must include Hollar as one side and an approved collateral as the other.
		InvalidAssetPair,
		/// Max buy price exceeded
		///
		/// The calculated buy price exceeds the maximum allowed buy price for the collateral.
		MaxBuyPriceExceeded,
		/// Max buy back amount in single block exceeded
		///
		/// The amount of Hollar being sold to HSM exceeds the maximum allowed in a single block for this collateral.
		MaxBuyBackExceeded,
		/// Max holding amount for collateral exceeded
		///
		/// The operation would cause the HSM to hold more of the collateral than the configured maximum.
		MaxHoldingExceeded,
		/// Slippage limit exceeded
		///
		/// The calculated amount is worse than the provided slippage limit.
		SlippageLimitExceeded,
		/// Invalid EVM contract interaction
		///
		/// The call to the EVM contract (GHO Hollar token) failed.
		InvalidEVMInteraction,
		/// Decimal retrieval failed
		///
		/// Failed to retrieve the decimal information for an asset.
		DecimalRetrievalFailed,
		/// No arbitrage opportunity
		///
		/// There is no profitable arbitrage opportunity for the specified collateral.
		NoArbitrageOpportunity,
		/// Offchain lock error
		///
		/// Failed to acquire the lock for offchain workers, likely because another operation is in progress.
		OffchainLockError,
		/// Asset not in the pool
		///
		/// The specified asset was not found in the pool.
		AssetNotFound,
		/// Provided pool state is invalid
		///
		/// The retrieved pool state has inconsistent or invalid data.
		InvalidPoolState,
		/// Collateral is not empty
		///
		/// Cannot remove a collateral asset that still has a non-zero balance in the HSM account.
		CollateralNotEmpty,
		/// Asset not in the pool
		///
		/// The collateral asset is not present in the specified pool.
		AssetNotInPool,
		/// Hollar is not in the pool
		///
		/// The Hollar asset is not present in the specified pool.
		HollarNotInPool,
		/// Insufficient collateral balance
		///
		/// The HSM does not have enough of the collateral asset to complete the operation.
		InsufficientCollateralBalance,
		/// GHO Contract address not found
		///
		/// The EVM address for the GHO (Hollar) token contract was not found.
		HollarContractAddressNotFound,

		/// HSM contains maximum number of allowed collateral assets.
		MaxNumberOfCollateralsReached,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		/// Accounting for weight in on finalize
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			<T as Config>::WeightInfo::on_finalize() * MAX_COLLATERALS as u64
		}

		/// Cleans up the HollarAmountReceived storage at the end of each block
		///
		/// This ensures that the rate limiting for Hollar buybacks is reset for the next block.
		fn on_finalize(_n: BlockNumberFor<T>) {
			let _ = <HollarAmountReceived<T>>::clear(u32::MAX, None);
		}

		/// Offchain worker entry point that processes arbitrage opportunities
		///
		/// This function:
		/// 1. Checks if the node is a validator
		/// 2. If so, attempts to find and execute arbitrage opportunities for all collateral assets
		/// 3. Only runs if it can obtain a lock (to prevent concurrent execution)
		fn offchain_worker(block_number: BlockNumberFor<T>) {
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

		/// Validates unsigned transactions for arbitrage execution
		///
		/// This function ensures that only valid arbitrage transactions originating from
		/// offchain workers are accepted, and prevents unauthorized external calls.
		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
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
				Call::execute_arbitrage { .. } => valid_tx(b"execute_arbitrage".to_vec()),
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
		/// This function adds a new asset as an approved collateral for Hollar. Only callable by
		/// the governance (root origin).
		///
		/// Parameters:
		/// - `origin`: Must be Root
		/// - `asset_id`: Asset ID to be added as collateral
		/// - `pool_id`: StableSwap pool ID where this asset and Hollar are paired
		/// - `purchase_fee`: Fee applied when buying Hollar with this asset (added to purchase price)
		/// - `max_buy_price_coefficient`: Maximum buy price coefficient for HSM to buy back Hollar
		/// - `buy_back_fee`: Fee applied when buying back Hollar (subtracted from buy price)
		/// - `buyback_rate`: Parameter that controls how quickly HSM can buy Hollar with this asset
		/// - `max_in_holding`: Optional maximum amount of collateral HSM can hold
		///
		/// Emits:
		/// - `CollateralAdded` when the collateral is successfully added
		///
		/// Errors:
		/// - `AssetAlreadyApproved` if the asset is already registered as a collateral
		/// - `PoolAlreadyHasCollateral` if another asset from the same pool is already approved
		/// - `HollarNotInPool` if Hollar is not found in the specified pool
		/// - `AssetNotInPool` if the collateral asset is not found in the specified pool
		/// - Other errors from underlying calls
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::add_collateral_asset())]
		#[allow(clippy::too_many_arguments)]
		pub fn add_collateral_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			pool_id: T::AssetId,
			purchase_fee: Permill,
			max_buy_price_coefficient: CoefficientRatio,
			buy_back_fee: Permill,
			buyback_rate: Perbill,
			max_in_holding: Option<Balance>,
		) -> DispatchResult {
			<T as Config>::AuthorityOrigin::ensure_origin(origin)?;

			let current_collateral_count = Collaterals::<T>::iter().count() as u32;

			ensure!(
				current_collateral_count < MAX_COLLATERALS,
				Error::<T>::MaxNumberOfCollateralsReached
			);

			ensure!(
				!Collaterals::<T>::contains_key(asset_id),
				Error::<T>::AssetAlreadyApproved
			);

			ensure!(asset_id != T::HollarId::get(), Error::<T>::AssetAlreadyApproved);

			// Check if there's already an asset from the same pool
			for (_, info) in Collaterals::<T>::iter() {
				ensure!(info.pool_id != pool_id, Error::<T>::PoolAlreadyHasCollateral);
			}
			// Ensure pool exists and pool assets contains hollar and collateral asset
			let pool_state = Self::get_stablepool_state(pool_id)?;
			ensure!(
				pool_state.assets.contains(&T::HollarId::get()),
				Error::<T>::HollarNotInPool
			);
			ensure!(pool_state.assets.contains(&asset_id), Error::<T>::AssetNotInPool);

			let collateral_info = CollateralInfo {
				pool_id,
				purchase_fee,
				max_buy_price_coefficient,
				buyback_rate,
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
				buyback_rate,
			});

			Ok(())
		}

		/// Remove a collateral asset
		///
		/// Removes an asset from the approved collaterals list. Only callable by the governance (root origin).
		/// The collateral must have a zero balance in the HSM account before it can be removed.
		///
		/// Parameters:
		/// - `origin`: Must be Root
		/// - `asset_id`: Asset ID to remove from collaterals
		///
		/// Emits:
		/// - `CollateralRemoved` when the collateral is successfully removed
		///
		/// Errors:
		/// - `AssetNotApproved` if the asset is not a registered collateral
		/// - `CollateralNotEmpty` if the HSM account still holds some of this asset
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_collateral_asset())]
		pub fn remove_collateral_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			<T as Config>::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(Collaterals::<T>::contains_key(asset_id), Error::<T>::AssetNotApproved);

			let amount = <T as Config>::Currency::total_balance(asset_id, &Self::account_id());
			ensure!(amount.is_zero(), Error::<T>::CollateralNotEmpty);

			Collaterals::<T>::remove(asset_id);

			Self::deposit_event(Event::<T>::CollateralRemoved { asset_id, amount });

			Ok(())
		}

		/// Update collateral asset parameters
		///
		/// Updates the parameters for an existing collateral asset. Only callable by the governance (root origin).
		/// Each parameter is optional and only provided parameters will be updated.
		///
		/// Parameters:
		/// - `origin`: Must be Root
		/// - `asset_id`: Asset ID to update
		/// - `purchase_fee`: New purchase fee (optional)
		/// - `max_buy_price_coefficient`: New max buy price coefficient (optional)
		/// - `buy_back_fee`: New buy back fee (optional)
		/// - `buyback_rate`: New buyback rate parameter (optional)
		/// - `max_in_holding`: New maximum holding amount (optional)
		///
		/// Emits:
		/// - `CollateralUpdated` when the collateral is successfully updated
		///
		/// Errors:
		/// - `AssetNotApproved` if the asset is not a registered collateral
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_collateral_asset())]
		pub fn update_collateral_asset(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			purchase_fee: Option<Permill>,
			max_buy_price_coefficient: Option<CoefficientRatio>,
			buy_back_fee: Option<Permill>,
			buyback_rate: Option<Perbill>,
			max_in_holding: Option<Option<Balance>>,
		) -> DispatchResult {
			<T as Config>::AuthorityOrigin::ensure_origin(origin)?;

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

				if let Some(param_b) = buyback_rate {
					info.buyback_rate = param_b;
				}

				if let Some(holding) = max_in_holding {
					info.max_in_holding = holding;
				}

				Self::deposit_event(Event::<T>::CollateralUpdated {
					asset_id,
					purchase_fee,
					max_buy_price_coefficient,
					buy_back_fee,
					buyback_rate,
				});
				Ok(())
			})
		}

		/// Sell asset to HSM
		///
		/// This function allows users to:
		/// 1. Sell Hollar back to HSM in exchange for collateral assets
		/// 2. Sell collateral assets to HSM in exchange for newly minted Hollar
		///
		/// The valid pairs must include Hollar as one side and an approved collateral as the other side.
		///
		/// Parameters:
		/// - `origin`: Account selling the asset
		/// - `asset_in`: Asset ID being sold
		/// - `asset_out`: Asset ID being bought
		/// - `amount_in`: Amount of asset_in to sell
		/// - `slippage_limit`: Minimum amount out for slippage protection
		///
		/// Emits:
		/// - `Swapped3` when the sell is successful
		///
		/// Errors:
		/// - `InvalidAssetPair` if the pair is not Hollar and an approved collateral
		/// - `AssetNotApproved` if the collateral asset isn't registered
		/// - `SlippageLimitExceeded` if the amount received is less than the slippage limit
		/// - `MaxBuyBackExceeded` if the sell would exceed the maximum buy back rate
		/// - `MaxBuyPriceExceeded` if the sell would exceed the maximum buy price
		/// - `InsufficientCollateralBalance` if HSM doesn't have enough collateral
		/// - `InvalidEVMInteraction` if there's an error interacting with the Hollar ERC20 contract
		/// - Other errors from underlying calls
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::sell()
			.saturating_add(<T as Config>::GasWeightMapping::gas_to_weight(<T as Config>::GasLimit::get(), true))
		)]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			slippage_limit: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let hollar_id = T::HollarId::get();

			Self::ensure_pair(asset_in, asset_out)?;

			let amount_out = if asset_in == hollar_id {
				// COLLATERAL OUT given HOLLAR IN
				let (hollar_in, collateral_out) = Self::do_trade_hollar_in(
					&who,
					asset_out,
					|pool_id, state| {
						//we need to know how much collateral needs to be paid for given hollar
						//so we simulate by buying exact amount of hollar
						let collateral_amount = Self::simulate_in_given_out(
							pool_id,
							asset_out,
							T::HollarId::get(),
							amount_in,
							Balance::MAX,
							state,
						)?;
						Ok((amount_in, collateral_amount))
					},
					|(hollar_amount, _), price| {
						let collateral_amount =
							math::calculate_collateral_amount(hollar_amount, price).ok_or(ArithmeticError::Overflow)?;
						Ok((hollar_amount, collateral_amount))
					},
				)?;

				debug_assert_eq!(hollar_in, amount_in);
				collateral_out
			} else {
				// HOLLAR OUT given COLLATERAL IN
				let (hollar_amount, collateral_amount) = Self::do_trade_hollar_out(&who, asset_in, |purchase_price| {
					let hollar_amount =
						math::calculate_hollar_amount(amount_in, purchase_price).ok_or(ArithmeticError::Overflow)?;
					Ok((hollar_amount, amount_in))
				})?;
				debug_assert_eq!(amount_in, collateral_amount);
				hollar_amount
			};

			ensure!(amount_out >= slippage_limit, Error::<T>::SlippageLimitExceeded);

			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				Self::account_id(),
				pallet_broadcast::types::Filler::HSM,
				pallet_broadcast::types::TradeOperation::ExactIn,
				vec![Asset::new(asset_in.into(), amount_in)],
				vec![Asset::new(asset_out.into(), amount_out)],
				vec![],
			);

			Ok(())
		}

		/// Buy asset from HSM
		///
		/// This function allows users to:
		/// 1. Buy Hollar from HSM using collateral assets
		/// 2. Buy collateral assets from HSM using Hollar
		///
		/// The valid pairs must include Hollar as one side and an approved collateral as the other side.
		///
		/// Parameters:
		/// - `origin`: Account buying the asset
		/// - `asset_in`: Asset ID being sold by the user
		/// - `asset_out`: Asset ID being bought by the user
		/// - `amount_out`: Amount of asset_out to buy
		/// - `slippage_limit`: Maximum amount in for slippage protection
		///
		/// Emits:
		/// - `Swapped3` when the buy is successful
		///
		/// Errors:
		/// - `InvalidAssetPair` if the pair is not Hollar and an approved collateral
		/// - `AssetNotApproved` if the collateral asset isn't registered
		/// - `SlippageLimitExceeded` if the amount input exceeds the slippage limit
		/// - `MaxHoldingExceeded` if the buy would cause HSM to exceed its maximum holding
		/// - `InvalidEVMInteraction` if there's an error interacting with the Hollar ERC20 contract
		/// - Other errors from underlying calls
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::buy()
			.saturating_add(<T as Config>::GasWeightMapping::gas_to_weight(<T as Config>::GasLimit::get(), true))
		)]
		pub fn buy(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_out: Balance,
			slippage_limit: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let hollar_id = T::HollarId::get();

			Self::ensure_pair(asset_in, asset_out)?;

			let amount_in = if asset_out == hollar_id {
				// COLLATERAL IN given HOLLAR OUT
				let (hollar_out, collateral_in) = Self::do_trade_hollar_out(&who, asset_in, |purchase_price| {
					let collateral_amount = math::calculate_collateral_amount(amount_out, purchase_price)
						.ok_or(ArithmeticError::Overflow)?;
					Ok((amount_out, collateral_amount))
				})?;
				debug_assert_eq!(hollar_out, amount_out);
				collateral_in
			} else {
				// HOLLAR IN given COLLATERAL OUT
				let (hollar_in, collateral_out) = Self::do_trade_hollar_in(
					&who,
					asset_out,
					|pool_id, state| {
						//we need to know how much hollar needs to be paid for given collateral amount
						//so we simulate by selling exact collateral in
						let hollar_amount =
							Self::simulate_out_given_in(pool_id, asset_out, T::HollarId::get(), amount_out, 0, state)?;
						Ok((hollar_amount, amount_out))
					},
					|(_, collateral_amount), price| {
						let hollar_amount_to_pay =
							math::calculate_hollar_amount(collateral_amount, price).ok_or(ArithmeticError::Overflow)?;
						Ok((hollar_amount_to_pay, collateral_amount))
					},
				)?;

				debug_assert_eq!(amount_out, collateral_out);
				hollar_in
			};

			ensure!(amount_in <= slippage_limit, Error::<T>::SlippageLimitExceeded);

			pallet_broadcast::Pallet::<T>::deposit_trade_event(
				who,
				Self::account_id(),
				pallet_broadcast::types::Filler::HSM,
				pallet_broadcast::types::TradeOperation::ExactOut,
				vec![Asset::new(asset_in.into(), amount_in)],
				vec![Asset::new(asset_out.into(), amount_out)],
				vec![],
			);

			Ok(())
		}

		/// Execute arbitrage opportunity between HSM and collateral stable pool
		///
		/// This call is designed to be triggered automatically by offchain workers. It:
		/// 1. Detects price imbalances between HSM and a stable pool for a collateral
		/// 2. If an opportunity exists, mints Hollar, swaps it for collateral on HSM
		/// 3. Swaps that collateral for Hollar on the stable pool
		/// 4. Burns the Hollar received from the arbitrage
		///
		/// This helps maintain the peg of Hollar by profiting from and correcting price imbalances.
		/// The call is unsigned and should only be executed by offchain workers.
		///
		/// Parameters:
		/// - `origin`: Must be None (unsigned)
		/// - `collateral_asset_id`: The ID of the collateral asset to check for arbitrage
		///
		/// Emits:
		/// - `ArbitrageExecuted` when the arbitrage is successful
		///
		/// Errors:
		/// - `AssetNotApproved` if the asset is not a registered collateral
		/// - `NoArbitrageOpportunity` if there's no profitable arbitrage opportunity
		/// - `MaxBuyPriceExceeded` if the arbitrage would exceed the maximum buy price
		/// - `InvalidEVMInteraction` if there's an error interacting with the Hollar ERC20 contract
		/// - Other errors from underlying calls
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::execute_arbitrage())]
		pub fn execute_arbitrage(origin: OriginFor<T>, collateral_asset_id: T::AssetId) -> DispatchResult {
			ensure_none(origin)?;

			let collateral_info = Self::collaterals(collateral_asset_id).ok_or(Error::<T>::AssetNotApproved)?;

			let hollar_amount_to_trade = Self::calculate_arbitrage_opportunity(collateral_asset_id, &collateral_info)?;

			if hollar_amount_to_trade > 0 {
				let hsm_account = Self::account_id();

				Self::mint_hollar(&hsm_account, hollar_amount_to_trade)?;

				// Sell hollar to HSM for collateral
				let (hollar_amount, collateral_received) = Self::do_trade_hollar_in(
					&hsm_account,
					collateral_asset_id,
					|pool_id, state| {
						//we need to know how much collateral needs to be paid for given hollar
						//so we simulate by buying exact amount of hollar
						let collateral_amount = Self::simulate_in_given_out(
							pool_id,
							collateral_asset_id,
							T::HollarId::get(),
							hollar_amount_to_trade,
							Balance::MAX,
							state,
						)?;
						Ok((hollar_amount_to_trade, collateral_amount))
					},
					|(hollar_amount, _), price| {
						let collateral_amount =
							math::calculate_collateral_amount(hollar_amount, price).ok_or(ArithmeticError::Overflow)?;
						Ok((hollar_amount, collateral_amount))
					},
				)?;
				debug_assert_eq!(hollar_amount, hollar_amount_to_trade);

				// Buy hollar from the collateral stable pool
				let origin: OriginFor<T> = Origin::<T>::Signed(hsm_account.clone()).into();
				pallet_stableswap::Pallet::<T>::buy(
					origin,
					collateral_info.pool_id,
					T::HollarId::get(),
					collateral_asset_id,
					hollar_amount_to_trade,
					collateral_received,
				)?;

				Self::burn_hollar(hollar_amount_to_trade)?;

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
	///
	/// Returns the account that holds all HSM funds and interacts with the GHO contract.
	/// The account is derived from the configured PalletId.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Check if an asset is an approved collateral
	///
	/// Returns true if the asset is in the Collaterals storage map, false otherwise.
	#[inline]
	fn is_collateral(asset_id: T::AssetId) -> bool {
		Collaterals::<T>::contains_key(asset_id)
	}

	/// Ensures that the asset pair is valid for HSM operations
	///
	/// A valid pair must include Hollar as one side and an approved collateral as the other.
	/// Returns Ok if the pair is valid, or an error otherwise.
	fn ensure_pair(asset_in: T::AssetId, asset_out: T::AssetId) -> DispatchResult {
		ensure!(
			(asset_in == T::HollarId::get() && Self::is_collateral(asset_out))
				|| (asset_out == T::HollarId::get() && Self::is_collateral(asset_in)),
			Error::<T>::InvalidAssetPair
		);

		Ok(())
	}

	/// Retrieves the state of a StableSwap pool
	///
	/// Gets the pool snapshot containing assets, reserves, pegs and other pool information.
	/// Returns an error if the pool doesn't exist or other retrieval errors occur.
	fn get_stablepool_state(pool_id: T::AssetId) -> Result<PoolSnapshot<T::AssetId>, DispatchError> {
		let Some(pool_snapshot) = pallet_stableswap::Pallet::<T>::initial_pool_snapshot(pool_id) else {
			return Err(pallet_stableswap::Error::<T>::PoolNotFound.into());
		};
		Ok(pool_snapshot)
	}

	/// Checks if adding more collateral would exceed the maximum holding limit
	///
	/// Returns true if either:
	/// 1. There is no maximum holding configured for this collateral, or
	/// 2. The current balance plus the new amount is within the maximum holding limit
	fn ensure_max_collateral_holding(
		asset_id: T::AssetId,
		info: &CollateralInfo<T::AssetId>,
		collateral_in: Balance,
	) -> bool {
		if let Some(max_holding) = info.max_in_holding {
			let current_holding = <T as Config>::Currency::total_balance(asset_id, &Self::account_id());
			current_holding.saturating_add(collateral_in) <= max_holding
		} else {
			true
		}
	}

	/// Processes Hollar coming into HSM in exchange for collateral
	///
	/// This function handles:
	/// 1. Calculating the execution price and fees
	/// 2. Ensuring various limits are not exceeded
	/// 3. Transferring Hollar from user to HSM
	/// 4. Transferring collateral from HSM to user
	/// 5. Burning the received Hollar
	/// 6. Updating the amount of Hollar received in this block
	///
	/// Returns the final Hollar and collateral amounts traded.
	#[require_transactional]
	fn do_trade_hollar_in(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		simulate_swap: impl FnOnce(T::AssetId, &PoolSnapshot<T::AssetId>) -> Result<(Balance, Balance), DispatchError>,
		calculate_final_amounts: impl FnOnce((Balance, Balance), Price) -> Result<(Balance, Balance), DispatchError>,
	) -> Result<(Balance, Balance), DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;
		let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
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

		let hollar_reserve = pool_state
			.asset_reserve_at(hollar_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let collateral_reserve = pool_state
			.asset_reserve_at(collateral_pos)
			.ok_or(Error::<T>::AssetNotFound)?;

		// If a pool does not have peg source set,
		// we need to correctly set the peg, respecting asset decimals
		let peg = if pool_state.has_peg_source_set::<T>(collateral_info.pool_id) {
			pool_state.pegs[collateral_pos]
		} else {
			let hollar_decimals = pool_state
				.asset_decimals_at(hollar_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			let collateral_decimals = pool_state
				.asset_decimals_at(collateral_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			(
				10u128.pow(hollar_decimals as u32),
				10u128.pow(collateral_decimals as u32),
			)
		};

		// 1. Calculate imbalance
		let imbalance =
			math::calculate_imbalance(hollar_reserve, peg, collateral_reserve).ok_or(ArithmeticError::Overflow)?;

		// 2. Calculate how much Hollar can HSM buy back in a single block
		let buyback_limit = math::calculate_buyback_limit(imbalance, collateral_info.buyback_rate);

		// 3. Simulate swap in pool
		let (sim_hollar_amount, sim_collateral_amount) = simulate_swap(collateral_info.pool_id, &pool_state)?;

		// Create a PegType for execution price (hollar_amount/collateral_amount)
		let execution_price = (sim_collateral_amount, sim_hollar_amount);

		// 4. Calculate final buy price with fee
		let buy_price = math::calculate_buy_price_with_fee(execution_price, collateral_info.buy_back_fee)
			.ok_or(ArithmeticError::Overflow)?;

		// %. Calculate final swap amounts
		let (final_hollar_amount, final_collateral_amount) =
			calculate_final_amounts((sim_hollar_amount, sim_collateral_amount), buy_price)?;

		log::trace!(target: "hsm", "Hollar amount {:?}, buyback limit {:?}", final_hollar_amount, buyback_limit);
		// Check if the requested amount exceeds the buyback limit
		ensure!(
			HollarAmountReceived::<T>::get(collateral_asset).saturating_add(final_hollar_amount) <= buyback_limit,
			Error::<T>::MaxBuyBackExceeded
		);

		// 6. Calculate max price
		let max_price = math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);
		ensure!(
			math::ensure_max_price(buy_price, max_price),
			Error::<T>::MaxBuyPriceExceeded
		);

		// Check HSM has enough collateral
		let current_holding = <T as Config>::Currency::balance(collateral_asset, &Self::account_id());
		ensure!(
			current_holding >= final_collateral_amount,
			Error::<T>::InsufficientCollateralBalance
		);

		// Execute the swap
		// 1. Transfer hollar from user to HSM
		<T as Config>::Currency::transfer(
			T::HollarId::get(),
			who,
			&Self::account_id(),
			final_hollar_amount,
			Preservation::Expendable,
		)?;

		// 2. Transfer collateral from HSM to user
		<T as Config>::Currency::transfer(
			collateral_asset,
			&Self::account_id(),
			who,
			final_collateral_amount,
			Preservation::Expendable,
		)?;

		// 3. Burn Hollar by calling GHO contract
		Self::burn_hollar(final_hollar_amount)?;

		// 5. Update Hollar amount received in this block
		HollarAmountReceived::<T>::mutate(collateral_asset, |amount| {
			*amount = amount.saturating_add(final_hollar_amount);
		});

		Ok((final_hollar_amount, final_collateral_amount))
	}

	/// Processes Hollar going out from HSM in exchange for collateral coming in
	///
	/// This function handles:
	/// 1. Calculating the purchase price with fees
	/// 2. Ensuring maximum collateral holding isn't exceeded
	/// 3. Transferring collateral from user to HSM
	/// 4. Minting new Hollar to the user
	///
	/// Returns the Hollar and collateral amounts traded.
	#[require_transactional]
	fn do_trade_hollar_out(
		who: &T::AccountId,
		collateral_asset: T::AssetId,
		calculate_amounts: impl FnOnce(Price) -> Result<(Balance, Balance), DispatchError>,
	) -> Result<(Balance, Balance), DispatchError> {
		let collateral_info = Collaterals::<T>::get(collateral_asset).ok_or(Error::<T>::AssetNotApproved)?;
		let pool_state = Self::get_stablepool_state(collateral_info.pool_id)?;
		let collateral_pos = pool_state
			.asset_idx(collateral_asset)
			.ok_or(Error::<T>::AssetNotFound)?;
		let hollar_pos = pool_state
			.asset_idx(T::HollarId::get())
			.ok_or(Error::<T>::AssetNotFound)?;

		// If a pool does not have peg source set,
		// we need to correctly set the peg, respecting asset decimals
		let peg = if pool_state.has_peg_source_set::<T>(collateral_info.pool_id) {
			pool_state.pegs[collateral_pos]
		} else {
			let hollar_decimals = pool_state
				.asset_decimals_at(hollar_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			let collateral_decimals = pool_state
				.asset_decimals_at(collateral_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			(
				10u128.pow(hollar_decimals as u32),
				10u128.pow(collateral_decimals as u32),
			)
		};
		let purchase_price = math::calculate_purchase_price(peg, collateral_info.purchase_fee);

		log::trace!(target: "hsm", "Peg: {:?}, Purchase price {:?}", peg, purchase_price);

		let (hollar_amount, collateral_amount) = calculate_amounts(purchase_price)?;

		ensure!(
			Self::ensure_max_collateral_holding(collateral_asset, &collateral_info, collateral_amount),
			Error::<T>::MaxHoldingExceeded
		);

		<T as Config>::Currency::transfer(
			collateral_asset,
			who,
			&Self::account_id(),
			collateral_amount,
			Preservation::Expendable,
		)?;

		Self::mint_hollar(who, hollar_amount)?;

		Ok((hollar_amount, collateral_amount))
	}

	/// Retrieve hollar contract address
	fn get_hollar_contract_address() -> Result<EvmAddress, DispatchError> {
		if cfg!(feature = "runtime-benchmarks") {
			// for benchmarking purposes, we simply return some address
			// it is because we dont have Hollar registered in registry as Erc20,
			// but we still read the hollar from registry storage, to account for registry read weight
			let _ = T::GhoContractAddress::contract_address(T::HollarId::get());
			Ok(EvmAddress::from_slice(&hex!(
				"0101010101010101010101010101010101010101"
			)))
		} else {
			T::GhoContractAddress::contract_address(T::HollarId::get())
				.ok_or(Error::<T>::HollarContractAddressNotFound.into())
		}
	}

	/// Mint Hollar by calling the GHO token contract
	///
	/// Creates new Hollar tokens by interacting with the GHO ERC20 contract.
	/// The HSM pallet acts as the facilitator for minting.
	///
	/// Returns Ok if successful, or an error if the EVM interaction fails.
	fn mint_hollar(who: &T::AccountId, amount: Balance) -> DispatchResult {
		let contract = Self::get_hollar_contract_address()?;
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
		if exit_reason != ExitReason::Succeed(ExitSucceed::Stopped) {
			log::error!(target: "hsm", "Mint Hollar EVM execution failed - {:?}. Reason: {:?}", exit_reason, value);
			return Err(Error::<T>::InvalidEVMInteraction.into());
		}

		Ok(())
	}

	/// Burn Hollar by calling the GHO token contract
	///
	/// Destroys Hollar tokens by interacting with the GHO ERC20 contract.
	/// The HSM pallet acts as the facilitator for burning.
	///
	/// Returns Ok if successful, or an error if the EVM interaction fails.
	fn burn_hollar(amount: Balance) -> DispatchResult {
		let contract = Self::get_hollar_contract_address()?;
		let pallet_address = T::EvmAccounts::evm_address(&Self::account_id());

		// Create the context for the EVM call
		let context = CallContext::new_call(contract, pallet_address);

		// Encode the burn function call with amount
		let mut data = Into::<u32>::into(ERC20Function::Burn).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());

		// Execute the EVM call
		let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());

		// Check if the call was successful
		if exit_reason != ExitReason::Succeed(ExitSucceed::Stopped) {
			log::error!(target: "hsm", "Burn Hollar EVM execution failed. Reason: {:?}, value {:?}", exit_reason, value);
			return Err(Error::<T>::InvalidEVMInteraction.into());
		}

		Ok(())
	}

	/// Simulates selling an asset with exact input in a StableSwap pool
	///
	/// Calculates how much of asset_out would be received by selling a specific amount of asset_in.
	/// Uses the StableSwap pool mechanism with the provided pool state snapshot.
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

	/// Simulates buying an asset with exact output in a StableSwap pool
	///
	/// Calculates how much of asset_in would be required to buy a specific amount of asset_out.
	/// Uses the StableSwap pool mechanism with the provided pool state snapshot.
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
	///
	/// This function:
	/// 1. Acquires a lock to prevent concurrent execution
	/// 2. Checks each collateral asset for arbitrage opportunities
	/// 3. Submits unsigned transactions to execute profitable arbitrages
	///
	/// This is called from the offchain worker to maintain the Hollar peg.
	fn process_arbitrage_opportunities(block_number: BlockNumberFor<T>) -> Result<(), DispatchError> {
		let lock_expiration = Duration::from_millis(LOCK_TIMEOUT);
		let mut lock = StorageLock::<'_, Time>::with_deadline(OFFCHAIN_WORKER_LOCK, lock_expiration);

		let r = if let Ok(_guard) = lock.try_lock() {
			log::debug!(
				target: "hsm::offchain_worker",
				"Processing arbitrage opportunities at block: {:?}", block_number
			);

			for (asset_id, _) in <Collaterals<T>>::iter() {
				if <T as Config>::Currency::balance(asset_id, &Self::account_id()) > 0 {
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
	/// Determines if there's a profitable arbitrage between the HSM and StableSwap pool
	/// for a specific collateral asset.
	///
	/// Returns the amount of Hollar to trade if there's an opportunity, or 0 otherwise.
	/// Also returns errors if conditions prevent arbitrage execution.
	fn calculate_arbitrage_opportunity(
		collateral_asset_id: T::AssetId,
		collateral_info: &CollateralInfo<T::AssetId>,
	) -> Result<Balance, DispatchError> {
		let hollar_id = T::HollarId::get();
		let pool_id = collateral_info.pool_id;

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
		let collateral_reserve = pool_state
			.asset_reserve_at(collateral_pos)
			.ok_or(Error::<T>::AssetNotFound)?;
		let hollar_reserve = pool_state
			.asset_reserve_at(hollar_pos)
			.ok_or(Error::<T>::AssetNotFound)?;

		// If a pool does not have peg source set,
		// we need to correctly set the peg, respecting asset decimals
		let peg = if pool_state.has_peg_source_set::<T>(collateral_info.pool_id) {
			pool_state.pegs[collateral_pos]
		} else {
			let hollar_decimals = pool_state
				.asset_decimals_at(hollar_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			let collateral_decimals = pool_state
				.asset_decimals_at(collateral_pos)
				.ok_or(Error::<T>::DecimalRetrievalFailed)?;
			(
				10u128.pow(hollar_decimals as u32),
				10u128.pow(collateral_decimals as u32),
			)
		};

		// 1. Calculate imbalance
		let imbalance =
			math::calculate_imbalance(hollar_reserve, peg, collateral_reserve).ok_or(ArithmeticError::Overflow)?;
		ensure!(!imbalance.is_zero(), Error::<T>::NoArbitrageOpportunity);
		let b_coefficient = collateral_info.buyback_rate;
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

		let exec_price_ratio: hydra_dx_math::ratio::Ratio = execution_price.into();
		let fee_ratio: hydra_dx_math::ratio::Ratio = (fee_complement.deconstruct() as u128, 1_000_000u128).into();
		let buy_price_ratio = exec_price_ratio.saturating_div(&fee_ratio);
		let buy_price = (buy_price_ratio.n, buy_price_ratio.d);

		let max_price = math::calculate_max_buy_price(peg, collateral_info.max_buy_price_coefficient);

		// Check if price exceeds max price - compare the ratios
		ensure!(
			math::ensure_max_price(buy_price, max_price),
			Error::<T>::MaxBuyPriceExceeded
		);

		// Calculate the amount of Hollar to trade
		// max_buy_amt = min(max_buy_amt, self.liquidity[tkn] / buy_price)
		let asset_holding = <T as Config>::Currency::balance(collateral_asset_id, &Self::account_id());
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
