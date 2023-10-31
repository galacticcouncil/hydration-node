// This file is part of HydraDX-node

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

pub mod weights;
use weights::WeightInfo;

#[cfg(test)]
mod tests;

mod traits;

use frame_support::{dispatch::DispatchResult, ensure, traits::Get, weights::Weight};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{DispatchInfoOf, One, PostDispatchInfoOf, Saturating, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	FixedU128,
};
use sp_std::prelude::*;

use pallet_transaction_payment::OnChargeTransaction;
use sp_std::marker::PhantomData;

use frame_support::sp_runtime::FixedPointNumber;
use frame_support::sp_runtime::FixedPointOperand;
use hydradx_traits::{pools::SpotPriceProvider, NativePriceOracle};
use orml_traits::{Happened, MultiCurrency};
use sp_arithmetic::traits::BaseArithmetic;

use frame_support::traits::IsSubType;

pub use crate::traits::*;

type AssetIdOf<T> = <T as Config>::AssetId;
//type BalanceOf<T> = <<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<T>>::Balance;
type BalanceOf<T> = <T as Config>::Balance;

/// Spot price type
pub type Price = FixedU128;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			let native_asset = T::NativeAssetId::get();

			let mut weight: u64 = 0;

			for (asset_id, fallback_price) in <AcceptedCurrencies<T>>::iter() {
				let maybe_price = T::SpotPriceProvider::spot_price(asset_id, native_asset);

				let price = maybe_price.unwrap_or(fallback_price);

				AcceptedCurrencyPrice::<T>::insert(asset_id, price);

				weight += T::WeightInfo::get_spot_price().ref_time();
			}

			Weight::from_ref_time(weight)
		}

		fn on_finalize(_n: T::BlockNumber) {
			let _ = <AcceptedCurrencyPrice<T>>::clear(u32::MAX, None);
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_transaction_payment::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type
		type AssetId: frame_support::traits::tokens::AssetId
			+ MaybeSerializeDeserialize;

		type Balance: frame_support::traits::tokens::Balance;

		/// The origin which can add/remove accepted currencies
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Spot price provider
		type SpotPriceProvider: SpotPriceProvider<AssetIdOf<Self>, Price = Price>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Native Asset
		#[pallet::constant]
		type NativeAssetId: Get<AssetIdOf<Self>>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// CurrencySet
		/// [who, currency]
		CurrencySet {
			account_id: T::AccountId,
			asset_id: AssetIdOf<T>,
		},

		/// New accepted currency added
		/// [currency]
		CurrencyAdded { asset_id: AssetIdOf<T> },

		/// Accepted currency removed
		/// [currency]
		CurrencyRemoved { asset_id: AssetIdOf<T> },

		/// Transaction fee paid in non-native currency
		/// [Account, Currency, Native fee amount, Non-native fee amount, Destination account]
		FeeWithdrawn {
			account_id: T::AccountId,
			asset_id: AssetIdOf<T>,
			native_fee_amount: BalanceOf<T>,
			non_native_fee_amount: BalanceOf<T>,
			destination_account_id: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Selected currency is not supported.
		UnsupportedCurrency,

		/// Account balance should be non-zero.
		ZeroBalance,

		/// Currency is already in the list of accepted currencies.
		AlreadyAccepted,

		/// It is not allowed to add Core Asset as accepted currency. Core asset is accepted by design.
		CoreAssetNotAllowed,

		/// Fallback price cannot be zero.
		ZeroPrice,

		/// Fallback price was not found.
		FallbackPriceNotFound,

		/// Math overflow
		Overflow,
	}

	/// Account currency map
	#[pallet::storage]
	#[pallet::getter(fn get_currency)]
	pub type AccountCurrencyMap<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, AssetIdOf<T>, OptionQuery>;

	/// Curated list of currencies which fees can be paid mapped to corresponding fallback price
	#[pallet::storage]
	#[pallet::getter(fn currencies)]
	pub type AcceptedCurrencies<T: Config> = StorageMap<_, Twox64Concat, AssetIdOf<T>, Price, OptionQuery>;

	/// Asset prices from the spot price provider or the fallback price if the price is not available. Updated at the beginning of every block.
	#[pallet::storage]
	#[pallet::getter(fn currency_price)]
	pub type AcceptedCurrencyPrice<T: Config> = StorageMap<_, Twox64Concat, AssetIdOf<T>, Price, OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub currencies: Vec<(AssetIdOf<T>, Price)>,
		pub account_currencies: Vec<(T::AccountId, AssetIdOf<T>)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				currencies: vec![],
				account_currencies: vec![],
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			for (asset, price) in &self.currencies {
				AcceptedCurrencies::<T>::insert(asset, price);
			}

			for (account, asset) in &self.account_currencies {
				<AccountCurrencyMap<T>>::insert(account, asset);
			}
		}
	}
	#[pallet::call]
	impl<T: Config> Pallet<T>
	{
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
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_currency())]
		pub fn set_currency(origin: OriginFor<T>, currency: AssetIdOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::set_account_currency(&who, currency)?;

			Self::deposit_event(Event::CurrencySet {
				account_id: who,
				asset_id: currency,
			});

			Ok(())
		}

		/// Add a currency to the list of accepted currencies.
		///
		/// Only member can perform this action.
		///
		/// Currency must not be already accepted. Core asset id cannot be explicitly added.
		///
		/// Emits `CurrencyAdded` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::add_currency())]
		pub fn add_currency(origin: OriginFor<T>, currency: AssetIdOf<T>, price: Price) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(currency != T::NativeAssetId::get(), Error::<T>::CoreAssetNotAllowed);

			AcceptedCurrencies::<T>::try_mutate_exists(currency, |maybe_price| -> DispatchResult {
				if maybe_price.is_some() {
					return Err(Error::<T>::AlreadyAccepted.into());
				}

				*maybe_price = Some(price);
				Self::deposit_event(Event::CurrencyAdded { asset_id: currency });
				Ok(())
			})
		}

		/// Remove currency from the list of supported currencies
		/// Only selected members can perform this action
		///
		/// Core asset cannot be removed.
		///
		/// Emits `CurrencyRemoved` when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_currency())]
		pub fn remove_currency(origin: OriginFor<T>, currency: AssetIdOf<T>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(currency != T::NativeAssetId::get(), Error::<T>::CoreAssetNotAllowed);

			AcceptedCurrencies::<T>::try_mutate(currency, |x| -> DispatchResult {
				if x.is_none() {
					return Err(Error::<T>::UnsupportedCurrency.into());
				}

				*x = None;

				Self::deposit_event(Event::CurrencyRemoved { asset_id: currency });

				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T>
{
	pub fn set_account_currency(account: &T::AccountId, currency: AssetIdOf<T>) -> DispatchResult {
		ensure!(
			currency == T::NativeAssetId::get() || AcceptedCurrencies::<T>::contains_key(currency),
			Error::<T>::UnsupportedCurrency
		);

		<AccountCurrencyMap<T>>::insert(account, currency);
		Ok(())
	}

	fn account_currency(who: &T::AccountId) -> AssetIdOf<T> {
		Pallet::<T>::get_currency(who).unwrap_or_else(T::NativeAssetId::get)
	}

	fn get_currency_price(currency: AssetIdOf<T>) -> Option<Price> {
		T::SpotPriceProvider::spot_price(currency, T::NativeAssetId::get())
	}
}

fn convert_fee_with_price<B>(fee: B, price: FixedU128) -> Option<B>
where
	B: FixedPointOperand + Ord + One,
{
	// Make sure that the fee is never less than 1
	price.checked_mul_int(fee).map(|f| f.max(One::one()))
}

pub struct OnChargeAssetFeeAdapter<MC, FR>(PhantomData<(MC,FR)>);

impl<T, MC, FR> OnChargeTransaction<T> for OnChargeAssetFeeAdapter<MC, FR>
where
	T: Config,
	MC: frame_support::traits::tokens::fungibles::Mutate<T::AccountId, Balance = BalanceOf<T>, AssetId = AssetIdOf<T>>,
	FR: Get<T::AccountId>,
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
	BalanceOf<T>: FixedPointOperand,
{
	type LiquidityInfo = Option<PaymentInfo<Self::Balance, AssetIdOf<T>, Price>>;
	type Balance = BalanceOf<T>;

	fn withdraw_fee(who: &T::AccountId, call: &T::RuntimeCall, dispatch_info: &DispatchInfoOf<T::RuntimeCall>, fee: Self::Balance, tip: Self::Balance) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None);
		}

		let currency = match call.is_sub_type() {
			Some(Call::set_currency { currency }) => *currency,
			_ => Pallet::<T>::account_currency(who),
		};

		let price = Pallet::<T>::get_currency_price(currency)
			.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

		let converted_fee =
			convert_fee_with_price(fee, price).ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

		match MC::burn_from(currency.into(), who, converted_fee) {
			Ok(_) => {
				if currency == T::NativeAssetId::get() {
					Ok(Some(PaymentInfo::Native(fee)))
				} else {
					Ok(Some(PaymentInfo::NonNative(converted_fee, currency, price)))
				}
			}
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	fn correct_and_deposit_fee(who: &T::AccountId, dispatch_info: &DispatchInfoOf<T::RuntimeCall>, post_info: &PostDispatchInfoOf<T::RuntimeCall>, corrected_fee: Self::Balance, tip: Self::Balance, already_withdrawn: Self::LiquidityInfo) -> Result<(), TransactionValidityError> {
		if let Some(paid) = already_withdrawn {
			// Calculate how much refund we should return
			let (currency, refund, fee, tip) = match paid {
				PaymentInfo::Native(paid_fee) => (
					T::NativeAssetId::get().into(),
					paid_fee.saturating_sub(corrected_fee),
					corrected_fee.saturating_sub(tip),
					tip,
				),
				PaymentInfo::NonNative(paid_fee, currency, price) => {
					// calculate corrected_fee in the non-native currency
					let converted_corrected_fee = convert_fee_with_price(corrected_fee, price)
						.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
					let refund = paid_fee.saturating_sub(converted_corrected_fee);
					let converted_tip = price
						.checked_mul_int(tip)
						.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
					(
						currency.into(),
						refund,
						converted_corrected_fee.saturating_sub(converted_tip),
						converted_tip,
					)
				}
			};

			// refund to the account that paid the fees
			MC::mint_into(currency, who, refund)
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			MC::mint_into(currency, &FR::get(),fee.saturating_add(tip))
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
		}
		Ok(())
	}
}

/*
/// We provide an oracle for the price of all currencies accepted as fee payment.
impl<T: Config> NativePriceOracle<AssetIdOf<T>, Price> for Pallet<T> {
	fn price(currency: AssetIdOf<T>) -> Option<Price> {
		if currency == T::NativeAssetId::get() {
			Some(Price::one())
		} else {
			Pallet::<T>::currency_price(currency)
		}
	}
}

 */

/// Type to automatically add a fee currency for an account on account creation.
pub struct AddTxAssetOnAccount<T, Inspector>(PhantomData<(T, Inspector)>);
impl<T: Config, Inspector> Happened<(T::AccountId, AssetIdOf<T>)> for AddTxAssetOnAccount<T, Inspector>
where Inspector: frame_support::traits::tokens::fungible::Inspect<T::AccountId>{
	fn happened((who, currency): &(T::AccountId, AssetIdOf<T>)) {
		if !AccountCurrencyMap::<T>::contains_key(who)
			&& AcceptedCurrencies::<T>::contains_key(currency)
			&& Inspector::balance(who).is_zero()
		{
			AccountCurrencyMap::<T>::insert(who, currency);
		}
	}
}

/// Type to automatically remove the fee currency for an account on account deletion.
///
/// Note: The fee currency is only removed if the system account is gone or the account
/// corresponding to the fee currency is empty.
pub struct RemoveTxAssetOnKilled<T, Inspector>(PhantomData<(T, Inspector)>);
impl<T: Config, Inspector> Happened<(T::AccountId, AssetIdOf<T>)> for RemoveTxAssetOnKilled<T, Inspector>
where
Inspector: frame_support::traits::fungibles::Inspect<T::AccountId, AssetId=AssetIdOf<T>>
{
	fn happened((who, _currency): &(T::AccountId, AssetIdOf<T>)) {
		if !frame_system::Pallet::<T>::account_exists(who) {
			AccountCurrencyMap::<T>::remove(who);
		} else if let Some(currency) = AccountCurrencyMap::<T>::get(who) {
			if Inspector::balance(currency, who).is_zero() {
				AccountCurrencyMap::<T>::remove(who);
			}
		}
	}
}
