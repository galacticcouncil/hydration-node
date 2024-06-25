//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
mod weights;

use crate::types::{CallData, IncrementalIntentId, IntentId, Moment};
use frame_support::{dispatch::DispatchResult, traits::Get};
pub use pallet::*;
use sp_runtime::DispatchError;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::types::{IncrementalIntentId, Intent, IntentId, Moment, Swap};
	use codec::{HasCompact, MaxEncodedLen};
	use frame_support::pallet_prelude::StorageValue;
	use frame_support::traits::Time;
	use frame_support::{
		pallet_prelude::{IsType, StorageMap, ValueQuery},
		Blake2_128Concat, Parameter,
	};
	use frame_system::pallet_prelude::*;
	use scale_info::TypeInfo;
	use sp_runtime::traits::{MaybeSerializeDeserialize, Member};

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

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		#[pallet::constant]
		type MaxCallData: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted
		IntentSubmitted(IntentId, Intent<T::AccountId, T::AssetId>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// No more intent ids available
		IntendIdsExhausted,

		/// Data too long
		TooLong,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId, T::AssetId>>;

	#[pallet::storage]
	/// Intent id sequencer
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

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
			// - swap is valid- eg no lrna buying?!

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

		#[crate::pallet::call_index(0)]
		#[crate::pallet::weight(T::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub(crate) fn get_intent_id(deadline: Moment, increment: IncrementalIntentId) -> IntentId {
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
