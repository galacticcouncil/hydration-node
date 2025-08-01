//! # Currencies Module
//!
//! ## Overview
//!
//! The currencies module provides a mixed currencies system, by configuring a
//! native currency which implements `BasicCurrencyExtended`, and a
//! multi-currency which implements `MultiCurrency`.
//!
//! It also provides an adapter, to adapt `frame_support::traits::Currency`
//! implementations into `BasicCurrencyExtended`.
//!
//! The currencies module provides functionality of both `MultiCurrencyExtended`
//! and `BasicCurrencyExtended`, via unified interfaces, and all calls would be
//! delegated to the underlying multi-currency and base currency system.
//! A native currency ID could be set by `Config::GetNativeCurrencyId`, to
//! identify the native currency.
//!
//! ### Implementations
//!
//! The currencies module provides implementations for following traits.
//!
//! - `MultiCurrency` - Abstraction over a fungible multi-currency system.
//! - `MultiCurrencyExtended` - Extended `MultiCurrency` with additional helper
//!   types and methods, like updating balance
//!   by a given signed integer amount.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `transfer` - Transfer some balance to another account, in a given
//!   currency.
//! - `transfer_native_currency` - Transfer some balance to another account, in
//!   native currency set in
//!   `Config::NativeCurrency`.
//! - `update_balance` - Update balance by signed integer amount, in a given
//!   currency, root origin required.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::manual_inspect)]

use codec::Codec;
use frame_support::{
	fail,
	pallet_prelude::*,
	traits::{
		Currency as PalletCurrency, ExistenceRequirement, Get, Imbalance, LockableCurrency as PalletLockableCurrency,
		NamedReservableCurrency as PalletNamedReservableCurrency, ReservableCurrency as PalletReservableCurrency,
		WithdrawReasons,
	},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::{AssetKind, BoundErc20};
use orml_traits::{
	arithmetic::{Signed, SimpleArithmetic},
	currency::TransferAll,
	BalanceStatus, BasicCurrency, BasicCurrencyExtended, BasicLockableCurrency, BasicReservableCurrency, GetByKey,
	LockIdentifier, MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
	NamedBasicReservableCurrency, NamedMultiReservableCurrency,
};
use orml_utilities::with_transaction_result;
use sp_runtime::{
	traits::{CheckedSub, MaybeSerializeDeserialize, StaticLookup, Zero},
	DispatchError, DispatchResult, Saturating,
};
use sp_std::vec::Vec;
use sp_std::{fmt::Debug, marker, result};

pub mod fungibles;
mod mock;
mod tests;
mod tests_fungibles;
mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub(crate) type BalanceOf<T> =
		<<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
	pub(crate) type CurrencyIdOf<T> =
		<<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
	pub(crate) type AmountOf<T> =
		<<T as Config>::MultiCurrency as MultiCurrencyExtended<<T as frame_system::Config>::AccountId>>::Amount;
	pub(crate) type ReserveIdentifierOf<T> = <<T as Config>::MultiCurrency as NamedMultiReservableCurrency<
		<T as frame_system::Config>::AccountId,
	>>::ReserveIdentifier;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type MultiCurrency: TransferAll<Self::AccountId>
			+ MultiCurrencyExtended<Self::AccountId>
			+ MultiLockableCurrency<Self::AccountId>
			+ MultiReservableCurrency<Self::AccountId>
			+ NamedMultiReservableCurrency<Self::AccountId>;

		type NativeCurrency: BasicCurrencyExtended<Self::AccountId, Balance = BalanceOf<Self>, Amount = AmountOf<Self>>
			+ BasicLockableCurrency<Self::AccountId, Balance = BalanceOf<Self>>
			+ BasicReservableCurrency<Self::AccountId, Balance = BalanceOf<Self>>
			+ NamedBasicReservableCurrency<Self::AccountId, ReserveIdentifierOf<Self>, Balance = BalanceOf<Self>>;

		type Erc20Currency: MultiCurrency<Self::AccountId, CurrencyId = EvmAddress, Balance = BalanceOf<Self>>;

		type BoundErc20: BoundErc20<AssetId = CurrencyIdOf<Self>>;

		#[pallet::constant]
		type ReserveAccount: Get<Self::AccountId>;

		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Unable to convert the Amount type into Balance.
		AmountIntoBalanceFailed,
		/// Balance is too low.
		BalanceTooLow,
		/// Deposit result is not expected
		DepositFailed,
		/// Operation is not supported for this currency
		NotSupported,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Currency transfer success.
		Transferred {
			currency_id: CurrencyIdOf<T>,
			from: T::AccountId,
			to: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Update balance success.
		BalanceUpdated {
			currency_id: CurrencyIdOf<T>,
			who: T::AccountId,
			amount: AmountOf<T>,
		},
		/// Deposit success.
		Deposited {
			currency_id: CurrencyIdOf<T>,
			who: T::AccountId,
			amount: BalanceOf<T>,
		},
		/// Withdraw success.
		Withdrawn {
			currency_id: CurrencyIdOf<T>,
			who: T::AccountId,
			amount: BalanceOf<T>,
		},
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer some balance to another account under `currency_id`.
		///
		/// The dispatch origin for this call must be `Signed` by the
		/// transactor.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::transfer_non_native_currency())]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			<Self as MultiCurrency<T::AccountId>>::transfer(currency_id, &from, &to, amount)?;
			Ok(())
		}

		/// Transfer some native currency to another account.
		///
		/// The dispatch origin for this call must be `Signed` by the
		/// transactor.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::transfer_native_currency())]
		pub fn transfer_native_currency(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			T::NativeCurrency::transfer(&from, &to, amount)?;

			Self::deposit_event(Event::Transferred {
				currency_id: T::GetNativeCurrencyId::get(),
				from,
				to,
				amount,
			});
			Ok(())
		}

		/// update amount of account `who` under `currency_id`.
		///
		/// The dispatch origin of this call must be _Root_.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::update_balance_non_native_currency())]
		pub fn update_balance(
			origin: OriginFor<T>,
			who: <T::Lookup as StaticLookup>::Source,
			currency_id: CurrencyIdOf<T>,
			amount: AmountOf<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			let dest = T::Lookup::lookup(who)?;
			<Self as MultiCurrencyExtended<T::AccountId>>::update_balance(currency_id, &dest, amount)?;
			Ok(())
		}
	}
}

