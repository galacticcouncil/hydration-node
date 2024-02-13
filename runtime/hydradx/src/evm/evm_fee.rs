use crate::evm::{WethAssetId, WethCurrency};
use crate::TreasuryAccount;
use frame_support::traits::tokens::{Fortitude, Precision};
use frame_support::traits::{Currency, ExistenceRequirement, Get, WithdrawReasons};
use hydradx_traits::FeePaymentCurrency;
use pallet_evm::{AddressMapping, Error};
use pallet_transaction_multi_payment::{DepositAll, DepositFee};
use primitives::{AccountId, AssetId, Balance};
use sp_runtime::traits::Convert;
use sp_std::marker::PhantomData;
use {
	frame_support::traits::{Currency as PalletCurrency, Imbalance, OnUnbalanced},
	pallet_evm::{EVMCurrencyAdapter, OnChargeEVMTransaction},
	sp_core::{H160, U256},
	sp_runtime::traits::UniqueSaturatedInto,
};

type CurrencyAccountId<T> = <T as frame_system::Config>::AccountId;
type BalanceFor<T> = <<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::Balance;
type PositiveImbalanceFor<T> =
	<<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::PositiveImbalance;
type NegativeImbalanceFor<T> =
	<<T as pallet_evm::Config>::Currency as PalletCurrency<CurrencyAccountId<T>>>::NegativeImbalance;

/// Implements the transaction payment for EVM transactions.
pub struct TransferEvmFees<OU, AC, EC, C, MC>(PhantomData<(OU, AC, EC, C, MC)>);

impl<T, OU, AC, EC, C, MC> OnChargeEVMTransaction<T> for TransferEvmFees<OU, AC, EC, C, MC>
where
	T: pallet_evm::Config,
	PositiveImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = NegativeImbalanceFor<T>>,
	NegativeImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = PositiveImbalanceFor<T>>,
	OU: OnUnbalanced<NegativeImbalanceFor<T>>,
	U256: UniqueSaturatedInto<BalanceFor<T>>,
	AC: FeePaymentCurrency<T::AccountId, AssetId = AssetId>,
	EC: Get<AssetId>,
	C: Convert<(AssetId, Balance), Balance>,
	BalanceFor<T>: From<Balance>,
	U256: UniqueSaturatedInto<Balance>,
	MC: frame_support::traits::tokens::fungibles::Mutate<T::AccountId, AssetId = AssetId, Balance = Balance>,
{
	type LiquidityInfo = Option<NegativeImbalanceFor<T>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		if fee.is_zero() {
			return Ok(None);
		}
		let account_id = T::AddressMapping::into_account_id(*who);

		let fee_currency = AC::get(&account_id).unwrap_or(EC::get());

		let converted = C::convert((fee_currency, fee.unique_saturated_into()));

		let burned = MC::burn_from(
			fee_currency,
			&account_id,
			converted,
			Precision::Exact,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::BalanceLow)?;

		Ok(None)
		/*
		let imbalance = C::withdraw(
			&account_id,
			fee.unique_saturated_into(),
			WithdrawReasons::FEE,
			ExistenceRequirement::AllowDeath,
		)
			.map_err(|_| Error::<T>::BalanceLow)?;
		Ok(Some(imbalance))

		 */
		//EVMCurrencyAdapter::<<T as pallet_evm::Config>::Currency, ()>::withdraw_fee(who, U256::from(converted))
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

type NegativeImbalance = <WethCurrency as PalletCurrency<AccountId>>::NegativeImbalance;

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	// this is called for substrate-based transactions
	fn on_unbalanceds<B>(_: impl Iterator<Item = NegativeImbalance>) {}

	// this is called from pallet_evm for Ethereum-based transactions
	// (technically, it calls on_unbalanced, which calls this when non-zero)
	fn on_nonzero_unbalanced(amount: NegativeImbalance) {
		let _ = DepositAll::<crate::Runtime>::deposit_fee(&TreasuryAccount::get(), WethAssetId::get(), amount.peek());
	}
}

pub struct ToWethConversion;

impl Convert<(AssetId, Balance), Balance> for ToWethConversion {
	fn convert((asset_id, balance): (AssetId, Balance)) -> Balance {
		//TODO: convert using oracle
		balance
	}
}

pub struct FromWethConversion;

impl Convert<(AssetId, Balance), Balance> for FromWethConversion {
	fn convert((asset_id, balance): (AssetId, Balance)) -> Balance {
		//TODO: convert using oracle
		balance
	}
}
