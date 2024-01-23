// This file is part of Basilisk-node.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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
mod traits;

use frame_support::traits::Contains;
use frame_support::{dispatch::DispatchResult, ensure, traits::Get, weights::Weight};
use frame_system::{ensure_signed, pallet_prelude::BlockNumberFor};
use hydra_dx_math::ema::EmaPrice;
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
use hydradx_traits::NativePriceOracle;
use orml_traits::{GetByKey, Happened, MultiCurrency};

pub use crate::traits::*;
use frame_support::traits::IsSubType;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{OraclePeriod, PriceOracle};

type AssetIdOf<T> = <<T as Config>::Currencies as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Config>::Currencies as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

/// Spot price type
pub type Price = FixedU128;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::weights::WeightToFee;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			let native_asset = T::NativeAssetId::get();

			let mut weight: u64 = 0;

			for (asset_id, fallback_price) in <AcceptedCurrencies<T>>::iter() {
				let maybe_price = Self::get_oracle_price(asset_id, native_asset);

				let price = maybe_price.unwrap_or(fallback_price);

				AcceptedCurrencyPrice::<T>::insert(asset_id, price);

				weight += T::WeightInfo::get_oracle_price().ref_time();
			}

			Weight::from_parts(weight, 0)
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			let _ = <AcceptedCurrencyPrice<T>>::clear(u32::MAX, None);
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_transaction_payment::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin which can add/remove accepted currencies
		type AcceptedCurrencyOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Multi Currency
		type Currencies: MultiCurrency<Self::AccountId>;

		/// On chain route provider
		type RouteProvider: RouteProvider<AssetIdOf<Self>>;

		/// Oracle price provider for routes
		type OraclePriceProvider: PriceOracle<AssetIdOf<Self>, Price = EmaPrice>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Convert a weight value into a deductible fee based on the currency type.
		type WeightToFee: WeightToFee<Balance = BalanceOf<Self>>;

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
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub currencies: Vec<(AssetIdOf<T>, Price)>,
		pub account_currencies: Vec<(T::AccountId, AssetIdOf<T>)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
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
	where
		BalanceOf<T>: FixedPointOperand,
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

			ensure!(
				currency == T::NativeAssetId::get() || AcceptedCurrencies::<T>::contains_key(currency),
				Error::<T>::UnsupportedCurrency
			);

			<AccountCurrencyMap<T>>::insert(who.clone(), currency);

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
			T::AcceptedCurrencyOrigin::ensure_origin(origin)?;

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
			T::AcceptedCurrencyOrigin::ensure_origin(origin)?;

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

impl<T: Config> Pallet<T> {
	fn account_currency(who: &T::AccountId) -> AssetIdOf<T>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		Pallet::<T>::get_currency(who).unwrap_or_else(T::NativeAssetId::get)
	}

	fn get_currency_price(currency: AssetIdOf<T>) -> Option<Price>
	where
		BalanceOf<T>: FixedPointOperand,
	{
		if let Some(price) = Self::price(currency) {
			Some(price)
		} else {
			// If not loaded in on_init, let's try first the spot price provider again
			// This is unlikely scenario as the price would be retrieved in on_init for each block
			let maybe_price = Self::get_oracle_price(currency, T::NativeAssetId::get());

			if let Some(price) = maybe_price {
				Some(price)
			} else {
				Self::currencies(currency)
			}
		}
	}

	fn get_oracle_price(
		asset_id: <T::Currencies as MultiCurrency<T::AccountId>>::CurrencyId,
		native_asset: <T::Currencies as MultiCurrency<T::AccountId>>::CurrencyId,
	) -> Option<FixedU128> {
		let on_chain_route = T::RouteProvider::get_route(AssetPair::new(asset_id, native_asset));

		T::OraclePriceProvider::price(&on_chain_route, OraclePeriod::Short)
			.map(|ratio| FixedU128::from_rational(ratio.n, ratio.d))
	}
}

fn convert_fee_with_price<B>(fee: B, price: FixedU128) -> Option<B>
where
	B: FixedPointOperand + Ord + One,
{
	// Make sure that the fee is never less than 1
	price.checked_mul_int(fee).map(|f| f.max(One::one()))
}

/// Deposits all fees to some account
pub struct DepositAll<T>(PhantomData<T>);

impl<T: Config> DepositFee<T::AccountId, AssetIdOf<T>, BalanceOf<T>> for DepositAll<T> {
	fn deposit_fee(who: &T::AccountId, currency: AssetIdOf<T>, amount: BalanceOf<T>) -> DispatchResult {
		<T as Config>::Currencies::deposit(currency, who, amount)?;
		Ok(())
	}
}

#[cfg(feature = "evm")]
use {
	frame_support::traits::{Currency as PalletCurrency, Imbalance, OnUnbalanced},
	pallet_evm::{EVMCurrencyAdapter, OnChargeEVMTransaction},
	sp_core::{H160, U256},
	sp_runtime::traits::UniqueSaturatedInto,
};
#[cfg(feature = "evm")]
type CurrencyAccountId<T> = <T as frame_system::Config>::AccountId;
#[cfg(feature = "evm")]
type BalanceFor<T> = <<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::Balance;
#[cfg(feature = "evm")]
type PositiveImbalanceFor<T> =
	<<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::PositiveImbalance;
#[cfg(feature = "evm")]
type NegativeImbalanceFor<T> =
	<<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::NegativeImbalance;

#[cfg(feature = "evm")]
/// Implements the transaction payment for EVM transactions.
pub struct TransferEvmFees<OU>(PhantomData<OU>);