impl<T: Config> MultiCurrency<T::AccountId> for Pallet<T> {
	type CurrencyId = CurrencyIdOf<T>;
	type Balance = BalanceOf<T>;

	fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::minimum_balance()
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::minimum_balance(contract),
				None => T::MultiCurrency::minimum_balance(currency_id),
			}
		}
	}

	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::total_issuance()
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::total_issuance(contract),
				None => T::MultiCurrency::total_issuance(currency_id),
			}
		}
	}

	fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::total_balance(who)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::total_balance(contract, who),
				None => T::MultiCurrency::total_balance(currency_id, who),
			}
		}
	}

	fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::free_balance(who)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::free_balance(contract, who),
				None => T::MultiCurrency::free_balance(currency_id, who),
			}
		}
	}

	fn ensure_can_withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::ensure_can_withdraw(who, amount)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::ensure_can_withdraw(contract, who, amount),
				None => T::MultiCurrency::ensure_can_withdraw(currency_id, who, amount),
			}
		}
	}

	fn transfer(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if amount.is_zero() || from == to {
			return Ok(());
		}
		#[cfg(any(feature = "try-runtime", test))]
		let (initial_source_balance, initial_dest_balance) = {
			(
				Self::total_balance(currency_id, from),
				Self::total_balance(currency_id, to),
			)
		};

		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::transfer(from, to, amount)?;
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::transfer(contract, from, to, amount)?,
				None => T::MultiCurrency::transfer(currency_id, from, to, amount)?,
			};
		}
		Self::deposit_event(Event::Transferred {
			currency_id,
			from: from.clone(),
			to: to.clone(),
			amount,
		});
		#[cfg(any(feature = "try-runtime", test))]
		{
			let (final_source_balance, final_dest_balance) = {
				(
					Self::total_balance(currency_id, from),
					Self::total_balance(currency_id, to),
				)
			};
			let amount_sent = initial_source_balance - final_source_balance;
			debug_assert_eq!(amount_sent, amount, "Transfer - source sent incorrect amount");
			debug_assert_eq!(
				initial_dest_balance + amount,
				final_dest_balance,
				"Transfer - dest received incorrect amount"
			);
		}
		Ok(())
	}

	fn deposit(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::deposit(who, amount)?;
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::deposit(contract, who, amount)?,
				None => T::MultiCurrency::deposit(currency_id, who, amount)?,
			}
		}
		Self::deposit_event(Event::Deposited {
			currency_id,
			who: who.clone(),
			amount,
		});
		Ok(())
	}

	fn withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(());
		}
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::withdraw(who, amount)?;
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::withdraw(contract, who, amount)?,
				None => T::MultiCurrency::withdraw(currency_id, who, amount)?,
			}
		}
		Self::deposit_event(Event::Withdrawn {
			currency_id,
			who: who.clone(),
			amount,
		});
		Ok(())
	}

	fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::can_slash(who, amount)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::can_slash(contract, who, amount),
				None => T::MultiCurrency::can_slash(currency_id, who, amount),
			}
		}
	}

	fn slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::slash(who, amount)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => T::Erc20Currency::slash(contract, who, amount),
				None => T::MultiCurrency::slash(currency_id, who, amount),
			}
		}
	}
}

