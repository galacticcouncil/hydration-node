use super::*;
use crate::assets::{DotAssetId, XykPaymentAssetSupport};
use crate::types::ShortOraclePrice;
use frame_support::traits::Contains;
use hydradx_adapters::price::ConvertBalance;
use pallet_asset_registry::AssetType;
use polkadot_xcm::{
	v5::{
		Asset, AssetId as XcmAssetId, Fungibility, Instruction,
		Instruction::{DepositReserveAsset, InitiateReserveWithdraw, TransferReserveAsset},
		Location, Xcm,
	},
	VersionedXcm,
};
use primitives::Balance;
use sp_runtime::traits::Convert;
use sp_runtime::{ArithmeticError, DispatchResult};

pub enum OperationKind {
	Burn,
	Withdraw,
	Transfer,
}

// TODO: move to the pallet
pub struct WithdrawCircuitBreaker;
impl WithdrawCircuitBreaker {
	pub fn convert_to_hdx(asset_id: AssetId, amount: Balance) -> Option<Balance> {
		if asset_id == CORE_ASSET_ID {
			return Some(amount);
		}
		let (converted, _) = ConvertBalance::<ShortOraclePrice, XykPaymentAssetSupport, DotAssetId>::convert((
			asset_id,
			CORE_ASSET_ID,
			amount,
		))?;
		Some(converted)
	}

	pub fn should_account_operation(asset_id: AssetId, op_kind: OperationKind, maybe_dest: Option<&AccountId>) -> bool {
		let asset_details = AssetRegistry::assets(asset_id);
		let asset_type = asset_details.map(|d| d.asset_type);

		match op_kind {
			OperationKind::Burn | OperationKind::Withdraw => {
				matches!(asset_type, Some(AssetType::External) | Some(AssetType::Erc20))
			}
			OperationKind::Transfer => {
				if let Some(dest) = maybe_dest {
					pallet_circuit_breaker::Pallet::<Runtime>::egress_accounts().contains(dest)
				} else {
					false
				}
			}
		}
	}

	pub fn on_egress(asset_id: AssetId, amount: Balance) -> DispatchResult {
		let amount_ref_currency = Self::convert_to_hdx(asset_id, amount).ok_or(ArithmeticError::Overflow)?;
		pallet_circuit_breaker::Pallet::<Runtime>::note_egress(amount_ref_currency)
	}

	pub fn is_lockdown_active() -> bool {
		let now = pallet_circuit_breaker::Pallet::<Runtime>::timestamp_now();
		pallet_circuit_breaker::Pallet::<Runtime>::is_lockdown_at(now)
	}

	/// Returns true if the XCM message is an egress message and the global lockdown is active.
	pub fn is_egress_blocked<Call>(message: &VersionedXcm<Call>) -> bool {
		if let Ok(xcm) = Xcm::<Call>::try_from(message.clone()) {
			return XcmEgressFilter::is_egress(&xcm) && Self::is_lockdown_active();
		}
		false
	}
}

impl pallet_currencies::OnWithdraw<AccountId, AssetId, Balance> for WithdrawCircuitBreaker {
	fn on_withdraw(asset_id: AssetId, _who: &AccountId, amount: Balance) -> DispatchResult {
		if Self::should_account_operation(asset_id, OperationKind::Withdraw, None) {
			Self::on_egress(asset_id, amount)?;
		}
		Ok(())
	}
}

impl orml_traits::currency::OnTransfer<AccountId, AssetId, Balance> for WithdrawCircuitBreaker {
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
