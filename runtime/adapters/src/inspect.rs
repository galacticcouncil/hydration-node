use frame_support::pallet_prelude::Get;
use frame_support::traits::fungible::Inspect as FungibleInspect;
use frame_support::traits::fungibles::Inspect as FungiblesInspect;
use frame_support::traits::tokens::{DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence};

/// An adapter to use inspect functionality for both native and multi currency
pub struct MultiInspectAdapter<AccountId, AssetId, Balance, NativeCurrency, MultiCurrency, GetNativeCurrencyId>(
	sp_std::marker::PhantomData<(
		AccountId,
		AssetId,
		Balance,
		NativeCurrency,
		MultiCurrency,
		GetNativeCurrencyId,
	)>,
);

impl<AccountId, AssetId, Balance, NativeCurrency, MultiCurrency, GetNativeCurrencyId> FungiblesInspect<AccountId>
	for MultiInspectAdapter<AccountId, AssetId, Balance, NativeCurrency, MultiCurrency, GetNativeCurrencyId>
where
	AssetId: frame_support::traits::tokens::AssetId,
	Balance: frame_support::traits::tokens::Balance,
	NativeCurrency: FungibleInspect<AccountId, Balance = Balance>,
	MultiCurrency: FungiblesInspect<AccountId, AssetId = AssetId, Balance = Balance>,
	GetNativeCurrencyId: Get<AssetId>,
{
	type AssetId = AssetId;
	type Balance = Balance;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::total_issuance()
		} else {
			MultiCurrency::total_issuance(asset)
		}
	}

	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::minimum_balance()
		} else {
			MultiCurrency::minimum_balance(asset)
		}
	}

	fn total_balance(asset: Self::AssetId, who: &AccountId) -> Self::Balance {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::total_balance(who)
		} else {
			MultiCurrency::total_balance(asset, who)
		}
	}

	fn balance(asset: Self::AssetId, who: &AccountId) -> Self::Balance {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::balance(who)
		} else {
			MultiCurrency::balance(asset, who)
		}
	}

	fn reducible_balance(
		asset: Self::AssetId,
		who: &AccountId,
		preservation: Preservation,
		force: Fortitude,
	) -> Self::Balance {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::reducible_balance(who, preservation, force)
		} else {
			MultiCurrency::reducible_balance(asset, who, preservation, force)
		}
	}

	fn can_deposit(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
		provenance: Provenance,
	) -> DepositConsequence {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::can_deposit(who, amount, provenance)
		} else {
			MultiCurrency::can_deposit(asset, who, amount, provenance)
		}
	}

	fn can_withdraw(
		asset: Self::AssetId,
		who: &AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		if GetNativeCurrencyId::get() == asset {
			NativeCurrency::can_withdraw(who, amount)
		} else {
			MultiCurrency::can_withdraw(asset, who, amount)
		}
	}

	fn asset_exists(asset: Self::AssetId) -> bool {
		if GetNativeCurrencyId::get() == asset {
			true
		} else {
			MultiCurrency::asset_exists(asset)
		}
	}
}
