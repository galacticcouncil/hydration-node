// SPDX-License-Identifier: Apache-2.0
//
// Runtime wiring for the gigahdx stack:
// - `AaveMoneyMarket` — `MoneyMarketOperations` adapter that bridges
//   `pallet-gigahdx` to the EVM-side AAVE V3 fork. `supply` mints aToken
//   (GIGAHDX) on behalf of the user from their stHDX; `withdraw` burns
//   aToken and returns stHDX. The pool address is read from
//   `pallet_gigahdx::GigaHdxPoolContract` (settable via `set_pool_contract`).
// - `TrackRewardConfig` / `RuntimeReferenda` — the two adapters that wire
//   `pallet-gigahdx-rewards` into the runtime (per-track reward table
//   and a `ReferendumInfoFor`-backed track lookup).

use crate::evm::aave_trade_executor::Function as AaveFunction;
use crate::evm::evm_error_decoder::EvmErrorDecoder;
use crate::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use crate::evm::precompiles::handle::EvmDataWriter;
use crate::evm::Erc20Currency;
use crate::evm::Executor;
use crate::Runtime;
use evm::ExitReason::Succeed;
use frame_support::sp_runtime::traits::Convert;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::LockIdentifier;
use frame_support::weights::Weight;
use hydradx_traits::evm::{CallContext, CallResult, Erc20Mapping, InspectEvmAccounts, ERC20, EVM};
use hydradx_traits::gigahdx::MoneyMarketOperations;
use pallet_evm::GasWeightMapping;
use pallet_gigahdx_rewards::traits::{ReferendaTrackInspect, TrackRewardTable};
use pallet_gigahdx_rewards::types::ReferendumIndex;
use pallet_referenda::ReferendumInfo;
use primitive_types::U256;
use primitives::{AccountId, AssetId, Balance, EvmAddress};
use sp_runtime::Permill;

const GAS_LIMIT: u64 = 500_000;

fn handle(result: CallResult) -> Result<(), DispatchError> {
	match &result.exit_reason {
		Succeed(_) => Ok(()),
		_ => {
			log::error!(
				target: "gigahdx::adapter",
				"AAVE EVM call failed: exit_reason={:?}, data=0x{}",
				result.exit_reason,
				hex::encode(&result.value),
			);
			Err(EvmErrorDecoder::convert(result))
		}
	}
}

pub struct AaveMoneyMarket;

impl AaveMoneyMarket {
	fn pool() -> Result<EvmAddress, DispatchError> {
		pallet_gigahdx::GigaHdxPoolContract::<Runtime>::get()
			.ok_or(DispatchError::Other("gigahdx: pool contract not set"))
	}
}

impl MoneyMarketOperations<AccountId, AssetId, Balance> for AaveMoneyMarket {
	fn supply(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		let pool = Self::pool()?;

		let approve_ctx = CallContext::new_call(asset_evm, who_evm);
		<Erc20Currency<Runtime> as ERC20>::approve(approve_ctx, pool, amount)?;

		// `Pool.supply` rounds scaled balance down, so the actual aToken
		// minted may be < `amount`. We return the balance delta so the pallet
		// preserves `Stakes.gigahdx == aToken.balanceOf`, which
		// `LockableAToken.burn`'s `freeBalance` check relies on.
		let balance_before = Self::balance_of(who);

		let supply_ctx = CallContext::new_call(pool, who_evm);
		let referral_code = 0_u16;
		let data = EvmDataWriter::new_with_selector(AaveFunction::Supply)
			.write(asset_evm)
			.write(amount)
			.write(who_evm)
			.write(referral_code)
			.build();
		handle(Executor::<Runtime>::call(supply_ctx, data, U256::zero(), GAS_LIMIT))?;

		let balance_after = Self::balance_of(who);
		Ok(balance_after.saturating_sub(balance_before))
	}

	fn withdraw(who: &AccountId, underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		let asset_evm = HydraErc20Mapping::asset_address(underlying_asset);
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		let pool = Self::pool()?;

		// Symmetric with `supply`: return actual underlying delta so callers
		// reconcile against AAVE's scaledBalance rounding.
		let balance_before = Self::balance_of(who);

		let withdraw_ctx = CallContext::new_call(pool, who_evm);
		let data = EvmDataWriter::new_with_selector(AaveFunction::Withdraw)
			.write(asset_evm)
			.write(amount)
			.write(who_evm)
			.build();
		handle(Executor::<Runtime>::call(withdraw_ctx, data, U256::zero(), GAS_LIMIT))?;

		let balance_after = Self::balance_of(who);
		Ok(balance_before.saturating_sub(balance_after))
	}

	fn balance_of(who: &AccountId) -> Balance {
		let atoken_addr = HydraErc20Mapping::asset_address(crate::assets::GigaHdxAssetIdConst::get());
		let who_evm = pallet_evm_accounts::Pallet::<Runtime>::evm_address(who);
		<Erc20Currency<Runtime> as ERC20>::balance_of(CallContext::new_view(atoken_addr), who_evm)
	}

	fn supply_weight() -> Weight {
		<Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(GAS_LIMIT, true)
	}