impl<T: Config> MultiCurrencyExtended<T::AccountId> for Pallet<T> {
	type Amount = AmountOf<T>;

	fn update_balance(currency_id: Self::CurrencyId, who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::update_balance(who, by_amount)?;
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::update_balance(currency_id, who, by_amount)?,
			}
		}
		Self::deposit_event(Event::BalanceUpdated {
			currency_id,
			who: who.clone(),
			amount: by_amount,
		});
		Ok(())
	}
}

impl<T: Config> MultiLockableCurrency<T::AccountId> for Pallet<T> {
	type Moment = BlockNumberFor<T>;

	fn set_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::set_lock(lock_id, who, amount)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::set_lock(lock_id, currency_id, who, amount),
			}
		}
	}

	fn extend_lock(
		lock_id: LockIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::extend_lock(lock_id, who, amount)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::extend_lock(lock_id, currency_id, who, amount),
			}
		}
	}

	fn remove_lock(lock_id: LockIdentifier, currency_id: Self::CurrencyId, who: &T::AccountId) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::remove_lock(lock_id, who)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::remove_lock(lock_id, currency_id, who),
			}
		}
	}
}

impl<T: Config> MultiReservableCurrency<T::AccountId> for Pallet<T> {
	fn can_reserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> bool {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::can_reserve(who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => false,
				None => T::MultiCurrency::can_reserve(currency_id, who, value),
			}
		}
	}

	fn slash_reserved(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::slash_reserved(who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => Default::default(),
				None => T::MultiCurrency::slash_reserved(currency_id, who, value),
			}
		}
	}

	fn reserved_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserved_balance(who)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => Default::default(),
				None => T::MultiCurrency::reserved_balance(currency_id, who),
			}
		}
	}

	fn reserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserve(who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::reserve(currency_id, who, value),
			}
		}
	}

	fn unreserve(currency_id: Self::CurrencyId, who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::unreserve(who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => value,
				None => T::MultiCurrency::unreserve(currency_id, who, value),
			}
		}
	}

	fn repatriate_reserved(
		currency_id: Self::CurrencyId,
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::repatriate_reserved(slashed, beneficiary, value, status)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => T::MultiCurrency::repatriate_reserved(currency_id, slashed, beneficiary, value, status),
			}
		}
	}
}

impl<T: Config> NamedMultiReservableCurrency<T::AccountId> for Pallet<T> {
	type ReserveIdentifier = ReserveIdentifierOf<T>;

