//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod engine;
#[cfg(test)]
mod tests;
mod traits;
pub mod types;
pub mod validity;
mod weights;

use crate::types::{
	CallData, IncrementalIntentId, Intent, IntentId, Moment, NamedReserveIdentifier, ProposedSolution, Solution, Swap,
};
use codec::{HasCompact, MaxEncodedLen};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::{dispatch::DispatchResult, traits::Get};
use frame_support::{Blake2_128Concat, Parameter};
use frame_system::pallet_prelude::*;
use hydradx_traits::router::RouterT;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use sp_runtime::DispatchError;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::engine::ICEEngine;
	use crate::traits::IceWeightBounds;
	use crate::types::Instruction;
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;
	use hydra_dx_math::ratio::Ratio;
	use hydradx_traits::price::PriceProvider;
	use orml_traits::NamedMultiReservableCurrency;
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

		/// Asset Id of hub asset
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
		type Weigher: traits::IceWeightBounds<Self::RuntimeCall>;

		/// Price provider
		type PriceProvider: PriceProvider<Self::AssetId, Price = Ratio>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type MaxCallData: Get<u32>;

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
			SolutionScore::<T>::kill(); //TODO: add test for this
			SolutionExecuted::<T>::kill(); //TODO: add test for this
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())]
		pub fn submit_intent(
			origin: OriginFor<T>,
			swap: Swap<T::AssetId>,
			deadline: Moment,
			partial: bool,
			on_success: Option<Vec<u8>>,
			on_failure: Option<Vec<u8>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let now = T::TimestampProvider::now();
			ensure!(deadline > now, Error::<T>::InvalidDeadline);
			ensure!(
				deadline < (now.saturating_add(T::MaxAllowedIntentDuration::get())),
				Error::<T>::InvalidDeadline
			);

			//TODO: additional checks:
			// - no lrna buying
			// - asset in != asset out

			T::ReservableCurrency::reserve_named(&T::NamedReserveId::get(), swap.asset_in, &who, swap.amount_in)?;

			let incremental_id = Self::get_next_incremental_id().ok_or(Error::<T>::IntendIdsExhausted)?;

			let on_success = Self::try_into_call_data(on_success)?;
			let on_failure = Self::try_into_call_data(on_failure)?;

			let intent = Intent {
				who,
				swap,
				deadline,
				partial,
				on_success,
				on_failure,
			};

			let intent_id = Self::get_intent_id(deadline, incremental_id);

			Intents::<T>::insert(intent_id, &intent);

			Self::deposit_event(Event::IntentSubmitted(intent_id, intent));

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight( {
			let mut w = T::WeightInfo::submit_solution();
			for instruction in solution.instructions.iter() {
				match instruction {
					Instruction::TransferIn { .. } => {
						w.saturating_accrue(T::Weigher::transfer_weight());
					},
					Instruction::TransferOut { .. } => {
						w.saturating_accrue(T::Weigher::transfer_weight());
					},
					Instruction::SwapExactIn { .. } => {
						w.saturating_accrue(T::Weigher::swap_weight());
					},
					Instruction::SwapExactOut { .. } => {
						w.saturating_accrue(T::Weigher::swap_weight());
					}
				}
			}
			w
		})]
		pub fn submit_solution(
			origin: OriginFor<T>,
			solution: ProposedSolution<T::AccountId, T::AssetId>,
			score: u64,
			block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			let solution = Solution {
				proposer: who.clone(),
				intents: solution.intents,
				instructions: solution.instructions,
				score,
			};

			if let Err(e) = ICEEngine::<T>::validate_solution(&solution) {
				//TODO: slash him, bob!
				return Err(e);
			}

			ICEEngine::<T>::execute_solution(solution)?;

			Self::clear_expired_intents();

			Self::deposit_event(Event::SolutionExecuted { who });

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

	pub(crate) fn try_into_call_data(v: Option<Vec<u8>>) -> Result<Option<CallData>, DispatchError> {
		let Some(data) = v else {
			return Ok(None);
		};
		CallData::try_from(data)
			.map_err(|_| Error::<T>::TooLong.into())
			.map(|v| Some(v))
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
				to_remove.push(intent_id);
			}
		}

		for intent_id in to_remove {
			Intents::<T>::remove(intent_id);
		}
	}
}