	fn withdraw_weight() -> Weight {
		<Runtime as pallet_evm::Config>::GasWeightMapping::gas_to_weight(GAS_LIMIT, true)
	}
}

/// No-op `MoneyMarketOperations` used during benchmarks. Returns 1:1 for
/// `supply` / `withdraw` so the pallet's `actual_minted` accounting stays
/// well-defined without invoking the EVM. The runtime swaps this in for
/// `AaveMoneyMarket` under `runtime-benchmarks` (see `assets.rs`).
#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkMoneyMarket;

#[cfg(feature = "runtime-benchmarks")]
impl MoneyMarketOperations<AccountId, AssetId, Balance> for BenchmarkMoneyMarket {
	fn supply(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}

	fn withdraw(_who: &AccountId, _underlying_asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		Ok(amount)
	}

	fn balance_of(_who: &AccountId) -> Balance {
		0
	}
}

// ---------------------------------------------------------------------------
// pallet-gigahdx-rewards wiring
// ---------------------------------------------------------------------------

/// Per-track reward percentage table. Tracks are defined in
/// `governance/tracks.rs`:
/// - `0` (root) → 10%
/// - `1` (whitelisted_caller) → 8%
/// - `5` (treasurer) → 5%
/// - any other track → 3% (default)
pub struct TrackRewardConfig;

impl TrackRewardTable<u16> for TrackRewardConfig {
	fn reward_percentage(track_id: u16) -> Permill {
		match track_id {
			0 => Permill::from_percent(10),
			1 => Permill::from_percent(8),
			5 => Permill::from_percent(5),
			_ => Permill::from_percent(3),
		}
	}
}

/// Track-id inspector backed by `pallet_referenda::ReferendumInfoFor`.
///
/// Only `ReferendumInfo::Ongoing(status)` exposes the track id directly on
/// this `polkadot-sdk` version. For all completed variants the track is not
/// preserved on the info entry; the rewards pallet keeps its own
/// `ReferendumTracks` cache populated during `on_before_vote` and falls back
/// to that when `track_of` returns `None`.
pub struct RuntimeReferenda;

impl ReferendaTrackInspect<ReferendumIndex, u16> for RuntimeReferenda {
	fn track_of(ref_index: ReferendumIndex) -> Option<u16> {
		match pallet_referenda::ReferendumInfoFor::<Runtime>::get(ref_index)? {
			ReferendumInfo::Ongoing(status) => Some(status.track),
			// Completed variants do not carry the track id on this SDK version.
			ReferendumInfo::Approved(..)
			| ReferendumInfo::Rejected(..)
			| ReferendumInfo::Cancelled(..)
			| ReferendumInfo::TimedOut(..)
			| ReferendumInfo::Killed(_) => None,
		}
	}
}

/// `ExternalClaims` impl: sum of HDX claimed by other pallets that should NOT
/// overlap with a gigahdx stake. `ghdxlock` is excluded because the pallet
/// accounts for it from its own ledger; `pyconvot` is excluded because a
/// conviction vote is intentionally permitted to share HDX with a stake.
pub struct HdxExternalClaims;

impl pallet_gigahdx::traits::ExternalClaims<AccountId> for HdxExternalClaims {
	fn on(who: &AccountId) -> Balance {
		const ALLOWED_OVERLAP: &[LockIdentifier] = &[*b"ghdxlock", *b"pyconvot"];
		pallet_balances::Locks::<Runtime>::get(who)
			.iter()
			.filter(|l| !ALLOWED_OVERLAP.contains(&l.id))
			.map(|l| l.amount)
			.fold(0, Balance::saturating_add)
	}
}

/// Adapter wiring `pallet_gigahdx::migrate` to the legacy NFT staking pallet.
pub struct LegacyStakingMigrator;

impl pallet_gigahdx::traits::LegacyStakeMigrator<AccountId> for LegacyStakingMigrator {
	fn force_unstake(who: &AccountId) -> Result<Balance, sp_runtime::DispatchError> {
		pallet_staking::Pallet::<Runtime>::force_unstake(who)
	}
}

/// `ExternalClaims` impl for the legacy staking pallet. Mirrors
/// `HdxExternalClaims` but with the legacy pallet's exclusions:
/// `stk_stks` (its own lock — already deducted via the position),
/// `ormlvest` (vesting — already deducted via the `Vesting` config),
/// `pyconvot` (governance overlap allowed). Everything else — `ghdxlock`
/// in particular — counts and blocks legacy staking from re-pledging
/// HDX already claimed elsewhere.
pub struct LegacyStakingExternalClaims;

impl pallet_staking::traits::ExternalClaims<AccountId> for LegacyStakingExternalClaims {
	fn on(who: &AccountId) -> Balance {
		const ALLOWED_OVERLAP: &[LockIdentifier] = &[*b"stk_stks", *b"ormlvest", *b"pyconvot"];
		pallet_balances::Locks::<Runtime>::get(who)
			.iter()
			.filter(|l| !ALLOWED_OVERLAP.contains(&l.id))
			.map(|l| l.amount)
			.fold(0, Balance::saturating_add)
	}
}