	fn slash_reserved_named(
		id: &Self::ReserveIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::slash_reserved_named(id, who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => Default::default(),
				None => T::MultiCurrency::slash_reserved_named(id, currency_id, who, value),
			}
		}
	}

	fn reserved_balance_named(
		id: &Self::ReserveIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserved_balance_named(id, who)
		} else {
			T::MultiCurrency::reserved_balance_named(id, currency_id, who)
		}
	}

	fn reserve_named(
		id: &Self::ReserveIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> DispatchResult {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::reserve_named(id, who, value)
		} else {
			if let Some(contract) = T::BoundErc20::contract_address(currency_id) {
				T::Erc20Currency::transfer(contract, who, &T::ReserveAccount::get(), value)?;
				T::MultiCurrency::deposit(currency_id, who, value)?;
			}
			T::MultiCurrency::reserve_named(id, currency_id, who, value)
		}
	}

	fn unreserve_named(
		id: &Self::ReserveIdentifier,
		currency_id: Self::CurrencyId,
		who: &T::AccountId,
		value: Self::Balance,
	) -> Self::Balance {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::unreserve_named(id, who, value)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(contract) => with_transaction_result::<Self::Balance>(|| {
					let remaining = T::MultiCurrency::unreserve_named(id, currency_id, who, value);
					let unreserved = value.saturating_sub(remaining);
					if unreserved > Zero::zero() {
						T::MultiCurrency::withdraw(currency_id, who, unreserved)?;
						T::Erc20Currency::transfer(contract, &T::ReserveAccount::get(), who, unreserved)?;
					}
					Ok(remaining)
				})
				.unwrap_or(value),
				None => T::MultiCurrency::unreserve_named(id, currency_id, who, value),
			}
		}
	}

	fn repatriate_reserved_named(
		id: &Self::ReserveIdentifier,
		currency_id: Self::CurrencyId,
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		if currency_id == T::GetNativeCurrencyId::get() {
			T::NativeCurrency::repatriate_reserved_named(id, slashed, beneficiary, value, status)
		} else {
			match T::BoundErc20::contract_address(currency_id) {
				Some(_) => fail!(Error::<T>::NotSupported),
				None => {
					T::MultiCurrency::repatriate_reserved_named(id, currency_id, slashed, beneficiary, value, status)
				}
			}
		}
	}
}

pub struct Currency<T, GetCurrencyId>(marker::PhantomData<T>, marker::PhantomData<GetCurrencyId>);

impl<T, GetCurrencyId> BasicCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Balance = BalanceOf<T>;

	fn minimum_balance() -> Self::Balance {
		<Pallet<T>>::minimum_balance(GetCurrencyId::get())
	}

	fn total_issuance() -> Self::Balance {
		<Pallet<T>>::total_issuance(GetCurrencyId::get())
	}

	fn total_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::total_balance(GetCurrencyId::get(), who)
	}

	fn free_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::free_balance(GetCurrencyId::get(), who)
	}

	fn ensure_can_withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::ensure_can_withdraw(GetCurrencyId::get(), who, amount)
	}

	fn transfer(from: &T::AccountId, to: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiCurrency<T::AccountId>>::transfer(GetCurrencyId::get(), from, to, amount)
	}

	fn deposit(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::deposit(GetCurrencyId::get(), who, amount)
	}

	fn withdraw(who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T>>::withdraw(GetCurrencyId::get(), who, amount)
	}

	fn can_slash(who: &T::AccountId, amount: Self::Balance) -> bool {
		<Pallet<T>>::can_slash(GetCurrencyId::get(), who, amount)
	}

	fn slash(who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		<Pallet<T>>::slash(GetCurrencyId::get(), who, amount)
	}
}

impl<T, GetCurrencyId> BasicCurrencyExtended<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Amount = AmountOf<T>;

	fn update_balance(who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		<Pallet<T> as MultiCurrencyExtended<T::AccountId>>::update_balance(GetCurrencyId::get(), who, by_amount)
	}
}

impl<T, GetCurrencyId> BasicLockableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	type Moment = BlockNumberFor<T>;

	fn set_lock(lock_id: LockIdentifier, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::set_lock(lock_id, GetCurrencyId::get(), who, amount)
	}

	fn extend_lock(lock_id: LockIdentifier, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::extend_lock(lock_id, GetCurrencyId::get(), who, amount)
	}

	fn remove_lock(lock_id: LockIdentifier, who: &T::AccountId) -> DispatchResult {
		<Pallet<T> as MultiLockableCurrency<T::AccountId>>::remove_lock(lock_id, GetCurrencyId::get(), who)
	}
}

