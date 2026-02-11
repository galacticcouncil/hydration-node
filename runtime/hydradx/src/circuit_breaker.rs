use super::*;
use crate::assets::XykPaymentAssetSupport;
use crate::types::ShortOraclePrice;
use hydradx_adapters::price::ConvertBalance;
use hydradx_traits::circuit_breaker::{AssetWithdrawHandler, WithdrawFuseControl};
use pallet_asset_registry::AssetType;
use pallet_circuit_breaker::types::EgressOperationKind;
use pallet_circuit_breaker::GlobalAssetCategory;
use primitives::Balance;
use sp_runtime::traits::Convert;
use sp_runtime::DispatchResult;
use sp_std::marker::PhantomData;

pub struct WithdrawLimitHandler<RC>(PhantomData<RC>);
impl<RC: Get<AssetId>> AssetWithdrawHandler<AccountId, AssetId, Balance> for WithdrawLimitHandler<RC> {
	type OnWithdraw = OnWithdrawHook<RC>;
	type OnDeposit = ();
	type OnTransfer = OnTransferHook<RC>;
}

pub struct WithdrawCircuitBreaker<ReferenceCurrencyId>(PhantomData<ReferenceCurrencyId>);
impl<ReferenceCurrencyId: Get<AssetId>> WithdrawCircuitBreaker<ReferenceCurrencyId> {
	fn convert_to_hdx(asset_id: AssetId, amount: Balance) -> Option<Balance> {
		let ref_currency = ReferenceCurrencyId::get();
		if asset_id == ref_currency {
			return Some(amount);
		}

		let (converted, _) = ConvertBalance::<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>::convert((
			asset_id,
			ref_currency,
			amount,
		))?;
		Some(converted)
	}

	pub fn global_asset_category(asset_id: AssetId) -> Option<GlobalAssetCategory> {
		if let Some(overridden) = CircuitBreaker::global_asset_overrides(asset_id) {
			return Some(overridden);
		}

		let asset_details = AssetRegistry::assets(asset_id)?;
		match asset_details.asset_type {
			AssetType::External | AssetType::Erc20 => Some(GlobalAssetCategory::External),
			AssetType::Token | AssetType::XYK | AssetType::StableSwap | AssetType::Bond => None,
		}
	}

	pub fn should_account_withdraw_operation(
		asset_id: AssetId,
		op_kind: EgressOperationKind,
		maybe_dest: Option<&AccountId>,
	) -> bool {
		if CircuitBreaker::ignore_withdraw_limit() || Self::global_asset_category(asset_id).is_none() {
			return false;
		}

		match op_kind {
			EgressOperationKind::Withdraw => true,
			EgressOperationKind::Transfer => maybe_dest.and_then(CircuitBreaker::is_account_egress).is_some(),
		}
	}

	pub fn should_account_deposit_operation(asset_id: AssetId, maybe_source: Option<AccountId>) -> bool {
		match Self::global_asset_category(asset_id) {
			Some(GlobalAssetCategory::External) => true,
			Some(GlobalAssetCategory::Local) => maybe_source.and_then(CircuitBreaker::is_account_egress).is_some(),
			_ => false,
		}
	}

	fn on_egress(asset_id: AssetId, amount: Balance) -> DispatchResult {
		let amount_ref_currency = Self::convert_to_hdx(asset_id, amount)
			.ok_or(pallet_circuit_breaker::Error::<Runtime>::FailedToConvertAsset)?;

		pallet_circuit_breaker::Pallet::<Runtime>::note_egress(amount_ref_currency)
	}

	pub fn is_lockdown_active() -> bool {
		let now = pallet_circuit_breaker::Pallet::<Runtime>::timestamp_now();
		pallet_circuit_breaker::Pallet::<Runtime>::is_lockdown_at(now)
	}
}

pub struct OnWithdrawHook<RC>(PhantomData<RC>);
impl<RC: Get<AssetId>> orml_traits::Handler<(AssetId, Balance)> for OnWithdrawHook<RC> {
	fn handle(t: &(AssetId, Balance)) -> DispatchResult {
		let (asset_id, amount) = *t;

		if WithdrawCircuitBreaker::<RC>::should_account_withdraw_operation(
			asset_id,
			EgressOperationKind::Withdraw,
			None,
		) {
			WithdrawCircuitBreaker::<RC>::on_egress(asset_id, amount)?;
		}
		Ok(())
	}
}

pub struct OnTransferHook<RC>(PhantomData<RC>);
impl<RC: Get<AssetId>> orml_traits::currency::OnTransfer<AccountId, AssetId, Balance> for OnTransferHook<RC> {
	fn on_transfer(asset_id: AssetId, _from: &AccountId, to: &AccountId, amount: Balance) -> DispatchResult {
		if WithdrawCircuitBreaker::<RC>::should_account_withdraw_operation(
			asset_id,
			EgressOperationKind::Transfer,
			Some(to),
		) {
			WithdrawCircuitBreaker::<RC>::on_egress(asset_id, amount)?;
		}
		Ok(())
	}
}

pub struct OnDepositHook<RC>(PhantomData<RC>);
impl<RC: Get<AssetId>> orml_traits::Handler<(AssetId, Balance, Option<AccountId>)> for OnDepositHook<RC> {
	fn handle(t: &(AssetId, Balance, Option<AccountId>)) -> DispatchResult {
		let (asset_id, amount, maybe_from) = t;

		if WithdrawCircuitBreaker::<RC>::should_account_deposit_operation(*asset_id, maybe_from.clone()) {
			let amount_ref_currency = WithdrawCircuitBreaker::<RC>::convert_to_hdx(*asset_id, *amount)
				.ok_or(pallet_circuit_breaker::Error::<Runtime>::FailedToConvertAsset)?;

			CircuitBreaker::note_deposit(amount_ref_currency)
		}
		Ok(())
	}
}

pub struct IgnoreWithdrawFuse<T>(PhantomData<T>);
impl<T: pallet_circuit_breaker::Config> WithdrawFuseControl for IgnoreWithdrawFuse<T> {
	fn set_withdraw_fuse_active(value: bool) {
		pallet_circuit_breaker::IgnoreWithdrawLimit::<T>::set(!value);
	}
}