#[cfg(feature = "evm")]
impl<T, OU> OnChargeEVMTransaction<T> for TransferEvmFees<OU>
where
	T: Config + pallet_evm::Config,
	PositiveImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = NegativeImbalanceFor<T>>,
	NegativeImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = PositiveImbalanceFor<T>>,
	OU: OnUnbalanced<NegativeImbalanceFor<T>>,
	U256: UniqueSaturatedInto<BalanceFor<T>>,
{
	type LiquidityInfo = Option<NegativeImbalanceFor<T>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		EVMCurrencyAdapter::<<T as pallet_evm::Config>::Currency, ()>::withdraw_fee(who, fee)
	}

	fn can_withdraw(who: &H160, amount: U256) -> Result<(), pallet_evm::Error<T>> {
		EVMCurrencyAdapter::<<T as pallet_evm::Config>::Currency, ()>::can_withdraw(who, amount)
	}
	fn correct_and_deposit_fee(
		who: &H160,
		corrected_fee: U256,
		base_fee: U256,
		already_withdrawn: Self::LiquidityInfo,
	) -> Self::LiquidityInfo {
		<EVMCurrencyAdapter<<T as pallet_evm::Config>::Currency, OU> as OnChargeEVMTransaction<
			T,
		>>::correct_and_deposit_fee(who, corrected_fee, base_fee, already_withdrawn)
	}

	fn pay_priority_fee(tip: Self::LiquidityInfo) {
		if let Some(tip) = tip {
			OU::on_unbalanced(tip);
		}
	}
}

/// Implements the transaction payment for native as well as non-native currencies
pub struct TransferFees<MC, DF, FR>(PhantomData<(MC, DF, FR)>);

impl<T, MC, DF, FR> OnChargeTransaction<T> for TransferFees<MC, DF, FR>
where
	T: Config,
	MC: MultiCurrency<<T as frame_system::Config>::AccountId>,
	AssetIdOf<T>: Into<MC::CurrencyId>,
	MC::Balance: FixedPointOperand,
	FR: Get<T::AccountId>,
	DF: DepositFee<T::AccountId, MC::CurrencyId, MC::Balance>,
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>>,
	BalanceOf<T>: FixedPointOperand,
{
	type LiquidityInfo = Option<PaymentInfo<Self::Balance, AssetIdOf<T>, Price>>;
	type Balance = <MC as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Withdraw the predicted fee from the transaction origin.
	///
	/// Note: The `fee` already includes the `tip`.
	fn withdraw_fee(
		who: &T::AccountId,
		call: &T::RuntimeCall,
		_info: &DispatchInfoOf<T::RuntimeCall>,
		fee: Self::Balance,
		_tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
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

		match MC::withdraw(currency.into(), who, converted_fee) {
			Ok(()) => {
				if currency == T::NativeAssetId::get() {
					Ok(Some(PaymentInfo::Native(fee)))
				} else {
					Ok(Some(PaymentInfo::NonNative(converted_fee, currency, price)))
				}
			}
			Err(_) => Err(InvalidTransaction::Payment.into()),
		}
	}

	/// Since the predicted fee might have been too high, parts of the fee may
	/// be refunded.
	///
	/// Note: The `fee` already includes the `tip`.
	fn correct_and_deposit_fee(
		who: &T::AccountId,
		_dispatch_info: &DispatchInfoOf<T::RuntimeCall>,
		_post_info: &PostDispatchInfoOf<T::RuntimeCall>,
		corrected_fee: Self::Balance,
		tip: Self::Balance,
		already_withdrawn: Self::LiquidityInfo,
	) -> Result<(), TransactionValidityError> {
		let fee_receiver = FR::get();

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
			MC::deposit(currency, who, refund)
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			// deposit the fee
			DF::deposit_fee(&fee_receiver, currency, fee + tip)
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
		}

		Ok(())
	}
}

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

/// Type to automatically add a fee currency for an account on account creation.
pub struct AddTxAssetOnAccount<T>(PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, AssetIdOf<T>)> for AddTxAssetOnAccount<T> {
	fn happened((who, currency): &(T::AccountId, AssetIdOf<T>)) {
		if !AccountCurrencyMap::<T>::contains_key(who)
			&& AcceptedCurrencies::<T>::contains_key(currency)
			&& T::Currencies::total_balance(T::NativeAssetId::get(), who).is_zero()
		{
			AccountCurrencyMap::<T>::insert(who, currency);
		}
	}
}

/// Type to automatically remove the fee currency for an account on account deletion.
///
/// Note: The fee currency is only removed if the system account is gone or the account
/// corresponding to the fee currency is empty.
pub struct RemoveTxAssetOnKilled<T>(PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, AssetIdOf<T>)> for RemoveTxAssetOnKilled<T> {
	fn happened((who, _currency): &(T::AccountId, AssetIdOf<T>)) {
		if !frame_system::Pallet::<T>::account_exists(who) {
			AccountCurrencyMap::<T>::remove(who);
		} else if let Some(currency) = AccountCurrencyMap::<T>::get(who) {
			if T::Currencies::total_balance(currency, who).is_zero() {
				AccountCurrencyMap::<T>::remove(who);
			}
		}
	}
}

impl<T: Config> Contains<AssetIdOf<T>> for Pallet<T> {
	fn contains(currency: &AssetIdOf<T>) -> bool {
		AcceptedCurrencies::<T>::contains_key(currency)
	}
}

impl<T: Config> GetByKey<AssetIdOf<T>, Option<FixedU128>> for Pallet<T> {
	fn get(k: &AssetIdOf<T>) -> Option<FixedU128> {
		AcceptedCurrencyPrice::<T>::get(k)
	}
}
