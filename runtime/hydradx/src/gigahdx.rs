// GIGAHDX runtime configuration — fee-processor Config impl.

use super::*;
use frame_support::{parameter_types, PalletId};
use pallet_currencies::fungibles::FungibleCurrencies;
use sp_runtime::Permill;

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::{price::OraclePriceProviderUsingRoute, OraclePriceProvider};

// ---------------------------------------------------------------------------
// pallet-fee-processor — FeeReceiver impls and Config
// ---------------------------------------------------------------------------

parameter_types! {
	pub const FeeProcessorPalletId: PalletId = PalletId(*b"feeproc/");
	pub const MinFeeConversionAmount: Balance = 1_000_000_000_000; // 1 HDX equivalent
	pub const MaxFeeConversionsPerBlock: u32 = 5;
}

/// Staking fee receiver — accumulates HDX in staking pot, no callback needed.
pub struct StakingFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for StakingFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_staking::Pallet::<Runtime>::pot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(50)
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// Referrals fee receiver — needs trader context for share calculation.
pub struct ReferralsFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for ReferralsFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_referrals::Pallet::<Runtime>::pot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(30)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		pallet_referrals::Pallet::<Runtime>::on_fee_received(trader, amount)
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

impl pallet_fee_processor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = FungibleCurrencies<Runtime>;
	type Convert = ConvertViaOmnipool<Omnipool>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type PriceProvider =
		OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNA>, ReferralsOraclePeriod>;
	#[cfg(feature = "runtime-benchmarks")]
	type PriceProvider = ReferralsDummyPriceProvider;
	type PalletId = FeeProcessorPalletId;
	type HdxAssetId = NativeAssetId;
	type LrnaAssetId = LRNA;
	type MinConversionAmount = MinFeeConversionAmount;
	type MaxConversionsPerBlock = MaxFeeConversionsPerBlock;
	type FeeReceivers = (StakingFeeReceiver, ReferralsFeeReceiver);
	type HdxFeeReceivers = (StakingFeeReceiver, ReferralsFeeReceiver);
	type WeightInfo = ();
}
