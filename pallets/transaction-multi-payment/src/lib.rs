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

pub mod weights;

use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{Currency, ExistenceRequirement, Get, Imbalance, OnUnbalanced, WithdrawReasons},
	transactional,
	weights::DispatchClass,
	weights::WeightToFeePolynomial,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, Saturating, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use sp_std::prelude::*;

use pallet_transaction_payment::OnChargeTransaction;
use sp_std::marker::PhantomData;

use frame_support::weights::{Pays, Weight};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::asset::AssetPair;
use primitives::traits::{CurrencySwap, AMM};
use primitives::{Amount, AssetId, Balance, CORE_ASSET_ID};

type NegativeImbalanceOf<C, T> = <C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use primitives::Price;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_transaction_payment::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency type in which fees will be paid.
		type Currency: Currency<Self::AccountId> + Send + Sync;

		/// Multi Currency
		type MultiCurrency: MultiCurrency<Self::AccountId>
			+ MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>;

		/// AMM pool to swap for native currency
		type AMMPool: AMM<Self::AccountId, AssetId, AssetPair, Balance>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Should fee be paid for setting a currency
		type WithdrawFeeForSetCurrency: Get<Pays>;

		/// Convert a weight value into a deductible fee based on the currency type.
		type WeightToFee: WeightToFeePolynomial<Balance = Balance>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// CurrencySet
		/// [who, currency]
		CurrencySet(T::AccountId, AssetId),

		/// New accepted currency added
		/// [who, currency]
		CurrencyAdded(T::AccountId, AssetId),

		/// Accepted currency removed
		/// [who, currency]
		CurrencyRemoved(T::AccountId, AssetId),

		/// Member added
		/// [who]
		MemberAdded(T::AccountId),

		/// Member removed
		/// [who]
		MemberRemoved(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Selected currency is not supported.
		UnsupportedCurrency,

		/// Account balance should be non-zero.
		ZeroBalance,

		/// Account is not allowed to add or remove accepted currency.
		NotAllowed,

		/// Currency is already in the list of accepted currencies.
		AlreadyAccepted,

		/// It is not allowed to add Core Asset as accepted currency. Core asset is accepted by design.
		CoreAssetNotAllowed,

		/// Account is already member of authorities.
		AlreadyMember,

		/// Account is not a member of authorities.
		NotAMember,

		/// Fallback price cannot be zero.
		ZeroPrice,
	}

	/// Account currency map
	#[pallet::storage]
	#[pallet::getter(fn get_currency)]
	pub type AccountCurrencyMap<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, AssetId, OptionQuery>;

	/// Curated list of currencies which fees can be paid with
	#[pallet::storage]
	#[pallet::getter(fn currencies)]
	pub type AcceptedCurrencies<T: Config> = StorageMap<_, Twox64Concat, AssetId, Price, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub type Authorities<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	/// Account to use when pool does not exist.
	#[pallet::storage]
	#[pallet::getter(fn fallback_acccount)]
	pub type FallbackAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub currencies: Vec<(AssetId, Price)>,
		pub authorities: Vec<T::AccountId>,
		pub fallback_account: T::AccountId,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				authorities: vec![],
				currencies: vec![],
				fallback_account: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			if self.fallback_account == Default::default() {
				panic!("Fallback account is not set");
			}

			FallbackAccount::<T>::put(self.fallback_account.clone());

			Authorities::<T>::put(self.authorities.clone());

			for (asset, price) in &self.currencies {
				AcceptedCurrencies::<T>::insert(asset, price);
			}
		}
	}
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set selected currency for given account.
		///
		/// This allows to set a currency for an account in which all transaction fees will be paid.
		/// Account balance cannot be zero.
		///
		/// Chosen currency must be in the list of accepted currencies.
		///
		/// When currency is set, fixed fee is withdrawn from the account to pay for the currency change
		///
		/// Emits `CurrencySet` event when successful.
		#[pallet::weight((<T as Config>::WeightInfo::set_currency(), DispatchClass::Normal, Pays::No))]
		#[transactional]
		pub fn set_currency(origin: OriginFor<T>, currency: AssetId) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			if currency == CORE_ASSET_ID || AcceptedCurrencies::<T>::contains_key(&currency) {
				if T::MultiCurrency::free_balance(currency, &who) == Balance::zero() {
					return Err(Error::<T>::ZeroBalance.into());
				}

				<AccountCurrencyMap<T>>::insert(who.clone(), currency);

				if T::WithdrawFeeForSetCurrency::get() == Pays::Yes {
					Self::withdraw_set_fee(&who, currency)?;
				}

				Self::deposit_event(Event::CurrencySet(who, currency));

				return Ok(().into());
			}

			Err(Error::<T>::UnsupportedCurrency.into())
		}

		/// Add a currency to the list of accepted currencies.
		///
		/// Only member can perform this action.
		///
		/// Currency must not be already accepted. Core asset id cannot be explicitly added.
		///
		/// Emits `CurrencyAdded` event when successful.
		#[pallet::weight((<T as Config>::WeightInfo::add_currency(), DispatchClass::Normal, Pays::No))]
		pub fn add_currency(origin: OriginFor<T>, currency: AssetId, price: Price) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(currency != CORE_ASSET_ID, Error::<T>::CoreAssetNotAllowed);

			// Only selected accounts can perform this action
			ensure!(Self::authorities().contains(&who), Error::<T>::NotAllowed);

			AcceptedCurrencies::<T>::try_mutate_exists(currency, |maybe_price| -> DispatchResultWithPostInfo {
				if maybe_price.is_some() {
					return Err(Error::<T>::AlreadyAccepted.into());
				}

				*maybe_price = Some(price);
				Self::deposit_event(Event::CurrencyAdded(who, currency));
				Ok(().into())
			})
		}

		/// Remove currency from the list of supported currencies
		/// Only selected members can perform this action
		///
		/// Core asset cannot be removed.
		///
		/// Emits `CurrencyRemoved` when successful.
		#[pallet::weight((<T as Config>::WeightInfo::remove_currency(), DispatchClass::Normal, Pays::No))]
		pub fn remove_currency(origin: OriginFor<T>, currency: AssetId) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(currency != CORE_ASSET_ID, Error::<T>::CoreAssetNotAllowed);

			// Only selected accounts can perform this action
			ensure!(Self::authorities().contains(&who), Error::<T>::NotAllowed);

			AcceptedCurrencies::<T>::try_mutate(currency, |x| -> DispatchResultWithPostInfo {
				if x.is_none() {
					return Err(Error::<T>::UnsupportedCurrency.into());
				}

				*x = None;

				Self::deposit_event(Event::CurrencyRemoved(who, currency));

				Ok(().into())
			})
		}

		/// Add an account as member to list of authorities who can manage list of accepted currencies
		///
		/// Members can be add or removed a currency from a list of accepted currencies.
		///
		/// Only root can be perform this action.
		///
		/// Emits `MemberAdded` when successful.
		#[pallet::weight((<T as Config>::WeightInfo::add_member(), DispatchClass::Normal, Pays::No))]
		pub fn add_member(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			ensure!(!Self::authorities().contains(&member), Error::<T>::AlreadyMember);

			Self::add_new_member(&member);

			Self::deposit_event(Event::MemberAdded(member));

			Ok(().into())
		}

		/// Rmove account from list of authorities who can manage list of accepted currencies
		///
		/// Only root can be perform this action.
		///
		/// Emits `MemberRemoved` when successful.
		#[pallet::weight((<T as Config>::WeightInfo::remove_member(), DispatchClass::Normal, Pays::No))]
		pub fn remove_member(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			ensure!(Self::authorities().contains(&member), Error::<T>::NotAMember);

			Authorities::<T>::mutate(|x| x.retain(|val| *val != member));

			Self::deposit_event(Event::MemberRemoved(member));

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Execute a trade to buy HDX and sell selected currency.
	pub fn swap_currency(who: &T::AccountId, fee: Balance) -> DispatchResult {
		// Let's determine currency in which user would like to pay the fee
		let fee_currency = match Pallet::<T>::get_currency(who) {
			Some(c) => c,
			_ => CORE_ASSET_ID,
		};

		// If not native currency, let's buy CORE asset first and then pay with that.
		if fee_currency != CORE_ASSET_ID {
			T::AMMPool::buy(
				&who,
				AssetPair {
					asset_out: CORE_ASSET_ID,
					asset_in: fee_currency,
				},
				fee,
				2u128 * fee,
				false,
			)?;
		}

		Ok(())
	}

	pub fn add_new_member(who: &T::AccountId) {
		Authorities::<T>::mutate(|x| x.push(who.clone()));
	}

	pub fn withdraw_set_fee(who: &T::AccountId, currency: AssetId) -> DispatchResult {
		let base_fee = Self::weight_to_fee(T::BlockWeights::get().get(DispatchClass::Normal).base_extrinsic);
		let adjusted_weight_fee = Self::weight_to_fee(T::WeightInfo::set_currency());
		let fee = base_fee.saturating_add(adjusted_weight_fee);

		Self::swap_currency(who, fee)?;
		T::MultiCurrency::withdraw(currency, who, fee)?;

		Ok(())
	}

	fn weight_to_fee(weight: Weight) -> Balance {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight: Weight = weight.min(T::BlockWeights::get().max_block);
		<T as Config>::WeightToFee::calc(&capped_weight)
	}
}

