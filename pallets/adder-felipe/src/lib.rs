#![cfg_attr(not(feature = "std"), no_std)]

// Every pallet uses the `pub use pallet::*` pattern
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_std::vec::Vec;
    
    /// Configure the pallet by specifying parameters and types it depends on
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// Maximum value that can be added (for safety)
        #[pallet::constant]
        type MaxValue: Get<u128>;
    }
    
    /// The pallet's storage items
    #[pallet::storage]
    #[pallet::getter(fn last_result)]
    pub type LastResult<T> = StorageValue<_, u128, ValueQuery>;
    
    /// Storage for results by account
    #[pallet::storage]
    #[pallet::getter(fn user_results)]
    pub type UserResults<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        u128,
        ValueQuery
    >;
    
    /// Pallets use events to inform users about important changes
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Addition performed successfully [who, x, y, result]
        AdditionPerformed {
            who: T::AccountId,
            x: u128,
            y: u128,
            result: u128,
        },
    }
    
    /// Errors that can occur in the pallet
    #[pallet::error]
    pub enum Error<T> {
        /// Arithmetic overflow occurred
        Overflow,
        /// Value exceeds maximum allowed
        ValueTooLarge,
    }
    
    /// The pallet struct
    #[pallet::pallet]
    pub struct Pallet<T>(_);
    
    /// Dispatchable functions (extrinsics) - these can be called from outside
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Add two numbers together
        /// 
        /// This function adds x and y, stores the result, and emits an event
        #[pallet::call_index(0)]
        #[pallet::weight(Weight::from_parts(10_000, 0))]
        pub fn add(
            origin: OriginFor<T>,
            x: u128,
            y: u128,
        ) -> DispatchResult {
            // Ensure the call is from a signed account
            let who = ensure_signed(origin)?;
            
            // Check values don't exceed maximum
            ensure!(x <= T::MaxValue::get(), Error::<T>::ValueTooLarge);
            ensure!(y <= T::MaxValue::get(), Error::<T>::ValueTooLarge);
            
            // Perform addition with overflow check
            let result = x.checked_add(y)
                .ok_or(Error::<T>::Overflow)?;
            
            // Store the result
            <LastResult<T>>::put(result);
            <UserResults<T>>::insert(&who, result);
            
            // Emit an event
            Self::deposit_event(Event::AdditionPerformed {
                who,
                x,
                y,
                result,
            });
            
            Ok(())
        }
        
        /// Add multiple numbers (demonstrating more complex logic)
        #[pallet::call_index(1)]
        #[pallet::weight(Weight::from_parts(10_000 * numbers.len() as u64, 0))]
        pub fn add_multiple(
            origin: OriginFor<T>,
            numbers: Vec<u128>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            // Use internal helper function
            let result = Self::calculate_sum(&numbers)?;
            
            <LastResult<T>>::put(result);
            <UserResults<T>>::insert(&who, result);
            
            Self::deposit_event(Event::AdditionPerformed {
                who,
                x: 0, // You might emit different event here
                y: 0,
                result,
            });
            
            Ok(())
        }
    }
    
    /// Helper functions (not dispatchable)
    impl<T: Config> Pallet<T> {
        /// Internal function to calculate sum
        pub fn calculate_sum(numbers: &[u128]) -> Result<u128, Error<T>> {
            numbers.iter().try_fold(0u128, |acc, &num| {
                ensure!(num <= T::MaxValue::get(), Error::<T>::ValueTooLarge);
                acc.checked_add(num).ok_or(Error::<T>::Overflow)
            })
        }
        
        /// Public read-only function (can be called without transaction)
        pub fn get_sum_without_storing(x: u128, y: u128) -> Option<u128> {
            x.checked_add(y)
        }
    }
}