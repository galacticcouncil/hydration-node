//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod engine;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;
pub mod validity;
mod weights;

use crate::types::{IncrementalIntentId, Intent, IntentId, Moment, NamedReserveIdentifier};
use codec::{HasCompact, MaxEncodedLen};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::traits::Time;
use frame_support::{dispatch::DispatchResult, traits::Get};
use frame_support::{Blake2_128Concat, Parameter};
use frame_system::pallet_prelude::*;
use hydradx_traits::router::RouterT;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::traits::{MaybeSerializeDeserialize, Member, Zero};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::engine::ICEEngine;
	use crate::traits::IceWeightBounds;
	use crate::types::{BoundedResolvedIntents, BoundedTrades, TradeInstruction};
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;
	use hydra_dx_math::ratio::Ratio;
	use hydradx_traits::price::PriceProvider;
	use sp_runtime::traits::BlockNumberProvider;
	use types::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ Ord
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Native asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<Self::AssetId>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// TODO: this two currencies could be merged into one, however it would need to implement support in the runtime for this
		type Currency: Mutate<Self::AccountId, AssetId = Self::AssetId, Balance = types::Balance>;

		type ReservableCurrency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::AssetId,
			Balance = Balance,
		>;

		type TradeExecutor: RouterT<
			Self::RuntimeOrigin,
			Self::AssetId,
			Balance,
			hydradx_traits::router::Trade<Self::AssetId>,
			hydradx_traits::router::AmountInAndOut<Balance>,
		>;

		/// The means of determining a solution's weight.
		type Weigher: IceWeightBounds<Self::RuntimeCall, Vec<hydradx_traits::router::Trade<Self::AssetId>>>;

		/// Price provider
		type PriceProvider: PriceProvider<Self::AssetId, Price = Ratio>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type MaxCallData: Get<u32>;

		/// The bond required to propose a new solution.
		#[pallet::constant]
		type ProposalBond: Get<Balance>;

		/// The account which receives slashed bonds in case of invalid solution.
		#[pallet::constant]
		type SlashReceiver: Get<Self::AccountId>;

		#[pallet::constant]
		type NamedReserveId: Get<NamedReserveIdentifier>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted
		IntentSubmitted(IntentId, Intent<T::AccountId, T::AssetId>),

		/// Solution was executed
		SolutionExecuted { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No more intent ids available
		IntendIdsExhausted,

		/// Data too long
		TooLong,

		/// Intent not found
		IntentNotFound,

		/// Price is missing in provided solution
		MissingPrice,

		/// Invalid block number
		InvalidBlockNumber,

		/// Invalid deadline
		InvalidDeadline,

		/// Insufficient reserved balance
		InsufficientReservedBalance,

		/// Invalid solution score
		InvalidScore,

		///
		IncorrectIntentAmountResolution,

		///
		InvalidTransferInstruction,

		///
		IntentLimitPriceViolation,

		/// Solution already executed in this block
		AlreadyExecuted,

		/// Invalid intent parameters
		InvalidIntent,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId, T::AssetId>>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::storage]
	/// Temporay storage for the best solution, used to exclude worse solutions when tx are submitted.
	#[pallet::getter(fn solution_score)]
	pub(super) type SolutionScore<T: Config> = StorageValue<_, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	/// Flag that indicates if the solution was executed in current block.
	#[pallet::getter(fn solution_executed)]
	pub(super) type SolutionExecuted<T: Config> = StorageValue<_, bool, ValueQuery, ExecDefault>;

	pub struct ExecDefault;

	impl Get<bool> for ExecDefault {
		fn get() -> bool {
			false
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			SolutionScore::<T>::kill();
			SolutionExecuted::<T>::kill();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent<T::AccountId, T::AssetId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let now = T::TimestampProvider::now();
			ensure!(intent.deadline > now, Error::<T>::InvalidDeadline);
			ensure!(
				intent.deadline < (now.saturating_add(T::MaxAllowedIntentDuration::get())),
				Error::<T>::InvalidDeadline
			);

			ensure!(intent.swap.amount_in > Balance::zero(), Error::<T>::InvalidIntent);
			ensure!(intent.swap.amount_out > Balance::zero(), Error::<T>::InvalidIntent);
			ensure!(intent.swap.asset_in != intent.swap.asset_out, Error::<T>::InvalidIntent);
			ensure!(intent.swap.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);

			T::ReservableCurrency::reserve_named(
				&T::NamedReserveId::get(),
				intent.swap.asset_in,
				&who,
				intent.swap.amount_in,
			)?;

			let incremental_id = Self::get_next_incremental_id().ok_or(Error::<T>::IntendIdsExhausted)?;
			let intent_id = Self::get_intent_id(intent.deadline, incremental_id);

			Intents::<T>::insert(intent_id, &intent);

			Self::deposit_event(Event::IntentSubmitted(intent_id, intent));

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight( {
			let mut w = T::WeightInfo::submit_solution();
			let intent_count = intents.len() as u64;
			let transfer_weight = T::Weigher::transfer_weight() * intent_count * 2; // transfer in and out
			w.saturating_accrue(transfer_weight);
			for instruction in trades.iter() {
				match instruction {
					TradeInstruction::SwapExactIn { route, .. } => {
						w.saturating_accrue(T::Weigher::sell_weight(route.to_vec()));
					},
					TradeInstruction::SwapExactOut { route, .. } => {
						w.saturating_accrue(T::Weigher::buy_weight(route.to_vec()));
					}
				}
			}
			w
		})]
		pub fn submit_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
			trades: BoundedTrades<T::AssetId>,
			score: u64,
			block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// check if the solution was already executed in this block
			// This is to prevent multiple solutions to be executed in the same block.
			// Although it should be handled by the tx validation, it is better to have it here too.
			// So we dont slash the user for the tx that should have been rejected.
			ensure!(!SolutionExecuted::<T>::get(), Error::<T>::AlreadyExecuted);

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			match ICEEngine::<T>::prepare_solution(intents, trades, score) {
				Ok(solution) => {
					ICEEngine::<T>::execute_solution(solution)?;
					Self::clear_expired_intents();
					Self::deposit_event(Event::SolutionExecuted { who });
					SolutionExecuted::<T>::set(true);
				}
				Err(e) => {
					//TODO: this does not actually work here, because in case of error, there cannot be storage update no more.
					T::Currency::transfer(
						T::NativeAssetId::get(),
						&who,
						&T::SlashReceiver::get(),
						T::ProposalBond::get(),
						Preservation::Expendable,
					)?;
					return Err(e);
				}
			}
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Holding account
	pub fn holding_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn get_intent_id(deadline: Moment, increment: IncrementalIntentId) -> IntentId {
		(deadline as u128) << 64 | increment as u128
	}

	pub(crate) fn get_next_incremental_id() -> Option<IncrementalIntentId> {
		NextIncrementalId::<T>::mutate(|id| -> Option<IncrementalIntentId> {
			let current_id = *id;
			*id = id.checked_add(1)?;
			Some(current_id)
		})
	}

	pub fn validate_submission(who: &T::AccountId, score: u64, block: BlockNumberFor<T>) -> bool {
		if block != T::BlockNumberProvider::current_block_number() {
			return false;
		}

		if let Some((from, current_score)) = SolutionScore::<T>::get() {
			if score > current_score {
				SolutionScore::<T>::put((who, score));
			}
			if from == *who {
				true
			} else {
				score > current_score
			}
		} else {
			SolutionScore::<T>::put((who, score));
			true
		}
	}

	fn clear_expired_intents() {
		//TODO: perhaps better way to do this is to use a priority queue/ordered list or something.
		let now = T::TimestampProvider::now();
		let mut to_remove = Vec::new();
		for (intent_id, intent) in Intents::<T>::iter() {
			if intent.deadline < now {
				to_remove.push((intent_id, intent));
			}
		}

		for (intent_id, intent) in to_remove {
			let remainder = T::ReservableCurrency::unreserve_named(
				&T::NamedReserveId::get(),
				intent.swap.asset_in,
				&intent.who,
				intent.swap.amount_in,
			); //TODO: add test
			debug_assert!(remainder.is_zero());
			Intents::<T>::remove(intent_id);
		}
	}

	fn ensure_proposal_bond(who: &T::AccountId) -> bool {
		let required_bond = T::ProposalBond::get();
		let balance =
			T::Currency::reducible_balance(T::NativeAssetId::get(), who, Preservation::Protect, Fortitude::Force); //TODO: check params
		balance > required_bond
	}
}
