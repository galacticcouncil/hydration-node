// GIGAHDX runtime configuration — adapters and Config impls for
// pallet-gigahdx and pallet-gigahdx-voting.

use super::*;
use crate::evm::aave_trade_executor::Aave;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::evm::Erc20Currency;
use frame_support::{parameter_types, PalletId};
use hydradx_traits::evm::{CallContext, Erc20Mapping, InspectEvmAccounts, ERC20};
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_liquidation::GigaHdxPoolContract;
use primitives::constants::time::DAYS;
use sp_runtime::{DispatchError, Permill};

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::{price::OraclePriceProviderUsingRoute, OraclePriceProvider};

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

parameter_types! {
	pub const StHdxAssetId: AssetId = 670;
	pub const GigaHdxAssetIdConst: AssetId = 67;
	pub const GigaHdxPalletId: PalletId = PalletId(*b"gigahdx!");
	pub const GigaRewardPotId: PalletId = PalletId(*b"gigarwd!");
	pub const GigaHdxCooldownPeriod: BlockNumber = 222 * DAYS;
	pub const GigaHdxMinStake: Balance = 10 * primitives::constants::currency::UNITS;
	pub const GigaHdxMinUnstake: Balance = 1 * primitives::constants::currency::UNITS;
	pub const GigaHdxMaxUnstakePositions: u32 = 10;
	pub const GigaHdxMaxVotes: u32 = 25;
}

// ---------------------------------------------------------------------------
// AaveMoneyMarket — supply/withdraw stHDX via real AAVE Pool contract
// ---------------------------------------------------------------------------

pub struct AaveMoneyMarket;

impl hydradx_traits::gigahdx::MoneyMarketOperations<AccountId, AssetId, Balance> for AaveMoneyMarket {
	fn supply(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let _ = pallet_evm_accounts::Pallet::<Runtime>::bind_evm_address(RuntimeOrigin::signed(who.clone()));

		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		let pool = GigaHdxPoolContract::<Runtime>::get();

		let ctx = CallContext::new_call(asset_evm, who_evm);
		Erc20Currency::<Runtime>::approve(ctx, pool, amount)?;

		Aave::do_supply_on_behalf_of(pool, who, who, asset_evm, amount)?;

		Ok(amount)
	}

	fn withdraw(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let _ = pallet_evm_accounts::Pallet::<Runtime>::bind_evm_address(RuntimeOrigin::signed(who.clone()));

		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let pool = GigaHdxPoolContract::<Runtime>::get();

		Aave::do_withdraw(pool, who, asset_evm, amount)?;

		Ok(amount)
	}

	fn balance_of(who: &AccountId) -> Balance {
		// Query the GIGAHDX aToken balance via its ERC20 contract address.
		// GigaHdxAssetIdConst (67) is registered in the asset registry with the aToken contract address.
		let gigahdx_contract = HydraErc20Mapping::asset_address(GigaHdxAssetIdConst::get());
		Erc20Currency::<Runtime>::free_balance(gigahdx_contract, who)
	}
}

// ---------------------------------------------------------------------------
// RuntimeReferendumInfo — GetReferendumOutcome + GetTrackId
// ---------------------------------------------------------------------------

pub struct RuntimeReferendumInfo;

impl hydradx_traits::gigahdx::GetReferendumOutcome<u32> for RuntimeReferendumInfo {
	fn is_referendum_finished(index: u32) -> bool {
		use frame_support::traits::{PollStatus, Polling};

		let r = <Referenda as Polling<pallet_conviction_voting::TallyOf<Runtime>>>::try_access_poll::<bool>(
			index,
			|status| {
				let finished = match status {
					PollStatus::Completed(_, _) => true,
					PollStatus::Ongoing(_, _) => false,
					PollStatus::None => false,
				};
				Ok(finished)
			},
		);
		r.unwrap_or(false)
	}

	fn referendum_outcome(index: u32) -> hydradx_traits::gigahdx::ReferendumOutcome {
		use hydradx_traits::gigahdx::ReferendumOutcome;
		use pallet_referenda::ReferendumInfo;

		let Some(info) = pallet_referenda::ReferendumInfoFor::<Runtime>::get(index) else {
			return ReferendumOutcome::Cancelled;
		};

		match info {
			ReferendumInfo::Ongoing(..) => ReferendumOutcome::Ongoing,
			ReferendumInfo::Approved(..) => ReferendumOutcome::Approved,
			ReferendumInfo::Rejected(..) => ReferendumOutcome::Rejected,
			ReferendumInfo::Cancelled(..) | ReferendumInfo::TimedOut(..) | ReferendumInfo::Killed(..) => {
				ReferendumOutcome::Cancelled
			}
		}
	}
}

impl hydradx_traits::gigahdx::GetTrackId<u32> for RuntimeReferendumInfo {
	type TrackId = u16;

	fn track_id(index: u32) -> Option<u16> {
		use pallet_referenda::ReferendumInfo;

		let info = pallet_referenda::ReferendumInfoFor::<Runtime>::get(index)?;

		match info {
			ReferendumInfo::Ongoing(status) => Some(status.track),
			// Completed referenda don't store the track — use our cache.
			_ => pallet_gigahdx_voting::ReferendumTracks::<Runtime>::get(index),
		}
	}
}

// ---------------------------------------------------------------------------
// RuntimeForceRemoveVote
// ---------------------------------------------------------------------------