impl<T, GetCurrencyId> BasicReservableCurrency<T::AccountId> for Currency<T, GetCurrencyId>
where
	T: Config,
	GetCurrencyId: Get<CurrencyIdOf<T>>,
{
	fn can_reserve(who: &T::AccountId, value: Self::Balance) -> bool {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::can_reserve(GetCurrencyId::get(), who, value)
	}

	fn slash_reserved(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::slash_reserved(GetCurrencyId::get(), who, value)
	}

	fn reserved_balance(who: &T::AccountId) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserved_balance(GetCurrencyId::get(), who)
	}

	fn reserve(who: &T::AccountId, value: Self::Balance) -> DispatchResult {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::reserve(GetCurrencyId::get(), who, value)
	}

	fn unreserve(who: &T::AccountId, value: Self::Balance) -> Self::Balance {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::unreserve(GetCurrencyId::get(), who, value)
	}

	fn repatriate_reserved(
		slashed: &T::AccountId,
		beneficiary: &T::AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		<Pallet<T> as MultiReservableCurrency<T::AccountId>>::repatriate_reserved(
			GetCurrencyId::get(),
			slashed,
			beneficiary,
			value,
			status,
		)
	}
}

pub type NativeCurrencyOf<T> = Currency<T, <T as Config>::GetNativeCurrencyId>;

/// Adapt other currency traits implementation to `BasicCurrency`.
pub struct BasicCurrencyAdapter<T, Currency, Amount, Moment>(marker::PhantomData<(T, Currency, Amount, Moment)>);

type PalletBalanceOf<A, Currency> = <Currency as PalletCurrency<A>>::Balance;

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletCurrency<AccountId>,
	T: Config,
{
	type Balance = PalletBalanceOf<AccountId, Currency>;

	fn minimum_balance() -> Self::Balance {
		Currency::minimum_balance()
	}

	fn total_issuance() -> Self::Balance {
		Currency::total_issuance()
	}

	fn total_balance(who: &AccountId) -> Self::Balance {
		Currency::total_balance(who)
	}

	fn free_balance(who: &AccountId) -> Self::Balance {
		Currency::free_balance(who)
	}

	fn ensure_can_withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		let new_balance = Self::free_balance(who)
			.checked_sub(&amount)
			.ok_or(Error::<T>::BalanceTooLow)?;

		Currency::ensure_can_withdraw(who, amount, WithdrawReasons::all(), new_balance)
	}

	fn transfer(from: &AccountId, to: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::transfer(from, to, amount, ExistenceRequirement::AllowDeath)
	}

	fn deposit(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		if !amount.is_zero() {
			let deposit_result = Currency::deposit_creating(who, amount);
			let actual_deposit = deposit_result.peek();
			ensure!(actual_deposit == amount, Error::<T>::DepositFailed);
		}

		Ok(())
	}

	fn withdraw(who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::withdraw(who, amount, WithdrawReasons::all(), ExistenceRequirement::AllowDeath).map(|_| ())
	}

	fn can_slash(who: &AccountId, amount: Self::Balance) -> bool {
		Currency::can_slash(who, amount)
	}

	fn slash(who: &AccountId, amount: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash(who, amount);
		gap
	}
}

// Adapt `frame_support::traits::Currency`
impl<T, AccountId, Currency, Amount, Moment> BasicCurrencyExtended<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Amount: Signed
		+ TryInto<PalletBalanceOf<AccountId, Currency>>
		+ TryFrom<PalletBalanceOf<AccountId, Currency>>
		+ SimpleArithmetic
		+ Codec
		+ Copy
		+ MaybeSerializeDeserialize
		+ Debug
		+ Default
		+ codec::MaxEncodedLen,
	Currency: PalletCurrency<AccountId>,
	T: Config,
{
	type Amount = Amount;

	fn update_balance(who: &AccountId, by_amount: Self::Amount) -> DispatchResult {
		let by_balance = by_amount
			.abs()
			.try_into()
			.map_err(|_| Error::<T>::AmountIntoBalanceFailed)?;
		if by_amount.is_positive() {
			Self::deposit(who, by_balance)
		} else {
			Self::withdraw(who, by_balance)
		}
	}
}

// Adapt `frame_support::traits::LockableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicLockableCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletLockableCurrency<AccountId>,
	T: Config,
{
	type Moment = Moment;

	fn set_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::set_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn extend_lock(lock_id: LockIdentifier, who: &AccountId, amount: Self::Balance) -> DispatchResult {
		Currency::extend_lock(lock_id, who, amount, WithdrawReasons::all());
		Ok(())
	}

	fn remove_lock(lock_id: LockIdentifier, who: &AccountId) -> DispatchResult {
		Currency::remove_lock(lock_id, who);
		Ok(())
	}
}

