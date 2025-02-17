#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;
pub mod types;
mod weights;

use crate::types::{AssetId, Balance, IncrementalIntentId, Intent, IntentId, Moment, NamedReserveIdentifier};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::Blake2_128Concat;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::pallet_prelude::*;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use sp_runtime::traits::AccountIdConversion;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::fungibles::Mutate;
	use frame_support::PalletId;
	use sp_runtime::traits::BlockNumberProvider;
	use types::Balance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<AssetId>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// TODO: this two currencies could be merged into one, however it would need to implement support in the runtime for this
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = types::Balance>;

		type ReservableCurrency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = AssetId,
			Balance = Balance,
		>;

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
		IntentSubmitted(IntentId, Intent<T::AccountId>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Data too long
		TooLong,

		/// Invalid block number
		InvalidBlockNumber,

		/// Invalid deadline
		InvalidDeadline,

		/// Insufficient reserved balance
		InsufficientReservedBalance,

		/// Invalid intent parameters
		InvalidIntent,

		/// Incorrect intent update
		InvalidIntentUpdate,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent<T::AccountId>>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent<T::AccountId>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(who == intent.who, Error::<T>::InvalidIntent);
			Self::add_intent(intent)?;
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

// PALLET PUBLIC API
impl<T: Config> Pallet<T> {
	/// Holding account
	pub fn holding_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn get_intent_id(deadline: Moment, increment: IncrementalIntentId) -> IntentId {
		(deadline as u128) << 64 | increment as u128
	}

	#[require_transactional]
	pub fn add_intent(intent: Intent<T::AccountId>) -> Result<IntentId, DispatchError> {
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
			&intent.who,
			intent.swap.amount_in,
		)?;

		let incremental_id = Self::get_next_incremental_id();
		let intent_id = Self::get_intent_id(intent.deadline, incremental_id);

		Intents::<T>::insert(intent_id, &intent);

		Self::deposit_event(Event::IntentSubmitted(intent_id, intent));
		Ok(intent_id)
	}

	pub fn get_valid_intents() -> Vec<(IntentId, Intent<T::AccountId>)> {
		let mut intents: Vec<(IntentId, Intent<T::AccountId>)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline > now);

		intents
	}
}

impl<T: Config> Pallet<T> {
	pub(crate) fn get_next_incremental_id() -> IncrementalIntentId {
		// We deliberately overflow here, so if we , for some reason, hit to max value, we will start from 0 again
		// it is not an issue, we create new intent id together with deadline, so it is not possible to create two intents with the same id
		NextIncrementalId::<T>::mutate(|id| -> IncrementalIntentId {
			let current_id = *id;
			(*id, _) = id.overflowing_add(1);
			current_id
		})
	}
}
