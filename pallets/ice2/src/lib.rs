#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;
mod tests;
mod types;
mod weights;

use frame_support::pallet_prelude::*;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use pallet_intent::types::ResolvedIntent;
use types::{Reason, Solution};
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use pallet_intent::types::BoundedResolvedIntents;
	use sp_runtime::traits::BlockNumberProvider;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	/// Temporary storage for the best solution, used to exclude worse solutions when tx is submitted.
	#[pallet::getter(fn solution_score)]
	pub(super) type SolutionScore<T: Config> = StorageValue<_, (T::AccountId, u64), OptionQuery>;

	#[pallet::storage]
	/// Flag that indicates if the solution was executed in current block.
	#[pallet::getter(fn solution_executed)]
	pub(super) type SolutionExecuted<T: Config> = StorageValue<_, bool, ValueQuery, ExecDefault>;

	// Default executed flag is false.
	pub struct ExecDefault;
	impl Get<bool> for ExecDefault {
		fn get() -> bool {
			false
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_intent::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution has been executed.
		Executed { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid block number
		InvalidBlockNumber,

		/// Solution already executed in this block
		AlreadyExecuted,

		/// Submitted solution is invalid due to the reason.
		InvalidSolution(Reason),
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
			intents: BoundedResolvedIntents,
			score: u64,
			valid_for_block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// check if the solution was already executed in this block
			// This is to prevent multiple solutions to be executed in the same block.
			// Although it should be handled by the tx validation, it is better to have it here too.
			// So we dont slash the user for the tx that should have been rejected.
			ensure!(!SolutionExecuted::<T>::get(), Error::<T>::AlreadyExecuted);

			// double-check the target block, although it should be done in the tx validation
			ensure!(
				valid_for_block == T::BlockNumberProvider::current_block_number(),
				Error::<T>::InvalidBlockNumber
			);

			// double-check again, should be done in tx validation
			ensure!(!intents.is_empty(), Error::<T>::InvalidSolution(Reason::Empty));

			let solution = Self::prepare_solution(&intents)?;

			if !Self::validate_solution_score(&solution, score) {
				return Err(Error::<T>::InvalidSolution(Reason::Score).into());
			}

			Self::execute_solution(solution)?;

			Self::deposit_event(Event::Executed { who });

			SolutionExecuted::<T>::set(true);
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {
			SolutionScore::<T>::kill();
			SolutionExecuted::<T>::kill();
		}
	}
}

// PALLET PUBLIC API
impl<T: Config> Pallet<T> {
	fn prepare_solution(resolved_intents: &[ResolvedIntent]) -> Result<Solution<T::AccountId>, DispatchError> {
		Ok(Solution::default())
	}

	fn validate_solution_score(solution: &Solution<T::AccountId>, score: u64) -> bool {
		true
	}

	#[require_transactional]
	fn execute_solution(solution: Solution<T::AccountId>) -> DispatchResult {
		Ok(())
	}
}
