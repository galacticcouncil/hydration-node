#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	traits::{Currency, ExistenceRequirement, Get, Imbalance, OnUnbalanced, WithdrawReasons},
};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{DispatchInfoOf, PostDispatchInfoOf, Saturating, Zero},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use sp_std::prelude::*;

use pallet_transaction_payment::OnChargeTransaction;
use sp_std::marker::PhantomData;

use frame_support::weights::Pays;
use frame_support::weights::Weight;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::traits::{CurrencySwap, AMM};
use primitives::{AssetId, Balance, CORE_ASSET_ID};

type NegativeImbalanceOf<C, T> = <C as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub trait WeightInfo {
	fn set_currency() -> Weight;
	fn swap_currency() -> Weight;
}

pub trait Config: frame_system::Config + pallet_transaction_payment::Config {
	/// Because this pallet emits events, it depends on the runtime's definition of an event.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;

	/// The currency type in which fees will be paid.
	type Currency: Currency<Self::AccountId> + Send + Sync;

	/// Multi Currency
	type MultiCurrency: MultiCurrency<Self::AccountId>
		+ MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;

	/// AMM pool to swap for native currency
	type AMMPool: AMM<Self::AccountId, AssetId, Balance>;

	/// Accepted Non native list of currencies
	type NonNativeAcceptedAssetId: Get<Vec<AssetId>>;

	/// Weight information for the extrinsics.
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
	{
		/// CurrencySet
		/// [who, currency]
		CurrectSet(AccountId, AssetId),
	}
);

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Config> {
		/// Selected currency is not supported
		UnsupportedCurrency,

		/// Zero Balance of selected currency
		ZeroBalance,
	}
}

decl_storage! {
	trait Store for Module<T: Config> as TransactionPayment {
		/// Account currency map
		pub AccountCurrencyMap get(fn get_currency): map hasher(blake2_128_concat) T::AccountId => Option<AssetId>;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		// Errors must be initialized if they are used by the pallet.
		type Error = Error<T>;

		// Events must be initialized if they are used by the pallet.
		fn deposit_event() = default;

		/// Set currency in which transaction fees are paid.
		/// This is feeless transaction.
		/// Selected currency must have non-zero balance otherwise is not allowed to be set.
		#[weight = (<T as Config>::WeightInfo::set_currency(), Pays::No)]
		pub fn set_currency(
			origin,
			currency: AssetId,
		)  -> DispatchResult {
			let who = ensure_signed(origin)?;

			match currency == CORE_ASSET_ID || T::NonNativeAcceptedAssetId::get().contains(&currency){
				true =>	{
					if T::MultiCurrency::free_balance(currency, &who) == Balance::zero(){
						return Err(Error::<T>::ZeroBalance.into());
					}

					<AccountCurrencyMap<T>>::insert(who.clone(), currency);

					Self::deposit_event(RawEvent::CurrectSet(who, currency));

					Ok(())
				},
				false => Err(Error::<T>::UnsupportedCurrency.into())
			}
		}
	}
}
impl<T: Config> Module<T> {
	pub fn swap_currency(who: &T::AccountId, fee: Balance) -> DispatchResult {
		// Let's determine currency in which user would like to pay the fee
		let fee_currency = match Module::<T>::get_currency(who) {
			Some(c) => c,
			_ => CORE_ASSET_ID,
		};

		// If not native currency, let's buy CORE asset first and then pay with that.
		if fee_currency != CORE_ASSET_ID {
			T::AMMPool::buy(&who, CORE_ASSET_ID, fee_currency, fee, 2u128 * fee, false)?;
		}

		Ok(())
	}
}

impl<T: Config> CurrencySwap<<T as frame_system::Config>::AccountId, Balance> for Module<T> {
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
				.map_err(|_| TransactionValidityError::Invalid(InvalidTransaction::Payment))?;
			// Call someone else to handle the imbalance (fee and tip separately)
			let imbalances = adjusted_paid.split(tip);
			OU::on_unbalanceds(Some(imbalances.0).into_iter().chain(Some(imbalances.1)));
		}
		Ok(())
	}
}
