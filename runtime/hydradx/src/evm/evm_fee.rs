//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0
use crate::{Runtime, TreasuryAccount};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
use frame_support::traits::{Get, IsType, TryDrop};
use frame_support::weights::Weight;
use frame_system::ExtensionsWeightInfo;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::circuit_breaker::WithdrawFuseControl;
use hydradx_traits::fee::SwappablePaymentAssetTrader;
use hydradx_traits::AccountFeeCurrency;
use pallet_evm::{AddressMapping, Error};
use pallet_transaction_multi_payment::{DepositAll, DepositFee};
use primitives::{AccountId, AssetId, Balance};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::{Convert, DispatchInfoOf, TransactionExtension, ValidateResult};
use sp_runtime::transaction_validity::{TransactionSource, TransactionValidityError, ValidTransaction};
use sp_runtime::Rounding;
use sp_std::marker::PhantomData;
use {
	frame_support::traits::OnUnbalanced,
	pallet_evm::OnChargeEVMTransaction,
	sp_core::{H160, U256},
	sp_runtime::traits::UniqueSaturatedInto,
};

#[cfg(feature = "std")]
mod fee_payer_override {
	use core::cell::RefCell;
	use primitives::AccountId;

	thread_local! {
		static FEE_PAYER: RefCell<Option<AccountId>> = const { RefCell::new(None) };
	}

	pub fn set(payer: AccountId) {
		FEE_PAYER.with(|v| *v.borrow_mut() = Some(payer));
	}

	pub fn get() -> Option<AccountId> {
		FEE_PAYER.with(|v| v.borrow().clone())
	}

	pub fn clear() {
		FEE_PAYER.with(|v| *v.borrow_mut() = None);
	}
}

#[cfg(not(feature = "std"))]
mod fee_payer_override {
	use primitives::AccountId;

	static mut FEE_PAYER: Option<AccountId> = None;

	pub fn set(payer: AccountId) {
		unsafe {
			FEE_PAYER = Some(payer);
		}
	}

	pub fn get() -> Option<AccountId> {
		unsafe { FEE_PAYER.clone() }
	}

	pub fn clear() {
		unsafe {
			FEE_PAYER = None;
		}
	}
}

pub fn set_evm_fee_payer(payer: AccountId) {
	fee_payer_override::set(payer);
}

pub fn evm_fee_payer() -> Option<AccountId> {
	fee_payer_override::get()
}

pub fn clear_evm_fee_payer() {
	fee_payer_override::clear();
}

fn contains_evm_call(call: &crate::RuntimeCall) -> bool {
	match call {
		crate::RuntimeCall::EVM(pallet_evm::Call::call { .. }) => true,
		crate::RuntimeCall::Utility(pallet_utility::Call::batch { calls })
		| crate::RuntimeCall::Utility(pallet_utility::Call::batch_all { calls })
		| crate::RuntimeCall::Utility(pallet_utility::Call::force_batch { calls }) => calls.iter().any(contains_evm_call),
		crate::RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. }) => contains_evm_call(call),
		_ => false,
	}
}

fn is_proxy_wrapping_evm(call: &crate::RuntimeCall) -> bool {
	match call {
		crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy { call, .. })
		| crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced { call, .. }) => contains_evm_call(call),
		_ => false,
	}
}

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
/// Supports multi-currency fees based on what is provided by AC - account currency.
#[allow(clippy::type_complexity)]
pub struct TransferEvmFees<OU, AccountCurrency, EvmFeeAsset, C, MC, SwappablePaymentAssetSupport, DotAssetId, WF>(
	PhantomData<(
		OU,
		AccountCurrency,
		EvmFeeAsset,
		C,
		MC,
		SwappablePaymentAssetSupport,
		DotAssetId,
		WF,
	)>,
);

impl<T, OU, AccountCurrency, EvmFeeAsset, C, MC, SwappablePaymentAssetSupport, DotAssetId, WF> OnChargeEVMTransaction<T>
	for TransferEvmFees<OU, AccountCurrency, EvmFeeAsset, C, MC, SwappablePaymentAssetSupport, DotAssetId, WF>
