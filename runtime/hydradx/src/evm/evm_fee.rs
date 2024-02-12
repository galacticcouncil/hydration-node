use crate::evm::{WethAssetId, WethCurrency};
use crate::TreasuryAccount;
use frame_support::traits::Get;
use pallet_transaction_multi_payment::{DepositAll, DepositFee};
use primitives::AccountId;
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
pub struct TransferEvmFees<OU>(PhantomData<OU>);

impl<T, OU> OnChargeEVMTransaction<T> for TransferEvmFees<OU>
where
	T: pallet_evm::Config,
	PositiveImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = NegativeImbalanceFor<T>>,
	NegativeImbalanceFor<T>: Imbalance<BalanceFor<T>, Opposite = PositiveImbalanceFor<T>>,
	OU: OnUnbalanced<NegativeImbalanceFor<T>>,
	U256: UniqueSaturatedInto<BalanceFor<T>>,
{
	type LiquidityInfo = Option<NegativeImbalanceFor<T>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		EVMCurrencyAdapter::<<T as pallet_evm::Config>::Currency, ()>::withdraw_fee(who, fee)
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
