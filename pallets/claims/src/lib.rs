// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_wraps)]

use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	sp_runtime::{
		traits::{DispatchInfoOf, SignedExtension},
		transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction},
	},
	traits::{Currency, Get, Imbalance, IsSubType, VestingSchedule},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use primitives::Balance;
use sp_runtime::traits::Zero;
use sp_std::{marker::PhantomData, prelude::*, vec::Vec};
pub use traits::*;
use weights::WeightInfo;

mod benchmarking;
mod claims_data;
mod migration;
mod traits;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type BalanceOf<T> = <CurrencyOf<T> as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type CurrencyOf<T> =
	<<T as Config>::VestingSchedule as VestingSchedule<<T as frame_system::Config>::AccountId>>::Currency;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			migration::import_initial_claims::<T>(&claims_data::CLAIMS_DATA)
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Prefix: Get<&'static [u8]>;
		type WeightInfo: WeightInfo;
		type Currency: Currency<Self::AccountId>;
		type VestingSchedule: VestingSchedule<Self::AccountId, Moment = Self::BlockNumber, Currency = Self::Currency>;
		// This type is needed to convert from Currency to Balance
		type CurrencyBalance: From<Balance>
			+ Into<<Self::Currency as Currency<<Self as frame_system::Config>::AccountId>>::Balance>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Claim(T::AccountId, EthereumAddress, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Ethereum signature is not valid
		InvalidEthereumSignature,
		/// Claim is not valid
		NoClaimOrAlreadyClaimed,
		/// Value reached maximum and cannot be incremented further
		BalanceOverflow,
	}

	/// Asset id storage for each shared token
	#[pallet::storage]
	#[pallet::getter(fn claims)]
	pub type Claims<T: Config> = StorageMap<_, Blake2_128Concat, EthereumAddress, BalanceOf<T>, ValueQuery>;

	/// Vesting schedule for a claim.
	/// The balance is how much should be unlocked per block.
	/// The block number is when the vesting should start.
	#[pallet::storage]
	#[pallet::getter(fn vesting)]
	pub type Vesting<T: Config> = StorageMap<_, Identity, EthereumAddress, (BalanceOf<T>, T::BlockNumber), ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub claims: Vec<(EthereumAddress, BalanceOf<T>)>,
		pub vesting: Vec<(EthereumAddress, (BalanceOf<T>, T::BlockNumber))>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				claims: vec![],
				vesting: vec![],
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.claims.iter().for_each(|(eth_address, initial_balance)| {
				Claims::<T>::mutate(eth_address, |amount| *amount += *initial_balance)
			});
			self.vesting.iter().for_each(|(k, v)| {
				Vesting::<T>::insert(k, v);
			});
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claim xHDX by providing signed message with Ethereum address.
		#[pallet::weight((<T as Config>::WeightInfo::claim(), DispatchClass::Normal, Pays::No))]
		pub fn claim(origin: OriginFor<T>, ethereum_signature: EcdsaSignature) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let (balance_due, address) = Self::validate_claim(&sender, &ethereum_signature)?;
			let vesting_check = Vesting::<T>::contains_key(&address);

			if vesting_check {
				let mut vesting = Vesting::<T>::get(&address);
				// Start vesting right away
				let start_vesting: T::BlockNumber = <frame_system::Pallet<T>>::block_number();
				vesting.1 = start_vesting;
				Self::process_vested_claim(sender, balance_due, address, vesting)?;
			} else {
				Self::process_claim(sender, balance_due, address)?;
			}
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Check if a claim is valid.
	///
	/// Recovers Ethereum address from a message signature and checks whether such address
	/// can make a valid claims and has not been already claimed.
	fn validate_claim(
		who: &T::AccountId,
		signature: &EcdsaSignature,
	) -> Result<(BalanceOf<T>, EthereumAddress), Error<T>> {
		let sender_hex = who.using_encoded(to_ascii_hex);

		let signer = signature.recover(&sender_hex, T::Prefix::get());

		match signer {
			Some(address) => {
				let balance_due = Claims::<T>::get(&address);

				if balance_due == Zero::zero() {
					return Err(Error::<T>::NoClaimOrAlreadyClaimed);
				};
				Ok((balance_due, address))
			}
			None => Err(Error::<T>::InvalidEthereumSignature),
		}
	}

	/// Process previously verified claim.
	///
	/// Deposits the balance into the claiming account.
	///
	/// Emits `Claimed` when successful
	fn process_claim(dest: T::AccountId, balance_due: BalanceOf<T>, address: EthereumAddress) -> DispatchResult {
		let imbalance = CurrencyOf::<T>::deposit_creating(&dest, balance_due);
		ensure!(
			imbalance.peek() != <CurrencyOf::<T> as Currency<T::AccountId>>::PositiveImbalance::zero().peek(),
			Error::<T>::BalanceOverflow
		);

		Claims::<T>::mutate(address, |bal| *bal = Zero::zero());

		Self::deposit_event(Event::Claim(dest, address, balance_due));

		Ok(())
	}

	/// Process previously verified vested claim.
	///
	/// Deposits the balance vested into the claiming account.
	///
	/// Emits `Claimed` when successful
	fn process_vested_claim(
		dest: T::AccountId,
		balance_due: BalanceOf<T>,
		address: EthereumAddress,
		vesting_schedule: (BalanceOf<T>, T::BlockNumber),
	) -> DispatchResult {
		CurrencyOf::<T>::deposit_creating(&dest, balance_due);

		T::VestingSchedule::add_vesting_schedule(&dest, balance_due, vesting_schedule.0, vesting_schedule.1)
			.expect("No other vesting schedule exists");

		Claims::<T>::mutate(address, |bal| *bal = Zero::zero());

		Self::deposit_event(Event::Claim(dest, address, balance_due));

		Ok(())
	}
}

/// Converts the given binary data into ASCII-encoded hex. It will be twice the length.
fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
	let mut r = Vec::with_capacity(data.len() * 2);
	let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
	for &b in data.iter() {
		push_nibble(b / 16);
		push_nibble(b % 16);
	}
	r
}

/// Signed extension that checks for the `claim` call and in that case, it verifies an Ethereum signature
#[derive(Encode, Decode, Clone, Eq, PartialEq)]
pub struct ValidateClaim<T: Config + Send + Sync>(PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for ValidateClaim<T> {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "ValidateClaim")
	}
}

impl<T: Config + Send + Sync> SignedExtension for ValidateClaim<T>
where
	<T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
	const IDENTIFIER: &'static str = "ValidateClaim";
	type AccountId = T::AccountId;
	type Call = <T as frame_system::Config>::Call;
	type AdditionalSigned = ();
	type Pre = ();

	fn additional_signed(&self) -> sp_std::result::Result<(), TransactionValidityError> {
		Ok(())
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		match call.is_sub_type() {
			Some(Call::claim(signature)) => match Pallet::<T>::validate_claim(who, &signature) {
				Ok(_) => Ok(ValidTransaction::default()),
				Err(error) => InvalidTransaction::Custom(error.as_u8()).into(),
			},
			_ => Ok(Default::default()),
		}
	}
}

impl<T: Config + Send + Sync> ValidateClaim<T> {
	#[cfg_attr(feature = "cargo-clippy", allow(clippy::new_without_default))]
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}