// Adapt `frame_support::traits::ReservableCurrency`
impl<T, AccountId, Currency, Amount, Moment> BasicReservableCurrency<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletReservableCurrency<AccountId>,
	T: Config,
{
	fn can_reserve(who: &AccountId, value: Self::Balance) -> bool {
		Currency::can_reserve(who, value)
	}

	fn slash_reserved(who: &AccountId, value: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash_reserved(who, value);
		gap
	}

	fn reserved_balance(who: &AccountId) -> Self::Balance {
		Currency::reserved_balance(who)
	}

	fn reserve(who: &AccountId, value: Self::Balance) -> DispatchResult {
		Currency::reserve(who, value)
	}

	fn unreserve(who: &AccountId, value: Self::Balance) -> Self::Balance {
		Currency::unreserve(who, value)
	}

	fn repatriate_reserved(
		slashed: &AccountId,
		beneficiary: &AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		Currency::repatriate_reserved(slashed, beneficiary, value, status)
	}
}

// Adapt `frame_support::traits::NamedReservableCurrency`
impl<T, AccountId, Currency, Amount, Moment, ReserveIdentifier>
	NamedBasicReservableCurrency<AccountId, ReserveIdentifier> for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: PalletNamedReservableCurrency<AccountId, ReserveIdentifier = ReserveIdentifier>,
	T: Config,
{
	fn slash_reserved_named(id: &ReserveIdentifier, who: &AccountId, value: Self::Balance) -> Self::Balance {
		let (_, gap) = Currency::slash_reserved_named(id, who, value);
		gap
	}

	fn reserved_balance_named(id: &ReserveIdentifier, who: &AccountId) -> Self::Balance {
		Currency::reserved_balance_named(id, who)
	}

	fn reserve_named(id: &ReserveIdentifier, who: &AccountId, value: Self::Balance) -> DispatchResult {
		Currency::reserve_named(id, who, value)
	}

	fn unreserve_named(id: &ReserveIdentifier, who: &AccountId, value: Self::Balance) -> Self::Balance {
		Currency::unreserve_named(id, who, value)
	}

	fn repatriate_reserved_named(
		id: &ReserveIdentifier,
		slashed: &AccountId,
		beneficiary: &AccountId,
		value: Self::Balance,
		status: BalanceStatus,
	) -> result::Result<Self::Balance, DispatchError> {
		Currency::repatriate_reserved_named(id, slashed, beneficiary, value, status)
	}
}

pub struct AssetTotalIssuance<T>(PhantomData<T>);

impl<T: Config> GetByKey<CurrencyIdOf<T>, BalanceOf<T>> for AssetTotalIssuance<T> {
	fn get(currency_id: &CurrencyIdOf<T>) -> BalanceOf<T> {
		Pallet::<T>::total_issuance(*currency_id)
	}
}

impl<T: Config> TransferAll<T::AccountId> for Pallet<T> {
	fn transfer_all(source: &T::AccountId, dest: &T::AccountId) -> DispatchResult {
		with_transaction_result(|| {
			// transfer non-native free to dest
			T::MultiCurrency::transfer_all(source, dest)?;

			// transfer all free to dest
			T::NativeCurrency::transfer(source, dest, T::NativeCurrency::free_balance(source))
		})
	}
}

use frame_support::traits::fungible::{Dust, Inspect, Mutate, Unbalanced};
use frame_support::traits::tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence};

impl<T: Config, AccountId, Currency, Amount, Moment> Inspect<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: Inspect<AccountId>,
{
	type Balance = <Currency as Inspect<AccountId>>::Balance;

	fn total_issuance() -> Self::Balance {
		<Currency as Inspect<AccountId>>::total_issuance()
	}

	fn minimum_balance() -> Self::Balance {
		<Currency as Inspect<AccountId>>::minimum_balance()
	}

	fn total_balance(who: &AccountId) -> Self::Balance {
		<Currency as Inspect<AccountId>>::total_balance(who)
	}

	fn balance(who: &AccountId) -> Self::Balance {
		<Currency as Inspect<AccountId>>::balance(who)
	}

	fn reducible_balance(who: &AccountId, preservation: Preservation, force: Fortitude) -> Self::Balance {
		<Currency as Inspect<AccountId>>::reducible_balance(who, preservation, force)
	}

