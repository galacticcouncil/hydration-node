#![cfg_attr(not(feature = "std"), no_std)]

mod default_weights;

use frame_support::{
	decl_error, decl_module, decl_storage,
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
use primitives::traits::AMM;
use primitives::{AssetId, Balance, CORE_ASSET_ID};

type NegativeImbalanceOf<C, T> = <C as Currency<<T as frame_system::Trait>::AccountId>>::NegativeImbalance;

pub trait WeightInfo {
	fn set_currency() -> Weight;
}

pub trait Trait: frame_system::Trait + pallet_transaction_payment::Trait {
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

// The pallet's errors
decl_error! {
	pub enum Error for Module<T: Trait> {
		/// Value was None
		UnsupportedCurrency,

		/// Zero Balance
		ZeroBalance,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as TransactionPayment {
		/// Account currency map
		pub AccountCurrencyMap get(fn get_currency): map hasher(blake2_128_concat) T::AccountId => Option<AssetId>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		#[weight = (<T as Trait>::WeightInfo::set_currency(), Pays::No)]
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

					<AccountCurrencyMap<T>>::insert(who, currency);
					Ok(())
				},
				false => Err(Error::<T>::UnsupportedCurrency.into())
			}
		}
	}
}

pub struct MultiCurrencyAdapter<C, OU>(PhantomData<(C, OU)>);

impl<T, C, OU> OnChargeTransaction<T> for MultiCurrencyAdapter<C, OU>
where
	T: Trait,
	T::TransactionByteFee: Get<<C as Currency<<T as frame_system::Trait>::AccountId>>::Balance>,
	C: Currency<<T as frame_system::Trait>::AccountId>,
	C::PositiveImbalance:
		Imbalance<<C as Currency<<T as frame_system::Trait>::AccountId>>::Balance, Opposite = C::NegativeImbalance>,
	C::NegativeImbalance:
		Imbalance<<C as Currency<<T as frame_system::Trait>::AccountId>>::Balance, Opposite = C::PositiveImbalance>,
	OU: OnUnbalanced<NegativeImbalanceOf<C, T>>,
	C::Balance: Into<Balance>,
{
	type LiquidityInfo = Option<NegativeImbalanceOf<C, T>>;
	type Balance = <C as Currency<<T as frame_system::Trait>::AccountId>>::Balance;

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

		// Let's determine currency in which user would like to pay the fee
		let fee_currency = match Module::<T>::get_currency(who) {
			Some(c) => c,
			_ => CORE_ASSET_ID,
		};
		// If not native currency, let's buy CORE asset first and then pay with that.
		if fee_currency != CORE_ASSET_ID {
			match T::AMMPool::buy(&who, CORE_ASSET_ID, fee_currency, fee.into(), 2u128 * fee.into(), false) {
				Ok(_) => {}
				Err(_) => {
					return Err(InvalidTransaction::Payment.into());
				}
			}
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