pub struct RuntimeForceRemoveVote;

impl hydradx_traits::gigahdx::ForceRemoveVote<AccountId> for RuntimeForceRemoveVote {
	fn remove_vote(who: &AccountId, class: Option<u16>, index: u32) -> frame_support::dispatch::DispatchResult {
		pallet_conviction_voting::Pallet::<Runtime>::remove_vote(RuntimeOrigin::signed(who.clone()), class, index)
	}
}

// ---------------------------------------------------------------------------
// RuntimeTrackRewards — per-track reward percentage
// ---------------------------------------------------------------------------

pub struct RuntimeTrackRewards;

impl hydradx_traits::gigahdx::TrackRewardConfig for RuntimeTrackRewards {
	fn reward_percentage(track_id: u16) -> Permill {
		match track_id {
			0 => Permill::from_percent(10), // root
			1 => Permill::from_percent(8),  // whitelisted_caller
			5 => Permill::from_percent(5),  // treasurer
			_ => Permill::from_percent(3),  // default
		}
	}
}

// ---------------------------------------------------------------------------
// pallet-gigahdx Config
// ---------------------------------------------------------------------------

impl pallet_gigahdx::Config for Runtime {
	type Currency = FungibleCurrencies<Runtime>;
	type LockableCurrency = Currencies;
	type MoneyMarket = AaveMoneyMarket;
	type Hooks = GigaHdxVoting;
	type PalletId = GigaHdxPalletId;
	type HdxAssetId = NativeAssetId;
	type StHdxAssetId = StHdxAssetId;
	type GigaHdxAssetId = GigaHdxAssetIdConst;
	type CooldownPeriod = GigaHdxCooldownPeriod;
	type MinStake = GigaHdxMinStake;
	type MinUnstake = GigaHdxMinUnstake;
	type MaxUnstakePositions = GigaHdxMaxUnstakePositions;
	type WeightInfo = ();
}

// ---------------------------------------------------------------------------
// pallet-gigahdx-voting Config
// ---------------------------------------------------------------------------

impl pallet_gigahdx_voting::Config for Runtime {
	type NativeCurrency = Balances;
	type Referenda = RuntimeReferendumInfo;
	type TrackRewards = RuntimeTrackRewards;
	type ForceRemoveVote = RuntimeForceRemoveVote;
	type GigaRewardPotId = GigaRewardPotId;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MaxVotes = GigaHdxMaxVotes;
	type VotingWeightInfo = ();
}

// ---------------------------------------------------------------------------
// pallet-fee-processor — FeeReceiver impls and Config
// ---------------------------------------------------------------------------

parameter_types! {
	pub const FeeProcessorPalletId: PalletId = PalletId(*b"feeproc/");
	pub const MaxFeeConversionsPerBlock: u32 = 5;
}

/// GigaHDX fee receiver — deposits HDX to gigapot, increasing exchange rate for all holders.
pub struct GigaHdxFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for GigaHdxFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_gigahdx::Pallet::<Runtime>::gigapot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(60)
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// GigaHDX reward fee receiver — deposits HDX to the GigaReward pot for governance voting rewards.
pub struct GigaHdxRewardFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for GigaHdxRewardFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_gigahdx_voting::Pallet::<Runtime>::giga_reward_pot_account()
	}

	fn percentage() -> Permill {
		Permill::from_percent(20)
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// Staking fee receiver for non-HDX path — 10% of converted HDX.
pub struct StakingFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for StakingFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_staking::Pallet::<Runtime>::pot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(10)
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// GigaHDX fee receiver for HDX path — 70% (no referrals, so gets extra 10%).
pub struct HdxGigaHdxFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for HdxGigaHdxFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_gigahdx::Pallet::<Runtime>::gigapot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(70)
	}

	fn on_pre_fee_deposit(_trader: AccountId, _amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}

	fn on_fee_received(_amount: Balance) -> Result<(), Self::Error> {
		Ok(())
	}
}

/// Staking fee receiver for HDX path — 10% of HDX trade fees.
pub struct HdxStakingFeeReceiver;

impl hydradx_traits::gigahdx::FeeReceiver<AccountId, Balance> for HdxStakingFeeReceiver {
	type Error = sp_runtime::DispatchError;

	fn destination() -> AccountId {
		pallet_staking::Pallet::<Runtime>::pot_account_id()
	}

	fn percentage() -> Permill {
		Permill::from_percent(10)
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
		Permill::from_percent(10)
	}

	fn on_pre_fee_deposit(trader: AccountId, amount: Balance) -> Result<(), Self::Error> {
		pallet_referrals::Pallet::<Runtime>::on_fee_received(trader, amount)
	}

	fn on_fee_received(amount: Balance) -> Result<(), Self::Error> {
		pallet_referrals::Pallet::<Runtime>::on_hdx_deposited(amount)
	}
}

impl pallet_fee_processor::Config for Runtime {
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
	type MaxConversionsPerBlock = MaxFeeConversionsPerBlock;
	type FeeReceivers = (
		GigaHdxFeeReceiver,
		GigaHdxRewardFeeReceiver,
		StakingFeeReceiver,
		ReferralsFeeReceiver,
	);
	type HdxFeeReceivers = (HdxGigaHdxFeeReceiver, GigaHdxRewardFeeReceiver, HdxStakingFeeReceiver);
	type WeightInfo = ();
}