impl<T: Config> CurrencySwap<<T as frame_system::Config>::AccountId, Balance> for Pallet<T> {
	fn swap_currency(who: &T::AccountId, fee: u128) -> DispatchResult {
		Self::swap_currency(who, fee)
	}
}

/// Implements the transaction payment for native as well as non-native currencies
pub struct MultiCurrencyAdapter<C, OU, SW>(PhantomData<(C, OU, SW)>);

impl<T, C, OU, SW> OnChargeTransaction<T> for MultiCurrencyAdapter<C, OU, SW>
where
	T: Config,
	T::TransactionByteFee: Get<<C as Currency<<T as frame_system::Config>::AccountId>>::Balance>,
	C: Currency<<T as frame_system::Config>::AccountId>,
	C::PositiveImbalance:
		Imbalance<<C as Currency<<T as frame_system::Config>::AccountId>>::Balance, Opposite = C::NegativeImbalance>,
	C::NegativeImbalance:
		Imbalance<<C as Currency<<T as frame_system::Config>::AccountId>>::Balance, Opposite = C::PositiveImbalance>,
	OU: OnUnbalanced<NegativeImbalanceOf<C, T>>,
	C::Balance: Into<Balance>,
	SW: CurrencySwap<T::AccountId, Balance>,
{
	type LiquidityInfo = Option<NegativeImbalanceOf<C, T>>;
	type Balance = <C as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Withdraw the predicted fee from the transaction origin.
	///
	/// Note: The `fee` already includes the `tip`.
	fn withdraw_fee(
		who: &T::AccountId,
		_call: &T::Call,
		_info: &DispatchInfoOf<T::Call>,
		fee: Self::Balance,
		tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None);
		}

		let withdraw_reason = if tip.is_zero() {
			WithdrawReasons::TRANSACTION_PAYMENT
		} else {
			WithdrawReasons::TRANSACTION_PAYMENT | WithdrawReasons::TIP
		};

		if SW::swap_currency(&who, fee.into()).is_err() {
			return Err(InvalidTransaction::Payment.into());
		}

		match C::withdraw(who, fee, withdraw_reason, ExistenceRequirement::KeepAlive) {
			Ok(imbalance) => Ok(Some(imbalance)),
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	/// Hand the fee and the tip over to the `[OnUnbalanced]` implementation.
	/// Since the predicted fee might have been too high, parts of the fee may
	/// be refunded.
	///
	/// Note: The `fee` already includes the `tip`.
	/// Note: This is the default implementation
	fn correct_and_deposit_fee(
		who: &T::AccountId,
		_dispatch_info: &DispatchInfoOf<T::Call>,
		_post_info: &PostDispatchInfoOf<T::Call>,
		corrected_fee: Self::Balance,
		tip: Self::Balance,
		already_withdrawn: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		if let Some(paid) = already_withdrawn {
			// Calculate how much refund we should return
			let refund_amount = paid.peek().saturating_sub(corrected_fee);
			// refund to the the account that paid the fees. If this fails, the
			// account might have dropped below the existential balance. In
			// that case we don't refund anything.
			let refund_imbalance =
				C::deposit_into_existing(&who, refund_amount).unwrap_or_else(|_| C::PositiveImbalance::zero());
			// merge the imbalance caused by paying the fees and refunding parts of it again.
			let adjusted_paid = paid
				.offset(refund_imbalance)
				.same()
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			// Call someone else to handle the imbalance (fee and tip separately)
			let imbalances = adjusted_paid.split(tip);
			OU::on_unbalanceds(Some(imbalances.0).into_iter().chain(Some(imbalances.1)));
		}
		Ok(())
	}
}
