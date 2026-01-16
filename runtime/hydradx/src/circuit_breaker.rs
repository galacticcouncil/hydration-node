use super::*;
use crate::assets::{DotAssetId, XykPaymentAssetSupport};
use crate::types::ShortOraclePrice;
use frame_support::traits::Contains;
use hydradx_adapters::price::ConvertBalance;
use polkadot_xcm::{
	v4::{
		Asset, AssetId as XcmAssetId, Fungibility, Instruction, Location, Xcm,
		Instruction::{DepositReserveAsset, InitiateReserveWithdraw, TransferReserveAsset},
	},
	VersionedXcm,
};
use primitives::Balance;
use sp_runtime::traits::Convert;
use sp_runtime::{ArithmeticError, DispatchResult};

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

	pub fn try_egress(asset_id: AssetId, amount: Balance) -> DispatchResult {
		let amount_ref_currency = Self::convert_to_hdx(asset_id, amount).ok_or(ArithmeticError::Overflow)?;
		pallet_circuit_breaker::Pallet::<Runtime>::try_note_egress(amount_ref_currency)
	}

	pub fn on_egress(asset_id: AssetId, amount: Balance) -> DispatchResult {
		let amount_ref_currency = Self::convert_to_hdx(asset_id, amount).ok_or(ArithmeticError::Overflow)?;
		pallet_circuit_breaker::Pallet::<Runtime>::note_egress(amount_ref_currency)
	}

	pub fn handle_xcm_assets(assets: &Vec<Asset>) -> DispatchResult {
		// TODO: handle considering AssetType
		for asset in assets {
			if let Asset {
				id: XcmAssetId(_location),
				fun: Fungibility::Fungible(amount),
			} = asset
			{
				if let Some(asset_id) = CurrencyIdConvert::convert(asset.clone()) {
					Self::on_egress(asset_id, *amount)?;
				}
			}
		}
		Ok(())
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

pub struct XcmLockdownFilter;
impl Contains<(Location, Xcm<RuntimeCall>)> for XcmLockdownFilter {
	fn contains(t: &(Location, Xcm<RuntimeCall>)) -> bool {
		let (_, message) = t;
		if XcmEgressFilter::is_egress(message) && WithdrawCircuitBreaker::is_lockdown_active() {
			return false;
		}
		true
	}
}

pub struct CircuitBreakerReserveTransferFilter<T>(sp_std::marker::PhantomData<T>);
impl<T: Contains<(Location, Vec<Asset>)>> Contains<(Location, Vec<Asset>)> for CircuitBreakerReserveTransferFilter<T> {
	fn contains(assets: &(Location, Vec<Asset>)) -> bool {
		if WithdrawCircuitBreaker::handle_xcm_assets(&assets.1).is_err() {
			return false;
		}
		T::contains(assets)
	}
}
