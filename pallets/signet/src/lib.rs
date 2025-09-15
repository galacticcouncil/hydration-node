#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

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
        type WeightInfo: WeightInfo;
    }

    // ========================================
    // Storage
    // ========================================
    
    /// The admin account that controls this pallet
    #[pallet::storage]
    #[pallet::getter(fn admin)]
    pub type Admin<T: Config> = StorageValue<_, T::AccountId>;

    // Storage for signature deposit amount
    /// The amount required as deposit for signature requests
    #[pallet::storage]
    #[pallet::getter(fn signature_deposit)]
    pub type SignatureDeposit<T: Config> = StorageValue<_, u128, ValueQuery>;

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
            signature_deposit: u128,  // 🆕 NEW: Added deposit to event
        },
        
        // 🆕 NEW: Event for deposit updates
        /// Signature deposit amount has been updated
        DepositUpdated {
            old_deposit: u128,
            new_deposit: u128,
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
        // Error for unauthorized access
        /// Unauthorized - caller is not admin
        Unauthorized,
    }

    // ========================================
    // Extrinsics
    // ========================================
    
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        
        /// Initialize the pallet with an admin account and initial deposit
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::initialize())]
        pub fn initialize(
            origin: OriginFor<T>,
            admin: T::AccountId,
            signature_deposit: u128,
        ) -> DispatchResult {
            // Only root (sudo) can initialize
            ensure_root(origin)?;
            
            // Make sure we haven't initialized already
            ensure!(Admin::<T>::get().is_none(), Error::<T>::AlreadyInitialized);
            
            // Store the admin
            Admin::<T>::put(&admin);
            
            SignatureDeposit::<T>::put(signature_deposit);
            
            // Emit event (updated to include deposit)
            Self::deposit_event(Event::Initialized { 
                admin,
                signature_deposit,  // 🆕 NEW
            });
            
            Ok(())
        }
        
        // Function to update deposit (admin only)
        /// Update the signature deposit amount (admin only)
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::update_deposit())]
        pub fn update_deposit(
            origin: OriginFor<T>,
            new_deposit: u128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Check that pallet is initialized and get admin
            let admin = Admin::<T>::get().ok_or(Error::<T>::NotInitialized)?;
            
            // Check that caller is admin
            ensure!(who == admin, Error::<T>::Unauthorized);
            
            // Get old deposit for event
            let old_deposit = SignatureDeposit::<T>::get();
            
            // Update the deposit
            SignatureDeposit::<T>::put(new_deposit);
            
            // Emit event
            Self::deposit_event(Event::DepositUpdated {
                old_deposit,
                new_deposit,
            });
            
            Ok(())
        }
        
        /// Emit a custom event with data
        #[pallet::call_index(2)]
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