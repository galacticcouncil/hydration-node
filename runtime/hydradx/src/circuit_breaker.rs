use super::*;
use crate::assets::XykPaymentAssetSupport;
use crate::types::ShortOraclePrice;
use hydradx_adapters::price::ConvertBalance;
use hydradx_traits::circuit_breaker::WithdrawFuseControl;
use pallet_asset_registry::AssetType;
use pallet_circuit_breaker::GlobalAssetCategory;
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

	pub fn should_account_operation(asset_id: AssetId, op_kind: OperationKind, maybe_dest: Option<&AccountId>) -> bool {
		if CircuitBreaker::ignore_withdraw_fuse() {
			return false;
		}

		let category = Self::global_asset_category(asset_id);

		match op_kind {
			OperationKind::Burn | OperationKind::Withdraw => {
				matches!(category, Some(GlobalAssetCategory::External))
			}
			OperationKind::Transfer => {
				if let Some(dest) = maybe_dest {
					category.is_some() && pallet_circuit_breaker::Pallet::<Runtime>::is_account_egress(dest).is_some()
				} else {
					false
				}
			}
		}
	}

	pub fn note_local_egress(asset_id: AssetId, amount: Balance) -> DispatchResult {
		if Self::global_asset_category(asset_id) != Some(GlobalAssetCategory::Local) {
			return Ok(());
		}

		Self::on_egress(asset_id, amount)
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

pub struct IgnoreWithdrawFuse<T>(PhantomData<T>);
impl<T: pallet_circuit_breaker::Config> WithdrawFuseControl for IgnoreWithdrawFuse<T> {
	fn set_withdraw_fuse_active(value: bool) {
		pallet_circuit_breaker::IgnoreWithdrawFuse::<T>::set(!value);
	}
}