	fn can_deposit(who: &AccountId, amount: Self::Balance, provenance: Provenance) -> DepositConsequence {
		<Currency as Inspect<AccountId>>::can_deposit(who, amount, provenance)
	}

	fn can_withdraw(who: &AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
		<Currency as Inspect<AccountId>>::can_withdraw(who, amount)
	}
}

impl<T: Config, AccountId, Currency, Amount, Moment> Unbalanced<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: Mutate<AccountId>,
	AccountId: Eq,
{
	fn handle_dust(dust: Dust<AccountId, Self>) {
		<Currency as Unbalanced<AccountId>>::handle_dust(Dust(dust.0))
	}

	fn write_balance(who: &AccountId, amount: Self::Balance) -> Result<Option<Self::Balance>, DispatchError> {
		<Currency as Unbalanced<AccountId>>::write_balance(who, amount)
	}

	fn set_total_issuance(amount: Self::Balance) {
		<Currency as Unbalanced<AccountId>>::set_total_issuance(amount)
	}

	fn deactivate(amount: Self::Balance) {
		<Currency as Unbalanced<AccountId>>::deactivate(amount)
	}

	fn reactivate(amount: Self::Balance) {
		<Currency as Unbalanced<AccountId>>::reactivate(amount)
	}
}

impl<T: Config, AccountId, Currency, Amount, Moment> Mutate<AccountId>
	for BasicCurrencyAdapter<T, Currency, Amount, Moment>
where
	Currency: Mutate<AccountId>,
	AccountId: Eq,
{
	fn done_mint_into(who: &AccountId, amount: Self::Balance) {
		<Currency as Mutate<AccountId>>::done_mint_into(who, amount)
	}

	fn done_burn_from(who: &AccountId, amount: Self::Balance) {
		<Currency as Mutate<AccountId>>::done_burn_from(who, amount)
	}

	fn done_shelve(who: &AccountId, amount: Self::Balance) {
		<Currency as Mutate<AccountId>>::done_shelve(who, amount)
	}

	fn done_restore(who: &AccountId, amount: Self::Balance) {
		<Currency as Mutate<AccountId>>::done_restore(who, amount)
	}

	fn done_transfer(source: &AccountId, dest: &AccountId, amount: Self::Balance) {
		<Currency as Mutate<AccountId>>::done_transfer(source, dest, amount)
	}
}

pub struct MockErc20Currency<T>(PhantomData<T>);
impl<T: Config> MultiCurrency<T::AccountId> for MockErc20Currency<T> {
	type CurrencyId = EvmAddress;
	type Balance = BalanceOf<T>;

	fn minimum_balance(_currency_id: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}

	fn total_issuance(_currency_id: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}

	fn total_balance(_currency_id: Self::CurrencyId, _who: &T::AccountId) -> Self::Balance {
		Default::default()
	}

	fn free_balance(_currency_id: Self::CurrencyId, _who: &T::AccountId) -> Self::Balance {
		Default::default()
	}

	fn ensure_can_withdraw(
		_currency_id: Self::CurrencyId,
		_who: &T::AccountId,
		_amount: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn transfer(
		_currency_id: Self::CurrencyId,
		_from: &T::AccountId,
		_to: &T::AccountId,
		_amount: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn deposit(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> DispatchResult {
		Ok(())
	}

	fn withdraw(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> DispatchResult {
		Ok(())
	}

	fn can_slash(_currency_id: Self::CurrencyId, _who: &T::AccountId, _value: Self::Balance) -> bool {
		false
	}

	fn slash(_currency_id: Self::CurrencyId, _who: &T::AccountId, _amount: Self::Balance) -> Self::Balance {
		Default::default()
	}
}

pub struct MockBoundErc20<T>(PhantomData<T>);
impl<T: Config> hydradx_traits::Inspect for MockBoundErc20<T> {
	type AssetId = CurrencyIdOf<T>;
	type Location = ();

	fn is_sufficient(_id: Self::AssetId) -> bool {
		false
	}

	fn exists(_id: Self::AssetId) -> bool {
		false
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		None
	}

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		None
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		false
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		None
	}
}

impl<T: Config> BoundErc20 for MockBoundErc20<T> {
	fn contract_address(_id: Self::AssetId) -> Option<EvmAddress> {
		None
	}
}
