use frame_support::{
	sp_runtime::DispatchResult,
	traits::{
		fungible, fungibles,
		tokens::{DepositConsequence, WithdrawConsequence},
		Get,
	},
};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::DispatchError;

pub struct CurrenciesAdapter<T>(sp_std::marker::PhantomData<T>);

type BalanceOf<T> =
	<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<
	<T as frame_system::Config>::AccountId,
>>::CurrencyId;
type AmountOf<T> = <<T as pallet_currencies::Config>::MultiCurrency as MultiCurrencyExtended<
	<T as frame_system::Config>::AccountId,
>>::Amount;

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config> fungibles::Inspect<T::AccountId> for CurrenciesAdapter<T>
where
    CurrencyIdOf<T>: Into<<T as orml_tokens::Config>::CurrencyId>,

    BalanceOf<T>: From<<T as pallet_balances::Config>::Balance>,
    BalanceOf<T>: Into<<T as pallet_balances::Config>::Balance>,

    BalanceOf<T>: From<<T as orml_tokens::Config>::Balance>,
    BalanceOf<T>: Into<<T as orml_tokens::Config>::Balance>,

    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as pallet_balances::Config>::Balance>>,
    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as orml_tokens::Config>::Balance>>,
{
    type AssetId = CurrencyIdOf<T>;
    type Balance = BalanceOf<T>;

    fn total_issuance(asset: Self::AssetId) -> Self::Balance {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::total_issuance().into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::total_issuance(asset.into()).into()
        }
    }

    fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::minimum_balance().into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::minimum_balance(asset.into()).into()
        }
    }

    fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::balance(who).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::balance(asset.into(), who).into()
        }
    }

    fn reducible_balance(asset: Self::AssetId, who: &T::AccountId, keep_alive: bool) -> Self::Balance {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::reducible_balance(who, keep_alive).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::reducible_balance(asset.into(), who, keep_alive).into()
        }
    }

    fn can_deposit(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance, mint: bool) -> DepositConsequence {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::can_deposit(who, amount.into(), mint)
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::can_deposit(asset.into(), who, amount.into(), mint)
        }
    }

    fn can_withdraw(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> WithdrawConsequence<Self::Balance> {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Inspect<T::AccountId>>::can_withdraw(who, amount.into()).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Inspect<T::AccountId>>::can_withdraw(asset.into(), who, amount.into()).into()
        }
    }
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config> fungibles::Mutate<T::AccountId> for CurrenciesAdapter<T>
where
    CurrencyIdOf<T>: Into<<T as orml_tokens::Config>::CurrencyId>,

    BalanceOf<T>: From<<T as pallet_balances::Config>::Balance>,
    BalanceOf<T>: Into<<T as pallet_balances::Config>::Balance>,

    BalanceOf<T>: From<<T as orml_tokens::Config>::Balance>,
    BalanceOf<T>: Into<<T as orml_tokens::Config>::Balance>,

    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as pallet_balances::Config>::Balance>>,
    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as orml_tokens::Config>::Balance>>,

    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as pallet_balances::Config>::Balance, DispatchError>>,
    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as orml_tokens::Config>::Balance, DispatchError>>,
{
    fn mint_into(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::mint_into(who, amount.into())
        } else {
            <orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::mint_into(asset.into(), who, amount.into())
        }
    }

    fn burn_from(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::burn_from(who, amount.into()).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::burn_from(asset.into(), who, amount.into()).into()
        }
    }

    fn slash(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::slash(who, amount.into()).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::slash(asset.into(), who, amount.into()).into()
        }
    }

    fn teleport(asset: Self::AssetId, source: &T::AccountId, dest: &T::AccountId, amount: Self::Balance) -> Result<Self::Balance, DispatchError> {
        if asset == T::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Mutate<T::AccountId>>::teleport(source, dest, amount.into()).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Mutate<T::AccountId>>::teleport(asset.into(), source, dest, amount.into()).into()
        }
    }
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config> fungibles::Transfer<T::AccountId> for CurrenciesAdapter<T>
where
    CurrencyIdOf<T>: Into<<T as orml_tokens::Config>::CurrencyId>,

    BalanceOf<T>: From<<T as pallet_balances::Config>::Balance>,
    BalanceOf<T>: Into<<T as pallet_balances::Config>::Balance>,

    BalanceOf<T>: From<<T as orml_tokens::Config>::Balance>,
    BalanceOf<T>: Into<<T as orml_tokens::Config>::Balance>,

    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as pallet_balances::Config>::Balance>>,
    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as orml_tokens::Config>::Balance>>,

    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as pallet_balances::Config>::Balance, DispatchError>>,
    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as orml_tokens::Config>::Balance, DispatchError>>,
{
    fn transfer(asset: Self::AssetId, source: &T::AccountId, dest: &T::AccountId, amount: Self::Balance, keep_alive: bool) -> Result<Self::Balance, DispatchError> {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::Transfer<T::AccountId>>::transfer(source, dest, amount.into(), keep_alive).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::Transfer<T::AccountId>>::transfer(asset.into(), source, dest, amount.into(), keep_alive).into()
        }
    }
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config> fungibles::InspectHold<T::AccountId> for CurrenciesAdapter<T>
where
    CurrencyIdOf<T>: Into<<T as orml_tokens::Config>::CurrencyId>,

    BalanceOf<T>: From<<T as pallet_balances::Config>::Balance>,
    BalanceOf<T>: Into<<T as pallet_balances::Config>::Balance>,

    BalanceOf<T>: From<<T as orml_tokens::Config>::Balance>,
    BalanceOf<T>: Into<<T as orml_tokens::Config>::Balance>,

    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as pallet_balances::Config>::Balance>>,
    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as orml_tokens::Config>::Balance>>,
{
    fn balance_on_hold(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::InspectHold<T::AccountId>>::balance_on_hold(who).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::InspectHold<T::AccountId>>::balance_on_hold(asset.into(), who).into()
        }
    }

    fn can_hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> bool {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::InspectHold<T::AccountId>>::can_hold(who, amount.into())
        } else {
            <orml_tokens::Pallet<T> as fungibles::InspectHold<T::AccountId>>::can_hold(asset.into(), who, amount.into())
        }
    }
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config> fungibles::MutateHold<T::AccountId> for CurrenciesAdapter<T>
where
    CurrencyIdOf<T>: Into<<T as orml_tokens::Config>::CurrencyId>,

    BalanceOf<T>: From<<T as pallet_balances::Config>::Balance>,
    BalanceOf<T>: Into<<T as pallet_balances::Config>::Balance>,

    BalanceOf<T>: From<<T as orml_tokens::Config>::Balance>,
    BalanceOf<T>: Into<<T as orml_tokens::Config>::Balance>,

    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as pallet_balances::Config>::Balance>>,
    WithdrawConsequence<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance>: From<WithdrawConsequence<<T as orml_tokens::Config>::Balance>>,

    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as pallet_balances::Config>::Balance, DispatchError>>,
    Result<<<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance, DispatchError>: From<Result<<T as orml_tokens::Config>::Balance, DispatchError>>,
{
    fn hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::MutateHold<T::AccountId>>::hold(who, amount.into())
        } else {
            <orml_tokens::Pallet<T> as fungibles::MutateHold<T::AccountId>>::hold(asset.into(), who, amount.into())
        }
    }

    fn release(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance, best_effort: bool) -> Result<Self::Balance, DispatchError> {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::MutateHold<T::AccountId>>::release(who, amount.into(), best_effort).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::MutateHold<T::AccountId>>::release(asset.into(), who, amount.into(), best_effort).into()
        }
    }

    fn transfer_held(asset: Self::AssetId, source: &T::AccountId, dest: &T::AccountId, amount: Self::Balance, best_effort: bool, on_hold: bool) -> Result<Self::Balance, DispatchError> {
        if asset == <T as pallet_currencies::Config>::GetNativeCurrencyId::get() {
            <pallet_balances::Pallet<T> as fungible::MutateHold<T::AccountId>>::transfer_held(source, dest, amount.into(), best_effort, on_hold).into()
        } else {
            <orml_tokens::Pallet<T> as fungibles::MutateHold<T::AccountId>>::transfer_held(asset.into(), source, dest, amount.into(), best_effort, on_hold).into()
        }
    }
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config>
	MultiCurrency<T::AccountId> for CurrenciesAdapter<T>
{
	type CurrencyId = <<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::CurrencyId;
	type Balance = <<T as pallet_currencies::Config>::MultiCurrency as MultiCurrency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	fn minimum_balance(currency_id: Self::CurrencyId) -> Self::Balance {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::minimum_balance(currency_id)
	}

	fn total_issuance(currency_id: Self::CurrencyId) -> Self::Balance {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::total_issuance(currency_id)
	}

	fn total_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::total_balance(currency_id, who)
	}

	fn free_balance(currency_id: Self::CurrencyId, who: &T::AccountId) -> Self::Balance {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::free_balance(currency_id, who)
	}

	fn ensure_can_withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::ensure_can_withdraw(currency_id, who, amount)
	}

	fn transfer(
		currency_id: Self::CurrencyId,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::transfer(currency_id, from, to, amount)
	}

	fn deposit(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::deposit(currency_id, who, amount)
	}

	fn withdraw(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::withdraw(currency_id, who, amount)
	}

	fn can_slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> bool {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::can_slash(currency_id, who, amount)
	}

	fn slash(currency_id: Self::CurrencyId, who: &T::AccountId, amount: Self::Balance) -> Self::Balance {
		<pallet_currencies::Pallet<T> as MultiCurrency<T::AccountId>>::slash(currency_id, who, amount)
	}
}

impl<T: pallet_currencies::Config + pallet_balances::Config + orml_tokens::Config + frame_system::Config>
	MultiCurrencyExtended<T::AccountId> for CurrenciesAdapter<T>
{
	type Amount = AmountOf<T>;

	fn update_balance(currency_id: CurrencyIdOf<T>, who: &T::AccountId, by_amount: Self::Amount) -> DispatchResult {
		<pallet_currencies::Pallet<T> as MultiCurrencyExtended<T::AccountId>>::update_balance(
			currency_id,
			who,
			by_amount,
		)
	}
}
