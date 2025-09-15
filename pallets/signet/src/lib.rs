#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

// For testing
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// Weight information for extrinsics
        type WeightInfo: WeightInfo;
    }

    // ========================================
    // Storage
    // ========================================
    
    /// The admin account that controls this pallet
    #[pallet::storage]
    #[pallet::getter(fn admin)]
    pub type Admin<T: Config> = StorageValue<_, T::AccountId>;

    // ========================================
    // Events
    // ========================================
    
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Custom data was emitted
        DataEmitted {
            who: T::AccountId,
            message: BoundedVec<u8, ConstU32<256>>,
            value: u128,
        },
        
        /// Pallet has been initialized with an admin
        Initialized {
            admin: T::AccountId,
        },
    }

    // ========================================
    // Errors
    // ========================================
    
    #[pallet::error]
    pub enum Error<T> {
        /// The provided message exceeds the maximum length of 256 bytes
        MessageTooLong,
        /// The pallet has already been initialized
        AlreadyInitialized,
        /// The pallet has not been initialized yet
        NotInitialized,
    }

    // ========================================
    // Extrinsics
    // ========================================
    
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        
        /// Initialize the pallet with an admin account
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::initialize())]
        pub fn initialize(
            origin: OriginFor<T>,
            admin: T::AccountId,
        ) -> DispatchResult {
            // Only root (sudo) can initialize
            ensure_root(origin)?;
            
            // Make sure we haven't initialized already
            ensure!(Admin::<T>::get().is_none(), Error::<T>::AlreadyInitialized);
            
            // Store the admin
            Admin::<T>::put(&admin);
            
            // Emit event
            Self::deposit_event(Event::Initialized { admin });
            
            Ok(())
        }
        
        /// Emit a custom event with data
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::emit_custom_event())]
        pub fn emit_custom_event(
            origin: OriginFor<T>,
            message: Vec<u8>,
            value: u128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Check that pallet is initialized
            ensure!(Admin::<T>::get().is_some(), Error::<T>::NotInitialized);
            
            let bounded_message = BoundedVec::<u8, ConstU32<256>>::try_from(message)
                .map_err(|_| Error::<T>::MessageTooLong)?;
            
            Self::deposit_event(Event::DataEmitted {
                who,
                message: bounded_message,
                value,
            });
            
            Ok(())
        }
    }
}