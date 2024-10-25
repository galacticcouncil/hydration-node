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
#![allow(clippy::too_many_arguments)]
#![allow(clippy::large_enum_variant)]

pub mod weights;

pub use weights::WeightInfo;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
mod traits;

pub use crate::traits::*;
use frame_support::storage::with_transaction;
use frame_support::traits::{Contains, IsSubType};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	sp_runtime::{
		traits::{DispatchInfoOf, One, PostDispatchInfoOf, Saturating, Zero},
		transaction_validity::{InvalidTransaction, TransactionValidityError},
		FixedPointNumber, FixedPointOperand, FixedU128,
	},
	traits::Get,
	weights::Weight,
};
use frame_system::{ensure_signed, pallet_prelude::BlockNumberFor};
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::fee::InspectTransactionFeeCurrency;
use hydradx_traits::fee::SwappablePaymentAssetTrader;
use hydradx_traits::{
	evm::InspectEvmAccounts,
	router::{AssetPair, RouteProvider},
	AccountFeeCurrency, NativePriceOracle, OraclePeriod, PriceOracle,
};
use orml_traits::{GetByKey, Happened, MultiCurrency};
use pallet_transaction_payment::OnChargeTransaction;
use sp_runtime::traits::TryConvert;
use sp_std::{marker::PhantomData, prelude::*};

pub type AssetIdOf<T> =
	<<T as Config>::Currencies as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as Config>::Currencies as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

