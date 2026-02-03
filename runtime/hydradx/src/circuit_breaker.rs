use super::*;
use crate::assets::XykPaymentAssetSupport;
use crate::types::ShortOraclePrice;
use hydradx_adapters::price::ConvertBalance;
use hydradx_traits::circuit_breaker::WithdrawFuseControl;
use pallet_asset_registry::AssetType;
use polkadot_xcm::v5::{
	Instruction::{DepositReserveAsset, InitiateReserveWithdraw, TransferReserveAsset},
	Xcm,
};
use primitives::Balance;
use sp_runtime::traits::Convert;
use sp_runtime::DispatchResult;
use sp_std::marker::PhantomData;

pub enum OperationKind {
	Burn,
	Withdraw,
	Transfer,
}

pub struct WithdrawCircuitBreaker<ReferenceCurrencyId>(PhantomData<ReferenceCurrencyId>);
impl<ReferenceCurrencyId> WithdrawCircuitBreaker<ReferenceCurrencyId>
where
	ReferenceCurrencyId: Get<AssetId>,
{
	fn convert_to_hdx(asset_id: AssetId, amount: Balance) -> Option<Balance> {
		if asset_id == ReferenceCurrencyId::get() {
			return Some(amount);
		}
		let (converted, _) = ConvertBalance::<ShortOraclePrice, XykPaymentAssetSupport, ReferenceCurrencyId>::convert(
			(asset_id, CORE_ASSET_ID, amount),
		)?;
		Some(converted)
	}

	pub fn should_account_operation(asset_id: AssetId, op_kind: OperationKind, maybe_dest: Option<&AccountId>) -> bool {
		if CircuitBreaker::ignore_withdraw_fuse() {
			return false;
		}

		let asset_details = AssetRegistry::assets(asset_id);
		let asset_type = asset_details.map(|d| d.asset_type);

		match op_kind {
			OperationKind::Burn | OperationKind::Withdraw if matches!(asset_type, Some(AssetType::External)) => true,
			OperationKind::Transfer => {
				if let Some(dest) = maybe_dest {
					pallet_circuit_breaker::Pallet::<Runtime>::is_account_egress(dest).is_some()
				} else {
					false
				}
			}
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

impl<ReferenceCurrencyId> pallet_currencies::OnWithdraw<AccountId, AssetId, Balance>
	for WithdrawCircuitBreaker<ReferenceCurrencyId>
where
	ReferenceCurrencyId: Get<AssetId>,
{
	fn on_withdraw(asset_id: AssetId, _who: &AccountId, amount: Balance) -> DispatchResult {
		if Self::should_account_operation(asset_id, OperationKind::Withdraw, None) {
			Self::on_egress(asset_id, amount)?;
		}
		Ok(())
	}
}

impl<ReferenceCurrencyId> orml_traits::currency::OnTransfer<AccountId, AssetId, Balance>
	for WithdrawCircuitBreaker<ReferenceCurrencyId>
where
	ReferenceCurrencyId: Get<AssetId>,
{
	fn on_transfer(asset_id: AssetId, _from: &AccountId, to: &AccountId, amount: Balance) -> DispatchResult {
		if Self::should_account_operation(asset_id, OperationKind::Transfer, Some(to)) {
			Self::on_egress(asset_id, amount)?;
		}
		Ok(())
	}
}

pub struct XcmEgressFilter;
impl XcmEgressFilter {
	pub fn is_egress<Call>(message: &Xcm<Call>) -> bool {
		message.0.iter().any(|inst| {
			matches!(
				inst,
				DepositReserveAsset { .. } | InitiateReserveWithdraw { .. } | TransferReserveAsset { .. }
			)
		})
	}
}

pub struct IgnoreWithdrawFuse<T>(PhantomData<T>);
impl<T: pallet_circuit_breaker::Config> WithdrawFuseControl for IgnoreWithdrawFuse<T> {
	fn set_withdraw_fuse_active(value: bool) {
		pallet_circuit_breaker::IgnoreWithdrawFuse::<T>::set(!value);
	}
}
