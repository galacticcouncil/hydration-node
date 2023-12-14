use crate::module::{BalanceOf, CurrencyIdOf};
use crate::{Config, Pallet};
use frame_support::traits::tokens::{
	fungible, fungibles, DepositConsequence, Fortitude, Precision, Preservation, Provenance, WithdrawConsequence,
};
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

	fn total_balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		<Pallet<T>>::total_balance(asset, who)
	}

	fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Inspect<T::AccountId>>::balance(who).into()
		} else {
			<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::balance(asset.into(), who).into()
		}
	}

	fn reducible_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		preservation: Preservation,
		force: Fortitude,
	) -> Self::Balance {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Inspect<T::AccountId>>::reducible_balance(who, preservation, force).into()
		} else {
			<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::reducible_balance(
				asset.into(),
				who,
				preservation,
				force,
			)
			.into()
		}
	}

	fn can_deposit(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
		provenance: Provenance,
	) -> DepositConsequence {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Inspect<T::AccountId>>::can_deposit(who, amount.into(), provenance)
		} else {
			<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::can_deposit(
				asset.into(),
				who,
				amount.into(),
				provenance,
			)
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

impl<T: Config> fungibles::Unbalanced<T::AccountId> for FungibleCurrencies<T>
where
	T::MultiCurrency: fungibles::Unbalanced<T::AccountId>,
	T::NativeCurrency: fungible::Unbalanced<T::AccountId>, // + fungible::Inspect<T::AccountId>,
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
	fn handle_dust(dust: fungibles::Dust<T::AccountId, Self>) {
		let asset = dust.0;
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Unbalanced<T::AccountId>>::handle_dust(fungible::Dust(dust.1.into()))
		} else {
			<T::MultiCurrency as fungibles::Unbalanced<T::AccountId>>::handle_dust(fungibles::Dust(
				dust.0.into(),
				dust.1.into(),
			))
		}
	}

	fn write_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Result<Option<Self::Balance>, DispatchError> {
		if asset == T::GetNativeCurrencyId::get() {
			let result = <T::NativeCurrency as fungible::Unbalanced<T::AccountId>>::write_balance(who, amount.into())?;
			Ok(result.map(|balance| balance.into()))
		} else {
			let result = <T::MultiCurrency as fungibles::Unbalanced<T::AccountId>>::write_balance(
				asset.into(),
				who,
				amount.into(),
			)?;
			Ok(result.map(|balance| balance.into()))
		}
	}

	fn set_total_issuance(asset: Self::AssetId, amount: Self::Balance) {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Unbalanced<T::AccountId>>::set_total_issuance(amount.into())
		} else {
			<T::MultiCurrency as fungibles::Unbalanced<T::AccountId>>::set_total_issuance(asset.into(), amount.into())
		}
	}

	fn deactivate(asset: Self::AssetId, amount: Self::Balance) {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Unbalanced<T::AccountId>>::deactivate(amount.into())
		} else {
			<T::MultiCurrency as fungibles::Unbalanced<T::AccountId>>::deactivate(asset.into(), amount.into())
		}
	}

	fn reactivate(asset: Self::AssetId, amount: Self::Balance) {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Unbalanced<T::AccountId>>::reactivate(amount.into())
		} else {
			<T::MultiCurrency as fungibles::Unbalanced<T::AccountId>>::reactivate(asset.into(), amount.into())
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
	fn mint_into(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Mutate<T::AccountId>>::mint_into(who, amount.into()).into()
		} else {
			<T::MultiCurrency as fungibles::Mutate<T::AccountId>>::mint_into(asset.into(), who, amount.into()).into()
		}
	}

	fn burn_from(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
		precision: Precision,
		force: Fortitude,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Mutate<T::AccountId>>::burn_from(who, amount.into(), precision, force)
				.into()
		} else {
			<T::MultiCurrency as fungibles::Mutate<T::AccountId>>::burn_from(
				asset.into(),
				who,
				amount.into(),
				precision,
				force,
			)
			.into()
		}
	}

	fn transfer(
		asset: Self::AssetId,
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		preservation: Preservation,
	) -> Result<Self::Balance, DispatchError> {
		if asset == T::GetNativeCurrencyId::get() {
			<T::NativeCurrency as fungible::Mutate<T::AccountId>>::transfer(source, dest, amount.into(), preservation)
				.into()
		} else {
			<T::MultiCurrency as fungibles::Mutate<T::AccountId>>::transfer(
				asset.into(),
				source,
				dest,
				amount.into(),
				preservation,
			)
			.into()
		}
	}
}