/// Spot price type
pub type Price = FixedU128;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::DecodeLimit;
	use frame_support::dispatch::PostDispatchInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::weights::WeightToFee;
	use frame_system::ensure_none;
	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::fee::SwappablePaymentAssetTrader;
	use sp_core::{H160, H256, U256};
	use sp_runtime::{ModuleError, TransactionOutcome};

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

		/// Supporting swappable assets as fee currencies
		type SwappablePaymentAssetSupport: SwappablePaymentAssetTrader<
			Self::AccountId,
			AssetIdOf<Self>,
			BalanceOf<Self>,
		>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		/// Convert a weight value into a deductible fee based on the currency type.
		type WeightToFee: WeightToFee<Balance = BalanceOf<Self>>;

		/// Native Asset
		#[pallet::constant]
		type NativeAssetId: Get<AssetIdOf<Self>>;

		/// Polkadot Native Asset (DOT)
		#[pallet::constant]
		type PolkadotNativeAssetId: Get<AssetIdOf<Self>>;

		/// EVM Asset
		#[pallet::constant]
		type EvmAssetId: Get<AssetIdOf<Self>>;

		/// EVM Accounts info
		type InspectEvmAccounts: InspectEvmAccounts<Self::AccountId>;

		type EvmPermit: EVMPermit;

		/// Try to retrieve fee currency from runtime call.
		/// It is generic implementation to avoid tight coupling with other pallets such as utility.
		type TryCallCurrency<'a>: TryConvert<&'a <Self as frame_system::Config>::RuntimeCall, AssetIdOf<Self>>;
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

		/// It is not allowed to change payment currency of an EVM account.
		EvmAccountNotAllowed,

		/// EVM permit expired.
		EvmPermitExpired,

		/// EVM permit is invalid.
		EvmPermitInvalid,

		/// EVM permit call failed.
		EvmPermitCallExecutionError,

		/// EVM permit call failed.
		EvmPermitRunnerError,
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

	#[pallet::storage]
	#[pallet::getter(fn tx_fee_currency_override)]
	pub type TransactionCurrencyOverride<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, AssetIdOf<T>, OptionQuery>;

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
		/// In case of sufficient asset, the chosen currency must be in the list of accepted currencies
		/// In case of insufficient asset, the chosen currency must have a XYK pool with DOT
		///
		/// When currency is set, fixed fee is withdrawn from the account to pay for the currency change
		///
		/// EVM accounts are now allowed to change thier payment currency.
		///
		/// Emits `CurrencySet` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_currency())]
		pub fn set_currency(origin: OriginFor<T>, currency: AssetIdOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if T::SwappablePaymentAssetSupport::is_transaction_fee_currency(currency) {
				ensure!(
					currency == T::NativeAssetId::get() || AcceptedCurrencies::<T>::contains_key(currency),
					Error::<T>::UnsupportedCurrency
				);
			} else {
				ensure!(
					T::SwappablePaymentAssetSupport::is_trade_supported(currency, T::PolkadotNativeAssetId::get()),
					Error::<T>::UnsupportedCurrency
				);
			}

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

		/// Reset currency of the specified account to HDX.
		/// If the account is EVM account, the payment currency is reset to WETH.
		/// Only selected members can perform this action.
		///
		/// Emits `CurrencySet` when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::reset_payment_currency())]
		pub fn reset_payment_currency(origin: OriginFor<T>, account_id: T::AccountId) -> DispatchResult {
			T::AcceptedCurrencyOrigin::ensure_origin(origin)?;

			let currency = if T::InspectEvmAccounts::is_evm_account(account_id.clone()) {
				let currency = T::EvmAssetId::get();
				AccountCurrencyMap::<T>::insert(account_id.clone(), currency);
				currency
			} else {
				AccountCurrencyMap::<T>::remove(account_id.clone());
				T::NativeAssetId::get()
			};

			Self::deposit_event(Event::CurrencySet {
				account_id,
				asset_id: currency,
			});

			Ok(())
		}

		/// Dispatch EVM permit.
		/// The main purpose of this function is to allow EVM accounts to pay for the transaction fee in non-native currency
		/// by allowing them to self-dispatch pre-signed permit.
		/// The EVM fee is paid in the currency set for the account.
		#[pallet::call_index(4)]
		#[pallet::weight(
			<T as Config>::EvmPermit::dispatch_weight(*gas_limit)
		)]
		pub fn dispatch_permit(
			origin: OriginFor<T>,
			from: H160,
			to: H160,
			value: U256,
			data: Vec<u8>,
			gas_limit: u64,
			deadline: U256,
			v: u8,
			r: H256,
			s: H256,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;

			// dispatch permit should never return error.
			// validate_unsigned should prevent the transaction getting to this point in case of invalid permit.
			// In case of any error, we call error handler ( which should pause this transaction) and return ok.

			if T::EvmPermit::validate_permit(from, to, data.clone(), value, gas_limit, deadline, v, r, s).is_err() {
				T::EvmPermit::on_dispatch_permit_error();
				return Ok(PostDispatchInfo::default());
			};

			let (gas_price, _) = T::EvmPermit::gas_price();

			// Set fee currency for the evm dispatch
			let account_id = T::InspectEvmAccounts::account_id(from);

			let encoded = data.clone();
			let mut encoded_extrinsic = encoded.as_slice();
			let maybe_call: Result<<T as frame_system::Config>::RuntimeCall, _> =
				DecodeLimit::decode_all_with_depth_limit(32, &mut encoded_extrinsic);

			let currency = if let Ok(call) = maybe_call {
				T::TryCallCurrency::try_convert(&call).unwrap_or_else(|_| Pallet::<T>::account_currency(&account_id))
			} else {
				Pallet::<T>::account_currency(&account_id)
			};

			TransactionCurrencyOverride::<T>::insert(account_id.clone(), currency);

			let result = T::EvmPermit::dispatch_permit(from, to, data, value, gas_limit, gas_price, None, None, vec![])
				.unwrap_or_else(|e| {
					// In case of runner error, account has not been charged, so we need to call error handler to pause dispatch error
					if e.error == Error::<T>::EvmPermitRunnerError.into() {
						T::EvmPermit::on_dispatch_permit_error();
					}
					e.post_info
				});

			TransactionCurrencyOverride::<T>::remove(account_id.clone());

			Ok(result)
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
	where
		AssetIdOf<T>: Into<u32>,
	{
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match call {
				Call::dispatch_permit {
					from,
					to,
					value,
					data,
					gas_limit,
					deadline,
					v,
					r,
					s,
				} => {
					// We need to wrap this as separate tx, and since we also "dry-run" the dispatch,
					// we need to rollback the changes if any
					let result = with_transaction::<(), DispatchError, _>(|| {
						// First verify signature
						let result = T::EvmPermit::validate_permit(
							*from,
							*to,
							data.clone(),
							*value,
							*gas_limit,
							*deadline,
							*v,
							*r,
							*s,
						);
						if let Some(error_res) = result.err() {
							return TransactionOutcome::Rollback(Err(error_res));
						}

						// Set fee currency for the evm dispatch
						let account_id = T::InspectEvmAccounts::account_id(*from);

						let encoded = data.clone();
						let mut encoded_extrinsic = encoded.as_slice();
						let maybe_call: Result<<T as frame_system::Config>::RuntimeCall, _> =
							DecodeLimit::decode_all_with_depth_limit(32, &mut encoded_extrinsic);

						let currency = if let Ok(call) = maybe_call {
							T::TryCallCurrency::try_convert(&call)
								.unwrap_or_else(|_| crate::pallet::Pallet::<T>::account_currency(&account_id))
						} else {
							Pallet::<T>::account_currency(&account_id)
						};

						TransactionCurrencyOverride::<T>::insert(account_id.clone(), currency);

						let (gas_price, _) = T::EvmPermit::gas_price();

						let result = T::EvmPermit::dispatch_permit(
							*from,
							*to,
							data.clone(),
							*value,
							*gas_limit,
							gas_price,
							None,
							None,
							vec![],
						);
						TransactionCurrencyOverride::<T>::remove(&account_id);
						match result {
							Ok(_post_info) => TransactionOutcome::Rollback(Ok(())),
							Err(e) => TransactionOutcome::Rollback(Err(e.error)),
						}
					});
					let nonce = T::EvmPermit::permit_nonce(*from);
					match result {
						Ok(()) => ValidTransaction::with_tag_prefix("EVMPermit")
							.and_provides((nonce, from))
							.priority(0)
							.longevity(64)
							.propagate(true)
							.build(),
						Err(e) => {
							let error_number = match e {
								DispatchError::Module(ModuleError { error, .. }) => error[0],
								_ => 0, // this case should never happen because an Error is always converted to DispatchError::Module(ModuleError)
							};
							InvalidTransaction::Custom(error_number).into()
						}
					}
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_currency(who: &T::AccountId) -> AssetIdOf<T>
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

/// Implements the transaction payment for native as well as non-native currencies
pub struct TransferFees<MC, DF, FR>(PhantomData<(MC, DF, FR)>);

impl<T, MC, DF, FR> OnChargeTransaction<T> for TransferFees<MC, DF, FR>
where
	T: Config + pallet_utility::Config,
	MC: MultiCurrency<<T as frame_system::Config>::AccountId>,
	AssetIdOf<T>: Into<MC::CurrencyId>,
	MC::Balance: FixedPointOperand,
	FR: Get<T::AccountId>,
	DF: DepositFee<T::AccountId, MC::CurrencyId, MC::Balance>,
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>> + IsSubType<pallet_utility::pallet::Call<T>>,
	<T as pallet_utility::Config>::RuntimeCall: IsSubType<Call<T>>,
	BalanceOf<T>: FixedPointOperand,
	BalanceOf<T>: From<MC::Balance>,
{
	type LiquidityInfo = Option<PaymentInfo<Self::Balance, AssetIdOf<T>, Price>>;
	type Balance = <MC as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Withdraw the predicted fee from the transaction origin.
	///
	/// Note: The `fee` already includes the `tip`.
	fn withdraw_fee(
		who: &T::AccountId,
		call: &<T as frame_system::Config>::RuntimeCall,
		_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		fee: Self::Balance,
		_tip: Self::Balance,
	) -> Result<Self::LiquidityInfo, TransactionValidityError> {
		if fee.is_zero() {
			return Ok(None);
		}

		let currency = if let Some(Call::set_currency { currency }) = call.is_sub_type() {
			*currency
		} else if let Some(pallet_utility::pallet::Call::batch { calls })
		| Some(pallet_utility::pallet::Call::batch_all { calls })
		| Some(pallet_utility::pallet::Call::force_batch { calls }) = call.is_sub_type()
		{
			// `calls` can be empty Vec
			match calls.first() {
				Some(first_call) => match first_call.is_sub_type() {
					Some(Call::set_currency { currency }) => *currency,
					_ => Pallet::<T>::account_currency(who),
				},
				_ => Pallet::<T>::account_currency(who),
			}
		} else {
			Pallet::<T>::account_currency(who)
		};

		let (converted_fee, currency, price) = if T::SwappablePaymentAssetSupport::is_transaction_fee_currency(currency)
		{
			let price = Pallet::<T>::get_currency_price(currency)
				.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			let converted_fee = convert_fee_with_price(fee, price)
				.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			(converted_fee, currency, price)
		} else {
			//In case of insufficient asset we buy DOT with insufficient asset, and using that DOT and amount as fee currency
			let dot_hdx_price = Pallet::<T>::get_currency_price(T::PolkadotNativeAssetId::get())
				.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			let fee_in_dot = convert_fee_with_price(fee, dot_hdx_price)
				.ok_or(TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			let amount_in = T::SwappablePaymentAssetSupport::calculate_in_given_out(
				currency,
				T::PolkadotNativeAssetId::get(),
				fee_in_dot.into(),
			)
			.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			let pool_fee = T::SwappablePaymentAssetSupport::calculate_fee_amount(amount_in)
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			let max_limit = amount_in.saturating_add(pool_fee);

			T::SwappablePaymentAssetSupport::buy(
				who,
				currency,
				T::PolkadotNativeAssetId::get(),
				fee_in_dot.into(),
				max_limit,
				who,
			)
			.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;

			(fee_in_dot, T::PolkadotNativeAssetId::get(), dot_hdx_price)
		};

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
		_dispatch_info: &DispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
		_post_info: &PostDispatchInfoOf<<T as frame_system::Config>::RuntimeCall>,
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
/// In the else statement, first we try to get the price from cache, otherwise we calculate it
/// The price calculation based on onchain-route is mainly used by EVM dry run as in the dry run we dont have storage filled with prices, so calculation is needed
impl<T: Config> NativePriceOracle<AssetIdOf<T>, Price> for Pallet<T> {
	fn price(currency: AssetIdOf<T>) -> Option<Price> {
		if currency == T::NativeAssetId::get() {
			Some(Price::one())
		} else {
			Pallet::<T>::currency_price(currency).or_else(|| Self::get_oracle_price(currency, T::NativeAssetId::get()))
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

/// Provides account's fee payment asset or default fee asset ( Native asset )
impl<T: Config> AccountFeeCurrency<T::AccountId> for Pallet<T> {
	type AssetId = AssetIdOf<T>;

	fn get(who: &T::AccountId) -> Self::AssetId {
		Pallet::<T>::account_currency(who)
	}
}

pub struct TryCallCurrency<T>(PhantomData<T>);
impl<T> TryConvert<&<T as frame_system::Config>::RuntimeCall, AssetIdOf<T>> for TryCallCurrency<T>
where
	T: Config + pallet_utility::Config,
	<T as frame_system::Config>::RuntimeCall: IsSubType<Call<T>> + IsSubType<pallet_utility::pallet::Call<T>>,
	<T as pallet_utility::Config>::RuntimeCall: IsSubType<Call<T>>,
{
	fn try_convert(
		call: &<T as frame_system::Config>::RuntimeCall,
	) -> Result<AssetIdOf<T>, &<T as frame_system::Config>::RuntimeCall> {
		if let Some(crate::pallet::Call::set_currency { currency }) = call.is_sub_type() {
			Ok(*currency)
		} else if let Some(pallet_utility::pallet::Call::batch { calls })
		| Some(pallet_utility::pallet::Call::batch_all { calls })
		| Some(pallet_utility::pallet::Call::force_batch { calls }) = call.is_sub_type()
		{
			// `calls` can be empty Vec
			match calls.first() {
				Some(first_call) => match first_call.is_sub_type() {
					Some(crate::pallet::Call::set_currency { currency }) => Ok(*currency),
					_ => Err(call),
				},
				_ => Err(call),
			}
		} else {
			Err(call)
		}
	}
}

pub struct NoCallCurrency<T>(PhantomData<T>);
impl<T: Config> TryConvert<&<T as frame_system::Config>::RuntimeCall, AssetIdOf<T>> for NoCallCurrency<T> {
	fn try_convert(
		call: &<T as frame_system::Config>::RuntimeCall,
	) -> Result<AssetIdOf<T>, &<T as frame_system::Config>::RuntimeCall> {
		Err(call)
	}
}
