#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement},
    PalletId,
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec::Vec;

pub use pallet::*;

// Type alias for cleaner code
type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
        
        /// Currency for handling deposits and fees
        type Currency: Currency<Self::AccountId>;
        
        /// The pallet's unique ID for deriving its account
        #[pallet::constant]
        type PalletId: Get<PalletId>;
        
        type WeightInfo: WeightInfo;
    }

    // ========================================
    // Storage
    // ========================================
    
    /// The admin account that controls this pallet
    #[pallet::storage]
    #[pallet::getter(fn admin)]
    pub type Admin<T: Config> = StorageValue<_, T::AccountId>;

    /// The amount required as deposit for signature requests
    #[pallet::storage]
    #[pallet::getter(fn signature_deposit)]
    pub type SignatureDeposit<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

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
            signature_deposit: BalanceOf<T>,
        },
        
        /// Signature deposit amount has been updated
        DepositUpdated {
            old_deposit: BalanceOf<T>,
            new_deposit: BalanceOf<T>,
        },
        
        /// Funds have been withdrawn from the pallet
        FundsWithdrawn {
            amount: BalanceOf<T>,
            recipient: T::AccountId,
        },
        
        // Event for signature requests
        /// A signature has been requested
        SignatureRequested {
            sender: T::AccountId,
            payload: [u8; 32],
            key_version: u32,
            deposit: BalanceOf<T>,
            path: Vec<u8>,
            algo: Vec<u8>,
            dest: Vec<u8>,
            params: Vec<u8>,
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
        /// Unauthorized - caller is not admin
        Unauthorized,
        /// Insufficient funds for withdrawal
        InsufficientFunds,
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
            signature_deposit: BalanceOf<T>,
        ) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(Admin::<T>::get().is_none(), Error::<T>::AlreadyInitialized);
            
            Admin::<T>::put(&admin);
            SignatureDeposit::<T>::put(signature_deposit);
            
            Self::deposit_event(Event::Initialized { 
                admin,
                signature_deposit,
            });
            
            Ok(())
        }
        
        /// Update the signature deposit amount (admin only)
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::update_deposit())]
        pub fn update_deposit(
            origin: OriginFor<T>,
            new_deposit: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let admin = Admin::<T>::get().ok_or(Error::<T>::NotInitialized)?;
            ensure!(who == admin, Error::<T>::Unauthorized);
            
            let old_deposit = SignatureDeposit::<T>::get();
            SignatureDeposit::<T>::put(new_deposit);
            
            Self::deposit_event(Event::DepositUpdated {
                old_deposit,
                new_deposit,
            });
            
            Ok(())
        }
        
        /// Withdraw funds from the pallet account (admin only)
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::withdraw_funds())]
        pub fn withdraw_funds(
            origin: OriginFor<T>,
            recipient: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let admin = Admin::<T>::get().ok_or(Error::<T>::NotInitialized)?;
            ensure!(who == admin, Error::<T>::Unauthorized);
            
            let pallet_account = Self::account_id();
            let pallet_balance = T::Currency::free_balance(&pallet_account);
            ensure!(pallet_balance >= amount, Error::<T>::InsufficientFunds);
            
            T::Currency::transfer(
                &pallet_account,
                &recipient,
                amount,
                ExistenceRequirement::AllowDeath,
            )?;
            
            Self::deposit_event(Event::FundsWithdrawn {
                amount,
                recipient,
            });
            
            Ok(())
        }
        
        // Sign function
        /// Request a signature for a payload
        #[pallet::call_index(3)]
        #[pallet::weight(<T as Config>::WeightInfo::sign())]
        pub fn sign(
            origin: OriginFor<T>,
            payload: [u8; 32],
            key_version: u32,
            path: Vec<u8>,
            algo: Vec<u8>,
            dest: Vec<u8>,
            params: Vec<u8>,
        ) -> DispatchResult {
            let requester = ensure_signed(origin)?;
            
            // Ensure initialized
            ensure!(Admin::<T>::get().is_some(), Error::<T>::NotInitialized);
            
            // Get deposit amount
            let deposit = SignatureDeposit::<T>::get();
            
            // Transfer deposit from requester to pallet account
            let pallet_account = Self::account_id();
            T::Currency::transfer(
                &requester,
                &pallet_account,
                deposit,
                ExistenceRequirement::KeepAlive,
            )?;
            
            // Emit event
            Self::deposit_event(Event::SignatureRequested {
                sender: requester,
                payload,
                key_version,
                deposit,
                path,
                algo,
                dest,
                params,
            });
            
            Ok(())
        }
        
        /// Emit a custom event with data
        #[pallet::call_index(4)]
        #[pallet::weight(<T as Config>::WeightInfo::emit_custom_event())]
        pub fn emit_custom_event(
            origin: OriginFor<T>,
            message: Vec<u8>,
            value: u128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
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
    
    // Helper functions
    impl<T: Config> Pallet<T> {
        /// Get the pallet's account ID (where funds are stored)
        pub fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }
    }
}