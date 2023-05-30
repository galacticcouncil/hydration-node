use crate::module::{BalanceOf, CurrencyIdOf};
use crate::{Config, Pallet};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::tokens::{fungible, fungibles, DepositConsequence, WithdrawConsequence};
use orml_traits::MultiCurrency;
use sp_runtime::traits::Get;
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;

pub struct FungibleCurrencies<T>(PhantomData<T>);

impl<T: Config> fungibles::Inspect<T::AccountId> for FungibleCurrencies<T>
where
    T::MultiCurrency: fungibles::Inspect<T::AccountId>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::AssetId: From<CurrencyIdOf<T>>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance>>,
    T::NativeCurrency: fungible::Inspect<T::AccountId>,
    <T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance>>,
{
    type AssetId = CurrencyIdOf<T>;
    type Balance = BalanceOf<T>;

    fn total_issuance(asset: Self::AssetId) -> Self::Balance {
        <Pallet<T>>::total_issuance(asset)
    }

    fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
        <Pallet<T>>::minimum_balance(asset)
    }

    fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Inspect<T::AccountId>>::balance(who).into()
        } else {
            <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::balance(asset.into(), who).into()
        }
    }

    fn reducible_balance(asset: Self::AssetId, who: &T::AccountId, keep_alive: bool) -> Self::Balance {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Inspect<T::AccountId>>::reducible_balance(who, keep_alive).into()
        } else {
            <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::reducible_balance(asset.into(), who, keep_alive)
                .into()
        }
    }

    fn can_deposit(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance, mint: bool) -> DepositConsequence {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Inspect<T::AccountId>>::can_deposit(who, amount.into(), mint)
        } else {
            <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::can_deposit(asset.into(), who, amount.into(), mint)
        }
    }

    fn can_withdraw(
        asset: Self::AssetId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> WithdrawConsequence<Self::Balance> {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Inspect<T::AccountId>>::can_withdraw(who, amount.into()).into()
        } else {
            <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::can_withdraw(asset.into(), who, amount.into())
                .into()
        }
    }

    fn asset_exists(asset: Self::AssetId) -> bool {
        if asset == T::GetNativeCurrencyId::get() {
            true
        } else {
            <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::asset_exists(asset.into())
        }
    }
}

impl<T: Config> fungibles::Mutate<T::AccountId> for FungibleCurrencies<T>
where
    T::MultiCurrency: fungibles::Inspect<T::AccountId> + fungibles::Mutate<T::AccountId>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::AssetId: From<CurrencyIdOf<T>>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance>>,
    T::NativeCurrency: fungible::Inspect<T::AccountId> + fungible::Mutate<T::AccountId>,
    <T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance>>,
    Result<Self::Balance, DispatchError>:
        From<Result<<T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance, DispatchError>>,
    Result<Self::Balance, DispatchError>:
        From<Result<<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance, DispatchError>>,
{
    fn mint_into(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
        <Pallet<T>>::deposit(asset, who, amount)
    }

    fn burn_from(
        asset: Self::AssetId,
        who: &T::AccountId,
        amount: Self::Balance,
    ) -> Result<Self::Balance, DispatchError> {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Mutate<T::AccountId>>::burn_from(who, amount.into()).into()
        } else {
            <T::MultiCurrency as fungibles::Mutate<T::AccountId>>::burn_from(asset.into(), who, amount.into()).into()
        }
    }
}

impl<T: Config> fungibles::Transfer<T::AccountId> for FungibleCurrencies<T>
where
    T::MultiCurrency:
        fungibles::Inspect<T::AccountId> + fungibles::Mutate<T::AccountId> + fungibles::Transfer<T::AccountId>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::AssetId: From<CurrencyIdOf<T>>,
    <T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance>>,
    T::NativeCurrency:
        fungible::Inspect<T::AccountId> + fungible::Mutate<T::AccountId> + fungible::Transfer<T::AccountId>,
    <T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance: Into<BalanceOf<T>> + From<BalanceOf<T>>,
    WithdrawConsequence<BalanceOf<T>>:
        From<WithdrawConsequence<<T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance>>,
    Result<Self::Balance, DispatchError>:
        From<Result<<T::NativeCurrency as fungible::Inspect<T::AccountId>>::Balance, DispatchError>>,
    Result<Self::Balance, DispatchError>:
        From<Result<<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::Balance, DispatchError>>,
{
    fn transfer(
        asset: Self::AssetId,
        source: &T::AccountId,
        dest: &T::AccountId,
        amount: Self::Balance,
        keep_alive: bool,
    ) -> Result<Self::Balance, DispatchError> {
        if asset == T::GetNativeCurrencyId::get() {
            <T::NativeCurrency as fungible::Transfer<T::AccountId>>::transfer(source, dest, amount.into(), keep_alive)
                .into()
        } else {
            <T::MultiCurrency as fungibles::Transfer<T::AccountId>>::transfer(
                asset.into(),
                source,
                dest,
                amount.into(),
                keep_alive,
            )
            .into()
        }
    }
}
