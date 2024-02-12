use frame_support::dispatch::DispatchResult;
use frame_support::traits::{Currency, ExistenceRequirement, fungible, fungibles, Get, SignedImbalance, WithdrawReasons};
use frame_support::traits::tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence};
use orml_tokens::{Config, CurrencyAdapter, NegativeImbalance, Pallet, PositiveImbalance};
use orml_traits::MultiCurrency;
use sp_runtime::DispatchError;
use sp_runtime::traits::Convert;
use hydradx_traits::FeePaymentCurrency;
use primitives::{AssetId, Balance};

pub struct FeeAssetCurrencyAdapter<T, AccountId,Inspector, GetCurrencyId, EC, C>(sp_std::marker::PhantomData<(T, AccountId, Inspector, GetCurrencyId, EC, C)>);


impl<T, AccountId, Inspector, GetCurrencyId, EC, C> fungible::Inspect<AccountId> for FeeAssetCurrencyAdapter<T, AccountId, Inspector, GetCurrencyId, EC, C>
    where
        Inspector: fungibles::Inspect<AccountId, AssetId = AssetId, Balance=  Balance>,
        GetCurrencyId: FeePaymentCurrency<AccountId, AssetId = AssetId>,
        EC: Get<AssetId>,
        C: Convert<(AssetId, Balance),Balance>,
{
    type Balance = Balance;

    fn total_issuance() -> Self::Balance {
        Inspector::total_issuance(EC::get())
    }
    fn minimum_balance() -> Self::Balance {
        Inspector::minimum_balance(EC::get())
    }
    fn balance(who: &AccountId) -> Self::Balance {
        let currency_id = GetCurrencyId::get(who).unwrap_or(EC::get());
        Inspector::balance(currency_id, who)
    }
    fn total_balance(who: &AccountId) -> Self::Balance {
        let currency_id = GetCurrencyId::get(who).unwrap_or(EC::get());
        Inspector::total_balance(currency_id, who)
    }
    fn reducible_balance(who: &AccountId, preservation: Preservation, fortitude: Fortitude) -> Self::Balance {
        let currency_id = GetCurrencyId::get(who).unwrap_or(EC::get());
        let currency_balance = Inspector::reducible_balance(currency_id, who, preservation, fortitude);
        let converted = C::convert((currency_id, currency_balance));
        converted
    }
    fn can_deposit(who: &AccountId, amount: Self::Balance, provenance: Provenance) -> DepositConsequence {
        let currency_id = GetCurrencyId::get(who).unwrap_or(EC::get());
        Inspector::can_deposit(currency_id, who, amount, provenance)
    }
    fn can_withdraw(who: &AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
        let currency_id = GetCurrencyId::get(who).unwrap_or(EC::get());
        Inspector::can_withdraw(currency_id, who, amount)
    }
}

impl<T, AccountId, Inspector, GetCurrencyId, EC, C> frame_support::traits::tokens::currency::Currency<AccountId> for FeeAssetCurrencyAdapter<T, AccountId,Inspector, GetCurrencyId, EC, C>
    where
        T: Currency<AccountId, Balance = Balance>,
{
    type Balance = T::Balance;
    type PositiveImbalance = T::PositiveImbalance;
    type NegativeImbalance = T::NegativeImbalance;

    fn total_balance(who: &AccountId) -> Self::Balance {
        T::total_balance(who)
    }

    fn can_slash(who: &AccountId, value: Self::Balance) -> bool {
        T::can_slash(who, value)
    }

    fn total_issuance() -> Self::Balance {
        T::total_issuance()
    }

    fn minimum_balance() -> Self::Balance {
        T::minimum_balance()
    }

    fn burn(mut amount: Self::Balance) -> Self::PositiveImbalance{
        T::burn(amount)
    }

    fn issue(mut amount: Self::Balance) -> Self::NegativeImbalance {
        T::issue(amount)
    }

    fn free_balance(who: &AccountId) -> Self::Balance {
        T::free_balance(who)
    }

    fn ensure_can_withdraw(
        who: &AccountId,
        amount: Self::Balance,
        _reasons: WithdrawReasons,
        _new_balance: Self::Balance,
    ) -> DispatchResult {
        T::ensure_can_withdraw(who, amount, _reasons, _new_balance)
    }

    fn transfer(
        source: &AccountId,
        dest: &AccountId,
        value: Self::Balance,
        _existence_requirement: ExistenceRequirement,
    ) -> DispatchResult {
        T::transfer(source, dest, value, _existence_requirement)
    }

    fn slash(who: &AccountId, value: Self::Balance) -> (Self::NegativeImbalance, Self::Balance) {
        T::slash(who, value)
    }

    fn deposit_into_existing(
        who: &AccountId,
        value: Self::Balance,
    ) -> sp_std::result::Result<Self::PositiveImbalance, DispatchError> {
        T::deposit_into_existing(who, value)
    }

    /// Deposit some `value` into the free balance of `who`, possibly creating a
    /// new account.
    fn deposit_creating(who: &AccountId, value: Self::Balance) -> Self::PositiveImbalance {
        T::deposit_creating(who, value)
    }

    fn withdraw(
        who: &AccountId,
        value: Self::Balance,
        _reasons: WithdrawReasons,
        liveness: ExistenceRequirement,
    ) -> sp_std::result::Result<Self::NegativeImbalance, DispatchError> {
        T::withdraw(who, value, _reasons, liveness)
    }

    fn make_free_balance_be(
        who: &AccountId,
        value: Self::Balance,
    ) -> SignedImbalance<T::Balance, Self::PositiveImbalance> {
        T::make_free_balance_be(who, value)
    }
}