//!
#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

mod weights;

use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		fungible, Currency, Get, LockIdentifier, LockableCurrency, PollStatus, Polling, ReservableCurrency,
		WithdrawReasons,
	},
};
use frame_system::pallet_prelude::BlockNumberFor;
pub use pallet::*;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Saturating, StaticLookup, Zero},
	ArithmeticError, DispatchError, Perbill,
};
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::{DispatchResultWithPostInfo, IsType, StorageDoubleMap, StorageMap, ValueQuery},
		traits::ClassCountOf,
		Twox64Concat,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::BoundedVec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::submit_intent())]
		pub fn submit_intent(origin: OriginFor<T>) -> DispatchResult {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
