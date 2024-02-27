use crate::TreasuryAccount;
use frame_support::traits::tokens::{Fortitude, Precision};
use frame_support::traits::{Get, TryDrop};
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::FeePaymentCurrency;
use pallet_evm::{AddressMapping, Error};
use pallet_transaction_multi_payment::{DepositAll, DepositFee};
use primitives::{AssetId, Balance};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::Convert;
use sp_runtime::Rounding;
use sp_std::marker::PhantomData;
use {
	frame_support::traits::OnUnbalanced,
	pallet_evm::OnChargeEVMTransaction,
	sp_core::{H160, U256},
	sp_runtime::traits::UniqueSaturatedInto,
};

#[derive(Copy, Clone, Default)]
pub struct EvmPaymentInfo<Price> {
	amount: Balance,
	asset_id: AssetId,
	price: Price,
}

impl<Price> EvmPaymentInfo<Price> {
	pub fn merge(self, other: Self) -> Self {
		EvmPaymentInfo {
			amount: self.amount.saturating_add(other.amount),
			asset_id: self.asset_id,
			price: self.price,
		}
	}
}

impl<Price> TryDrop for EvmPaymentInfo<Price> {
	fn try_drop(self) -> Result<(), Self> {
		if self.amount == 0 {
			Ok(())
		} else {
			Err(self)
		}
	}
}

/// Implements the transaction payment for EVM transactions.
pub struct TransferEvmFees<OU, AC, EC, C, MC>(PhantomData<(OU, AC, EC, C, MC)>);

impl<T, OU, AC, EC, C, MC> OnChargeEVMTransaction<T> for TransferEvmFees<OU, AC, EC, C, MC>
where
	T: pallet_evm::Config,
	OU: OnUnbalanced<EvmPaymentInfo<EmaPrice>>,
	U256: UniqueSaturatedInto<Balance>,
	AC: FeePaymentCurrency<T::AccountId, AssetId = AssetId>,
	EC: Get<AssetId>,
	C: Convert<(AssetId, AssetId, Balance), (Balance, EmaPrice)>,
	U256: UniqueSaturatedInto<Balance>,
	MC: frame_support::traits::tokens::fungibles::Mutate<T::AccountId, AssetId = AssetId, Balance = Balance>
		+ frame_support::traits::tokens::fungibles::Inspect<T::AccountId, AssetId = AssetId, Balance = Balance>,
{
	type LiquidityInfo = Option<EvmPaymentInfo<EmaPrice>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		if fee.is_zero() {
			return Ok(None);
		}
		let account_id = T::AddressMapping::into_account_id(*who);
		let fee_currency = AC::get(&account_id).unwrap_or(EC::get());
		let (converted, price) = C::convert((EC::get(), fee_currency, fee.unique_saturated_into()));

		// Ensure that converted fee is not zero
		if converted == 0 {
			return Err(Error::<T>::WithdrawFailed);
		}

		let burned = MC::burn_from(
			fee_currency,
			&account_id,
			converted,
			Precision::Exact,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::BalanceLow)?;

		Ok(Some(EvmPaymentInfo {
			amount: burned,
			asset_id: fee_currency,
			price,
		}))
	}

	fn can_withdraw(who: &H160, amount: U256) -> Result<(), pallet_evm::Error<T>> {
		let account_id = T::AddressMapping::into_account_id(*who);
		let fee_currency = AC::get(&account_id).unwrap_or(EC::get());
		let (converted, _) = C::convert((EC::get(), fee_currency, amount.unique_saturated_into()));

		// Ensure that converted amount is not zero
		if converted == 0 {
			return Err(Error::<T>::BalanceLow);
		}
		MC::can_withdraw(fee_currency, &account_id, converted)
			.into_result(false)
			.map_err(|_| Error::<T>::BalanceLow)?;
		Ok(())
	}
	fn correct_and_deposit_fee(
		who: &H160,
		corrected_fee: U256,
		_base_fee: U256,
		already_withdrawn: Self::LiquidityInfo,
	) -> Self::LiquidityInfo {
		if let Some(paid) = already_withdrawn {
			let account_id = T::AddressMapping::into_account_id(*who);

			let adjusted_paid = if let Some(converted_corrected_fee) = multiply_by_rational_with_rounding(
				corrected_fee.unique_saturated_into(),
				paid.price.n,
				paid.price.d,
				Rounding::Up,
			) {
				// Calculate how much refund we should return
				let refund_amount = paid.amount.saturating_sub(converted_corrected_fee);

				// refund to the account that paid the fees. If this fails, the
				// account might have dropped below the existential balance. In
				// that case we don't refund anything.
				let result = MC::mint_into(paid.asset_id, &account_id, refund_amount);

				let refund_imbalance = if let Ok(amount) = result {
					// just in case of partial refund
					refund_amount.saturating_sub(amount)
				} else {
					// If error, we refund the whole amount back to treasury
					refund_amount
				};
				// figure out how much is left to mint
				// refund_amount already minted back to account, imbalance what is left to mint if any
				paid.amount
					.saturating_sub(refund_amount)
					.saturating_add(refund_imbalance)
			} else {
				// if conversion failed for some reason, we refund the whole amount back to treasury
				paid.amount
			};

			// We can simply refund all the remaining amount back to treasury
			OU::on_unbalanced(EvmPaymentInfo {
				amount: adjusted_paid,
				asset_id: paid.asset_id,
				price: paid.price,
			});
			return None;
		}
		None
	}

	fn pay_priority_fee(tip: Self::LiquidityInfo) {
		if let Some(tip) = tip {
			OU::on_unbalanced(tip);
		}
	}
}

pub struct DepositEvmFeeToTreasury;
impl OnUnbalanced<EvmPaymentInfo<EmaPrice>> for DepositEvmFeeToTreasury {
	// this is called for substrate-based transactions
	fn on_unbalanceds<B>(amounts: impl Iterator<Item = EvmPaymentInfo<EmaPrice>>) {
		Self::on_unbalanced(amounts.fold(EvmPaymentInfo::default(), |i, x| x.merge(i)))
	}

	// this is called from pallet_evm for Ethereum-based transactions
	// (technically, it calls on_unbalanced, which calls this when non-zero)
	fn on_nonzero_unbalanced(payment_info: EvmPaymentInfo<EmaPrice>) {
		//TODO: perhaps better to avoid using something multi payment pallet - it is fine for now
		//consider creating general purpose adapter or something, or simple move the DepositAll to adapters - it has nothing to do with multi payment pallet
		let result = DepositAll::<crate::Runtime>::deposit_fee(
			&TreasuryAccount::get(),
			payment_info.asset_id,
			payment_info.amount,
		);
		debug_assert_eq!(result, Ok(()));
	}
}