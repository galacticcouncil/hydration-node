//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod engine;
#[cfg(test)]
mod tests;
pub mod types;
mod weights;

use crate::types::{
	BoundedPrices, BoundedResolvedIntents, CallData, IncrementalIntentId, Intent, IntentId, Moment, Price,
	ResolvedIntent, Solution, Swap,
};
use codec::{Encode, HasCompact, MaxEncodedLen};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::{dispatch::DispatchResult, traits::Get};
use frame_support::{Blake2_128Concat, Parameter};
use frame_system::pallet_prelude::*;
use hydradx_traits::router::RouterT;
pub use pallet::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{AccountIdConversion, Hash};
use sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use sp_runtime::DispatchError;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::engine::{ExecutionPlan, OmniXEngine};
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;

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

		///
		type Currency: Mutate<Self::AccountId, AssetId = Self::AssetId, Balance = types::Balance>;

		type TradeExecutor: RouterT<
			Self::RuntimeOrigin,
			Self::AssetId,
			crate::types::Balance,
			hydradx_traits::router::Trade<Self::AssetId>,
			hydradx_traits::router::AmountInAndOut<crate::types::Balance>,
		>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type MaxCallData: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution was noted
		SolutionNoted { proposer: T::AccountId, hash: T::Hash },

		/// New intent was submitted
		IntentSubmitted(IntentId, Intent<T::AccountId, T::AssetId>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No more intent ids available
		IntendIdsExhausted,

		/// Data too long
		TooLong,

		/// Provided solution is invalid
		InvalidSolution,

		/// Intent not found
		IntentNotFound,

		/// Solution not found
		SolutionNotFound,

		/// Price is missing in provided solution
		MissingPrice,

		/// Execution contains too many instructions
		TooManyInstructions,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId, T::AssetId>>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_solution)]
	pub(super) type Solutions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, ExecutionPlan<T::AccountId, T::AssetId>>;

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

			//TODO: check:
			// - deadline is in the future, not too far in the future
			// - swap is valid- eg no lrna buying?! asset in!= asset out etc.

			//TODO: reserve IN amount

			let incremental_id = Self::get_next_incrementat_id().ok_or(Error::<T>::IntendIdsExhausted)?;

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
		#[pallet::weight(T::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
			resolved_intents: Vec<ResolvedIntent>,
			sell_prices: Vec<(T::AssetId, Price)>,
			buy_prices: Vec<(T::AssetId, Price)>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let intents =
				BoundedResolvedIntents::try_from(resolved_intents).map_err(|_| crate::pallet::Error::<T>::TooLong)?;
			let buy_prices = BoundedPrices::try_from(buy_prices).map_err(|_| crate::pallet::Error::<T>::TooLong)?;
			let sell_prices = BoundedPrices::try_from(sell_prices).map_err(|_| crate::pallet::Error::<T>::TooLong)?;

			let solution = Solution {
				proposer: who.clone(),
				intents,
				sell_prices,
				buy_prices,
			};

			let hash = T::Hashing::hash(&solution.encode());

			let plan = OmniXEngine::<T, T::Currency, T::TradeExecutor>::prepare_execution_plan(&solution)?;

			Solutions::<T>::insert(&hash, plan);

			Self::deposit_event(Event::SolutionNoted { proposer: who, hash });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::execute_solution())]
		pub fn execute_solution(origin: OriginFor<T>, hash: T::Hash) -> DispatchResult {
			ensure_signed(origin)?;
			let exec_plan = Solutions::<T>::get(&hash).ok_or(Error::<T>::SolutionNotFound)?;
			OmniXEngine::<T, T::Currency, T::TradeExecutor>::execute_solution(exec_plan)?;
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

	pub(crate) fn get_next_incrementat_id() -> Option<IncrementalIntentId> {
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
}