where
	T: pallet_evm::Config,
	OU: OnUnbalanced<EvmPaymentInfo<EmaPrice>>,
	U256: UniqueSaturatedInto<Balance>,
	AccountCurrency: AccountFeeCurrency<T::AccountId, AssetId = AssetId>,
	EvmFeeAsset: Get<AssetId>,
	C: Convert<(AssetId, AssetId, Balance), Option<(Balance, EmaPrice)>>, // Conversion from default fee asset to account currency
	U256: UniqueSaturatedInto<Balance>,
	MC: frame_support::traits::tokens::fungibles::Mutate<T::AccountId, AssetId = AssetId, Balance = Balance>
		+ frame_support::traits::tokens::fungibles::Inspect<T::AccountId, AssetId = AssetId, Balance = Balance>,
	SwappablePaymentAssetSupport: SwappablePaymentAssetTrader<T::AccountId, AssetId, Balance>,
	DotAssetId: Get<AssetId>,
	T::AddressMapping: pallet_evm::AddressMapping<T::AccountId>,
	T::AccountId: IsType<AccountId>,
	WF: WithdrawFuseControl,
{
	type LiquidityInfo = Option<EvmPaymentInfo<EmaPrice>>;

	fn withdraw_fee(who: &H160, fee: U256) -> Result<Self::LiquidityInfo, pallet_evm::Error<T>> {
		if fee.is_zero() {
			return Ok(None);
		}
		let evm_account_id = T::AddressMapping::into_account_id(*who);

		pallet_evm_accounts::Pallet::<crate::Runtime>::mark_as_evm_account(&evm_account_id.clone().into());

		let fee_payer = evm_fee_payer().map(|a| a.into()).unwrap_or(evm_account_id);

		let account_fee_currency = AccountCurrency::get(&fee_payer);

		let (converted, fee_currency, price) =
			if SwappablePaymentAssetSupport::is_transaction_fee_currency(account_fee_currency) {
				let Some((converted, price)) =
					C::convert((EvmFeeAsset::get(), account_fee_currency, fee.unique_saturated_into()))
				else {
					return Err(Error::<T>::WithdrawFailed);
				};
				(converted, account_fee_currency, price)
			} else {
				let dot = DotAssetId::get();
				let Some((fee_in_dot, eth_dot_price)) =
					C::convert((EvmFeeAsset::get(), dot, fee.unique_saturated_into()))
				else {
					return Err(Error::<T>::WithdrawFailed);
				};

				let amount_in =
					SwappablePaymentAssetSupport::calculate_in_given_out(account_fee_currency, dot, fee_in_dot)
						.map_err(|_| Error::<T>::WithdrawFailed)?;
				let pool_fee = SwappablePaymentAssetSupport::calculate_fee_amount(amount_in)
					.map_err(|_| Error::<T>::WithdrawFailed)?;
				let max_limit = amount_in.saturating_add(pool_fee);

				SwappablePaymentAssetSupport::buy(
					&fee_payer,
					account_fee_currency,
					dot,
					fee_in_dot,
					max_limit,
					&fee_payer,
				)
				.map_err(|_| Error::<T>::WithdrawFailed)?;

				(fee_in_dot, dot, eth_dot_price)
			};

		if converted == 0 {
			return Err(Error::<T>::WithdrawFailed);
		}

		WF::set_withdraw_fuse_active(false);
		let burned = MC::burn_from(
			fee_currency,
			&fee_payer,
			converted,
			Preservation::Expendable,
			Precision::Exact,
			Fortitude::Polite,
		)
		.map_err(|_| Error::<T>::BalanceLow)?;
		WF::set_withdraw_fuse_active(true);

		Ok(Some(EvmPaymentInfo {
			amount: burned,
			asset_id: fee_currency,
			price,
		}))
	}

	fn can_withdraw(who: &H160, amount: U256) -> Result<(), pallet_evm::Error<T>> {
		let evm_account_id = T::AddressMapping::into_account_id(*who);
		let fee_payer = evm_fee_payer().map(|a| a.into()).unwrap_or(evm_account_id);

		let fee_currency = AccountCurrency::get(&fee_payer);
		let Some((converted, _)) = C::convert((EvmFeeAsset::get(), fee_currency, amount.unique_saturated_into()))
		else {
			return Err(Error::<T>::BalanceLow);
		};

		if converted == 0 {
			return Err(Error::<T>::BalanceLow);
		}
		MC::can_withdraw(fee_currency, &fee_payer, converted)
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
			let evm_account_id = T::AddressMapping::into_account_id(*who);
			let fee_payer = evm_fee_payer().map(|a| a.into()).unwrap_or(evm_account_id);

			WF::set_withdraw_fuse_active(false);

			let adjusted_paid = if let Some(converted_corrected_fee) = multiply_by_rational_with_rounding(
				corrected_fee.unique_saturated_into(),
				paid.price.n,
				paid.price.d,
				Rounding::Up,
			) {
				let refund_amount = paid.amount.saturating_sub(converted_corrected_fee);

				let result = MC::mint_into(paid.asset_id, &fee_payer, refund_amount);

				let refund_imbalance = if let Ok(amount) = result {
					// Ensure that we minted all amount, in case of partial refund for some reason,
					// refund the difference back to treasury
					debug_assert_eq!(amount, refund_amount);
					refund_amount.saturating_sub(amount)
				} else {
					// If error, we refund the whole amount back to treasury
					refund_amount
				};
				// figure out how much is left to mint back
				// refund_amount already minted back to account, imbalance is what is left to mint if any
				paid.amount
					.saturating_sub(refund_amount)
					.saturating_add(refund_imbalance)
			} else {
				// if conversion failed for some reason, we refund the whole amount back to treasury
				paid.amount
			};

			WF::set_withdraw_fuse_active(true);

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
	fn on_unbalanceds(amounts: impl Iterator<Item = EvmPaymentInfo<EmaPrice>>) {
		Self::on_unbalanced(amounts.fold(EvmPaymentInfo::default(), |i, x| x.merge(i)))
	}

	// this is called from pallet_evm for Ethereum-based transactions
	// (technically, it calls on_unbalanced, which calls this when non-zero)
	fn on_nonzero_unbalanced(payment_info: EvmPaymentInfo<EmaPrice>) {
		let result = DepositAll::<crate::Runtime>::deposit_fee(
			&TreasuryAccount::get(),
			payment_info.asset_id,
			payment_info.amount,
		);
		debug_assert_eq!(result, Ok(()));
	}
}

/// Picks the asset used to pay transaction fees for a given account.
///
/// Resolution order:
/// 1) If the account has an explicit fee-currency override set in
///    `pallet_transaction_multi_payment`, use it.
/// 2) Otherwise, defer to `account_currency(a)`, which returns either a
///    per-account currency (if present) or falls back by account type:
///    EVM account → `EvmAssetId`, non-EVM account → `NativeAssetId`.
pub struct FeeCurrencyOverrideOrDefault();

impl AccountFeeCurrency<AccountId> for FeeCurrencyOverrideOrDefault {
	type AssetId = AssetId;

	fn get(a: &AccountId) -> Self::AssetId {
		// Check if account has fee currency override set - used eg. by dispatch_permit
		if let Some(currency) = pallet_transaction_multi_payment::Pallet::<Runtime>::tx_fee_currency_override(a) {
			currency
		} else {
			// Otherwise, resolve via account_currency (handles per-account setting
			// 	and type-based defaults: EVM → EvmAssetId, non-EVM → NativeAssetId).
			pallet_transaction_multi_payment::Pallet::<Runtime>::account_currency(a)
		}
	}

	fn set(who: &AccountId, asset_id: Self::AssetId) -> DispatchResult {
		<pallet_transaction_multi_payment::Pallet<Runtime> as AccountFeeCurrency<AccountId>>::set(who, asset_id)
	}

	fn is_payment_currency(asset_id: Self::AssetId) -> DispatchResult {
		<pallet_transaction_multi_payment::Pallet<Runtime> as AccountFeeCurrency<AccountId>>::is_payment_currency(
			asset_id,
		)
	}
}

#[derive(Default, Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug, scale_info::TypeInfo)]
pub struct SetEvmFeePayer;

impl TransactionExtension<crate::RuntimeCall> for SetEvmFeePayer {
	const IDENTIFIER: &'static str = "SetEvmFeePayer";

	type Implicit = ();
	type Val = ();
	type Pre = Option<AccountId>;

	fn weight(&self, call: &crate::RuntimeCall) -> Weight {
		if is_proxy_wrapping_evm(call) {
			<crate::Runtime as frame_system::Config>::ExtensionsWeightInfo::check_non_zero_sender()
		} else {
			Weight::zero()
		}
	}

	fn validate(
		&self,
		origin: crate::RuntimeOrigin,
		_call: &crate::RuntimeCall,
		_info: &DispatchInfoOf<crate::RuntimeCall>,
		_len: usize,
		_implicit: Self::Implicit,
		_implication: &impl sp_runtime::traits::Implication,
		_source: TransactionSource,
	) -> ValidateResult<Self::Val, crate::RuntimeCall> {
		Ok((ValidTransaction::default(), (), origin))
	}

	fn prepare(
		self,
		_val: Self::Val,
		origin: &crate::RuntimeOrigin,
		call: &crate::RuntimeCall,
		_info: &DispatchInfoOf<crate::RuntimeCall>,
		_len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		if is_proxy_wrapping_evm(call) {
			if let Ok(signer) = frame_system::ensure_signed(origin.clone()) {
				set_evm_fee_payer(signer.clone());
				return Ok(Some(signer));
			}
		}
		Ok(None)
	}

	fn post_dispatch_details(
		pre: Self::Pre,
		_info: &DispatchInfoOf<crate::RuntimeCall>,
		_post_info: &sp_runtime::traits::PostDispatchInfoOf<crate::RuntimeCall>,
		_len: usize,
		_result: &DispatchResult,
	) -> Result<Weight, TransactionValidityError> {
		if pre.is_some() {
			clear_evm_fee_payer();
		}
		Ok(Weight::zero())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn fee_payer_thread_local_set_get_clear() {
		let alice: AccountId = [1u8; 32].into();

		assert_eq!(evm_fee_payer(), None);

		set_evm_fee_payer(alice.clone());
		assert_eq!(evm_fee_payer(), Some(alice));

		clear_evm_fee_payer();
		assert_eq!(evm_fee_payer(), None);
	}

	#[test]
	fn fee_payer_override_replaces_previous_value() {
		let alice: AccountId = [1u8; 32].into();
		let bob: AccountId = [2u8; 32].into();

		set_evm_fee_payer(alice.clone());
		assert_eq!(evm_fee_payer(), Some(alice));

		set_evm_fee_payer(bob.clone());
		assert_eq!(evm_fee_payer(), Some(bob));

		clear_evm_fee_payer();
	}

	#[test]
	fn contains_evm_call_detects_direct_evm_call() {
		let call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		assert!(contains_evm_call(&call));
	}

	#[test]
	fn contains_evm_call_detects_batched_evm_call() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let batch = crate::RuntimeCall::Utility(pallet_utility::Call::batch { calls: vec![evm_call] });
		assert!(contains_evm_call(&batch));
	}

	#[test]
	fn contains_evm_call_returns_false_for_non_evm_call() {
		let call = crate::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		assert!(!contains_evm_call(&call));
	}

	#[test]
	fn is_proxy_wrapping_evm_detects_proxy_evm() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let proxy_call = crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([0u8; 32]),
			force_proxy_type: None,
			call: Box::new(evm_call),
		});
		assert!(is_proxy_wrapping_evm(&proxy_call));
	}

	#[test]
	fn is_proxy_wrapping_evm_returns_false_for_proxy_non_evm() {
		let remark = crate::RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
		let proxy_call = crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([0u8; 32]),
			force_proxy_type: None,
			call: Box::new(remark),
		});
		assert!(!is_proxy_wrapping_evm(&proxy_call));
	}

	#[test]
	fn is_proxy_wrapping_evm_detects_proxy_batch_evm() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let batch = crate::RuntimeCall::Utility(pallet_utility::Call::batch_all { calls: vec![evm_call] });
		let proxy_call = crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([0u8; 32]),
			force_proxy_type: None,
			call: Box::new(batch),
		});
		assert!(is_proxy_wrapping_evm(&proxy_call));
	}

	#[test]
	fn is_proxy_wrapping_evm_returns_false_for_direct_evm_call() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		assert!(!is_proxy_wrapping_evm(&evm_call));
	}

	#[test]
	fn is_proxy_wrapping_evm_detects_proxy_announced() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let proxy_announced = crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy_announced {
			delegate: AccountId::from([1u8; 32]),
			real: AccountId::from([0u8; 32]),
			force_proxy_type: None,
			call: Box::new(evm_call),
		});
		assert!(is_proxy_wrapping_evm(&proxy_announced));
	}

	#[test]
	fn contains_evm_call_detects_as_derivative_wrapping() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let as_derivative = crate::RuntimeCall::Utility(pallet_utility::Call::as_derivative {
			index: 0,
			call: Box::new(evm_call),
		});
		assert!(contains_evm_call(&as_derivative));
	}

	#[test]
	fn is_proxy_wrapping_evm_detects_proxy_as_derivative_evm() {
		let evm_call = crate::RuntimeCall::EVM(pallet_evm::Call::call {
			source: H160::zero(),
			target: H160::zero(),
			input: vec![],
			value: U256::zero(),
			gas_limit: 0,
			max_fee_per_gas: U256::zero(),
			max_priority_fee_per_gas: None,
			nonce: None,
			access_list: vec![],
			authorization_list: vec![],
		});
		let as_derivative = crate::RuntimeCall::Utility(pallet_utility::Call::as_derivative {
			index: 0,
			call: Box::new(evm_call),
		});
		let proxy_call = crate::RuntimeCall::Proxy(pallet_proxy::Call::proxy {
			real: AccountId::from([0u8; 32]),
			force_proxy_type: None,
			call: Box::new(as_derivative),
		});
		assert!(is_proxy_wrapping_evm(&proxy_call));
	}
}
