// This file is part of HydraDX-node.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use crate::system::NativeAssetId;

use hydradx_adapters::{
	AssetFeeOraclePriceProvider, EmaOraclePriceAdapter, FreezableNFT, MultiCurrencyLockedBalance, OmnipoolHookAdapter,
	OracleAssetVolumeProvider, PriceAdjustmentAdapter, StableswapHooksAdapter, VestingInfo,
};

use hydradx_adapters::{RelayChainBlockHashProvider, RelayChainBlockNumberProvider};
use hydradx_traits::{
	router::{inverse_route, PoolType, Trade},
	AccountIdFor, AssetKind, AssetPairAccountIdFor, OnTradeHandler, OraclePeriod, Source,
};
use pallet_currencies::BasicCurrencyAdapter;
use pallet_omnipool::{
	traits::{EnsurePriceWithin, OmnipoolHooks},
	weights::WeightInfo as OmnipoolWeights,
};
use pallet_otc::NamedReserveIdentifier;
use pallet_stableswap::weights::WeightInfo as StableswapWeights;
use pallet_transaction_multi_payment::{AddTxAssetOnAccount, RemoveTxAssetOnKilled};
use primitives::constants::chain::XYK_SOURCE;
use primitives::constants::time::DAYS;
use primitives::constants::{
	chain::OMNIPOOL_SOURCE,
	currency::{NATIVE_EXISTENTIAL_DEPOSIT, UNITS},
};

use core::ops::RangeInclusive;
use frame_support::{
	parameter_types,
	sp_runtime::app_crypto::sp_core::crypto::UncheckedFrom,
	sp_runtime::traits::{One, PhantomData},
	sp_runtime::{FixedU128, Perbill, Permill},
	traits::{
		AsEnsureOriginWithArg, ConstU32, Contains, Currency, EnsureOrigin, Imbalance, NeverEnsureOrigin, OnUnbalanced,
	},
	BoundedVec, PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use orml_traits::currency::MutationHooks;
use orml_traits::{GetByKey, MultiCurrency};
use pallet_dynamic_fees::types::FeeParams;
use pallet_lbp::weights::WeightInfo as LbpWeights;
use pallet_route_executor::{weights::WeightInfo as RouterWeights, AmmTradeWeights, MAX_NUMBER_OF_TRADES};
use pallet_staking::types::Action;
use pallet_staking::SigmoidPercentage;
use pallet_xyk::weights::WeightInfo as XykWeights;
use sp_runtime::{DispatchError, FixedPointNumber};
use sp_std::num::NonZeroU16;

parameter_types! {
	pub const NativeExistentialDeposit: u128 = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

// pallet-treasury did not impl OnUnbalanced<Credit>, need an adapter to handle dust.
type CreditOf = frame_support::traits::fungible::Credit<<Runtime as frame_system::Config>::AccountId, Balances>;
type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;
pub struct DustRemovalAdapter;
impl OnUnbalanced<CreditOf> for DustRemovalAdapter {
	fn on_nonzero_unbalanced(amount: CreditOf) {
		let new_amount = NegativeImbalance::new(amount.peek());
		Treasury::on_nonzero_unbalanced(new_amount);
	}
}

parameter_types! {
	pub const MaxHolds: u32 = 0;
	pub const MaxFreezes: u32 = 0;
}

impl pallet_balances::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::balances::HydraWeight<Runtime>;
	type Balance = Balance;
	type DustRemoval = DustRemovalAdapter;
	type ExistentialDeposit = NativeExistentialDeposit;
	type AccountStore = System;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = ();
	type FreezeIdentifier = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type MaxHolds = MaxHolds;
	type MaxFreezes = MaxFreezes;
}

pub struct CurrencyHooks;
impl MutationHooks<AccountId, AssetId, Balance> for CurrencyHooks {
	type OnDust = Duster;
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = AddTxAssetOnAccount<Runtime>;
	type OnKilledTokenAccount = RemoveTxAssetOnKilled<Runtime>;
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = weights::tokens::HydraWeight<Runtime>;
	type ExistentialDeposits = AssetRegistry;
	type CurrencyHooks = CurrencyHooks;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = DustRemovalWhitelist;
}

// The latest versions of the orml-currencies pallet don't emit events.
// The infrastructure relies on the events from this pallet, so we use the latest version of
// the pallet that contains and emit events and was updated to the polkadot version we use.
impl pallet_currencies::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
	type GetNativeCurrencyId = NativeAssetId;
	type WeightInfo = weights::currencies::HydraWeight<Runtime>;
}

pub struct RootAsVestingPallet;
impl EnsureOrigin<RuntimeOrigin> for RootAsVestingPallet {
	type Success = AccountId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o).and_then(|o| match o {
			RawOrigin::Root => Ok(VestingPalletId::get().into_account_truncating()),
			r => Err(RuntimeOrigin::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		Ok(RuntimeOrigin::from(RawOrigin::Signed(zero_account_id)))
	}
}

parameter_types! {
	pub MinVestedTransfer: Balance = 100;
	pub const MaxVestingSchedules: u32 = 100;
	pub const VestingPalletId: PalletId = PalletId(*b"py/vstng");
}

impl orml_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type MinVestedTransfer = MinVestedTransfer;
	type VestedTransferOrigin = RootAsVestingPallet;
	type WeightInfo = weights::vesting::HydraWeight<Runtime>;
	type MaxVestingSchedules = MaxVestingSchedules;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
}

parameter_types! {
	pub ClaimMessagePrefix: &'static [u8] = b"I hereby claim all my HDX tokens to wallet:";
}

impl pallet_claims::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Prefix = ClaimMessagePrefix;
	type WeightInfo = weights::claims::HydraWeight<Runtime>;
	type Currency = Balances;
	type CurrencyBalance = Balance;
}

parameter_types! {
	pub const RegistryStrLimit: u32 = 32;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

impl pallet_asset_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = SuperMajorityTechCommittee;
	type AssetId = AssetId;
	type Balance = Balance;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStrLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type NativeAssetId = NativeAssetId;
	type WeightInfo = weights::registry::HydraWeight<Runtime>;
}

parameter_types! {
	pub const CollectionDeposit: Balance = 0;
	pub const ItemDeposit: Balance = 0;
	pub const KeyLimit: u32 = 256;	// Max 256 bytes per key
	pub const ValueLimit: u32 = 1024;	// Max 1024 bytes per value
	pub const UniquesMetadataDepositBase: Balance = 1_000 * UNITS;
	pub const AttributeDepositBase: Balance = UNITS;
	pub const DepositPerByte: Balance = UNITS;
	pub const UniquesStringLimit: u32 = 72;
}

impl pallet_uniques::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Balances;
	type ForceOrigin = MajorityOfCouncil;
	// Standard collection creation is disallowed
	type CreateOrigin = AsEnsureOriginWithArg<NeverEnsureOrigin<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = UniquesStringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const LRNA: AssetId = 1;
	pub const MinTradingLimit : Balance = 1_000u128;
	pub const MinPoolLiquidity: Balance = 1_000_000u128;
	pub const MaxInRatio: Balance = 3u128;
	pub const MaxOutRatio: Balance = 3u128;
	pub const OmnipoolCollectionId: CollectionId = 1337u128;
	pub const EmaOracleSpotPriceLastBlock: OraclePeriod = OraclePeriod::LastBlock;
	pub const EmaOracleSpotPriceShort: OraclePeriod = OraclePeriod::Short;
	pub const OmnipoolMaxAllowedPriceDifference: Permill = Permill::from_percent(1);
	pub MinimumWithdrawalFee: Permill = Permill::from_rational(1u32,10000);
}

impl pallet_omnipool::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Currencies;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type AssetRegistry = AssetRegistry;
	type HdxAssetId = NativeAssetId;
	type HubAssetId = LRNA;
	type MinWithdrawalFee = MinimumWithdrawalFee;
	type MinimumTradingLimit = MinTradingLimit;
	type MinimumPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type PositionItemId = ItemId;
	type CollectionId = CollectionId;
	type NFTCollectionId = OmnipoolCollectionId;
	type NFTHandler = Uniques;
	type WeightInfo = weights::omnipool::HydraWeight<Runtime>;
	type OmnipoolHooks = OmnipoolHookAdapter<Self::RuntimeOrigin, NativeAssetId, LRNA, Runtime>;
	type PriceBarrier = (
		EnsurePriceWithin<
			AccountId,
			AssetId,
			EmaOraclePriceAdapter<EmaOracleSpotPriceLastBlock, Runtime>,
			OmnipoolMaxAllowedPriceDifference,
			CircuitBreakerWhitelist,
		>,
		EnsurePriceWithin<
			AccountId,
			AssetId,
			EmaOraclePriceAdapter<EmaOracleSpotPriceShort, Runtime>,
			OmnipoolMaxAllowedPriceDifference,
			CircuitBreakerWhitelist,
		>,
	);
	type ExternalPriceOracle = EmaOraclePriceAdapter<EmaOracleSpotPriceShort, Runtime>;
	type Fee = pallet_dynamic_fees::UpdateAndRetrieveFees<Runtime>;
}

pub struct CircuitBreakerWhitelist;

impl Contains<AccountId> for CircuitBreakerWhitelist {
	fn contains(a: &AccountId) -> bool {
		<PalletId as AccountIdConversion<AccountId>>::into_account_truncating(&TreasuryPalletId::get()) == *a
	}
}

parameter_types! {
	pub const DefaultMaxNetTradeVolumeLimitPerBlock: (u32, u32) = (5_000, 10_000);	// 50%
	pub const DefaultMaxLiquidityLimitPerBlock: Option<(u32, u32)> = Some((500, 10_000));	// 5%
}

impl pallet_circuit_breaker::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type WhitelistedAccounts = CircuitBreakerWhitelist;
	type DefaultMaxNetTradeVolumeLimitPerBlock = DefaultMaxNetTradeVolumeLimitPerBlock;
	type DefaultMaxAddLiquidityLimitPerBlock = DefaultMaxLiquidityLimitPerBlock;
	type DefaultMaxRemoveLiquidityLimitPerBlock = DefaultMaxLiquidityLimitPerBlock;
	type OmnipoolHubAsset = LRNA;
	type WeightInfo = weights::circuit_breaker::HydraWeight<Runtime>;
}

parameter_types! {
	pub SupportedPeriods: BoundedVec<OraclePeriod, ConstU32<{ pallet_ema_oracle::MAX_PERIODS }>> = BoundedVec::truncate_from(vec![
		OraclePeriod::LastBlock, OraclePeriod::Short, OraclePeriod::TenMinutes]);
}

impl pallet_ema_oracle::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::ema_oracle::HydraWeight<Runtime>;
	/// The definition of the oracle time periods currently assumes a 6 second block time.
	/// We use the parachain blocks anyway, because we want certain guarantees over how many blocks correspond
	/// to which smoothing factor.
	type BlockNumberProvider = System;
	type SupportedPeriods = SupportedPeriods;
	/// With every asset trading against LRNA we will only have as many pairs as there will be assets, so
	/// 40 seems a decent upper bound for the forseeable future.
	///
	type MaxUniqueEntries = ConstU32<40>;
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a) || pallet_duster::DusterWhitelist::<Runtime>::contains(a)
	}
}

parameter_types! {
	pub const DustingReward: u128 = 0;
}

impl pallet_duster::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type MultiCurrency = Currencies;
	type MinCurrencyDeposits = AssetRegistry;
	type Reward = DustingReward;
	type NativeCurrencyId = NativeAssetId;
	type BlacklistUpdateOrigin = SuperMajorityTechCommittee;
	type WeightInfo = ();
}

parameter_types! {
	pub const OmniWarehouseLMPalletId: PalletId = PalletId(*b"OmniWhLM");
	#[derive(PartialEq, Eq)]
	pub const MaxEntriesPerDeposit: u8 = 5; //NOTE: Rebenchmark when this change, TODO:
	pub const MaxYieldFarmsPerGlobalFarm: u8 = 50; //NOTE: Includes deleted/destroyed farms, TODO:
	pub const MinPlannedYieldingPeriods: BlockNumber = 14_440;  //1d with 6s blocks, TODO:
	pub const MinTotalFarmRewards: Balance = NATIVE_EXISTENTIAL_DEPOSIT * 100; //TODO:
}

type OmnipoolLiquidityMiningInstance = warehouse_liquidity_mining::Instance1;
impl warehouse_liquidity_mining::Config<OmnipoolLiquidityMiningInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Currencies;
	type PalletId = OmniWarehouseLMPalletId;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
	type AmmPoolId = AssetId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type AssetRegistry = AssetRegistry;
	type NonDustableWhitelistHandler = Duster;
	type PriceAdjustment = PriceAdjustmentAdapter<Runtime, OmnipoolLiquidityMiningInstance>;
}

parameter_types! {
	pub const OmniLMPalletId: PalletId = PalletId(*b"Omni//LM");
	pub const OmnipoolLMCollectionId: CollectionId = 2584_u128;
	pub const OmnipoolLMOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
	pub const OmnipoolLMOracleSource: Source = OMNIPOOL_SOURCE;
}

impl pallet_omnipool_liquidity_mining::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type CreateOrigin = AllTechnicalCommitteeMembers;
	type PalletId = OmniLMPalletId;
	type NFTCollectionId = OmnipoolLMCollectionId;
	type NFTHandler = Uniques;
	type LiquidityMiningHandler = OmnipoolWarehouseLM;
	type OracleSource = OmnipoolLMOracleSource;
	type OraclePeriod = OmnipoolLMOraclePeriod;
	type PriceOracle = EmaOracle;
	type WeightInfo = weights::omnipool_lm::HydraWeight<Runtime>;
}

// The reason why there is difference between PROD and benchmark is that it is not possible
// to set validation data in parachain system pallet in the benchmarks.
// So for benchmarking, we mock it out and return some hardcoded parent hash
pub struct RelayChainBlockHashProviderAdapter<Runtime>(sp_std::marker::PhantomData<Runtime>);

#[cfg(not(feature = "runtime-benchmarks"))]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		let validation_data = cumulus_pallet_parachain_system::Pallet::<Runtime>::validation_data();
		match validation_data {
			Some(data) => Some(data.parent_head.hash()),
			None => None,
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		None
	}
}

use hydradx_traits::pools::SpotPriceProvider;
#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::PriceOracle;

#[cfg(feature = "runtime-benchmarks")]
use hydra_dx_math::ema::EmaPrice;

#[cfg(feature = "runtime-benchmarks")]
pub struct DummyOraclePriceProvider;
#[cfg(feature = "runtime-benchmarks")]
impl PriceOracle<AssetId> for DummyOraclePriceProvider {
	type Price = EmaPrice;

	fn price(_route: &[Trade<AssetId>], _period: OraclePeriod) -> Option<Self::Price> {
		Some(EmaPrice::one())
	}
}

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::OraclePriceProvider;

#[cfg(feature = "runtime-benchmarks")]
pub struct DummySpotPriceProvider;
#[cfg(feature = "runtime-benchmarks")]
impl SpotPriceProvider<AssetId> for DummySpotPriceProvider {
	type Price = FixedU128;

	fn pair_exists(_asset_a: AssetId, _asset_b: AssetId) -> bool {
		true
	}

	fn spot_price(_asset_a: AssetId, _asset_b: AssetId) -> Option<Self::Price> {
		Some(FixedU128::one())
	}
}

parameter_types! {
	pub MinBudgetInNativeCurrency: Balance = 1000 * UNITS;
	pub MaxSchedulesPerBlock: u32 = 20;
	pub MaxPriceDifference: Permill = Permill::from_rational(15u32, 1000u32);
	pub NamedReserveId: NamedReserveIdentifier = *b"dcaorder";
	pub MaxNumberOfRetriesOnError: u8 = 3;
	pub DCAOraclePeriod: OraclePeriod = OraclePeriod::Short;

}

impl pallet_dca::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type Currencies = Currencies;
	type RelayChainBlockHashProvider = RelayChainBlockHashProviderAdapter<Runtime>;
	type RandomnessProvider = DCA;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type OraclePriceProvider = OraclePriceProvider<AssetId, EmaOracle, LRNA>;
	#[cfg(feature = "runtime-benchmarks")]
	type OraclePriceProvider = DummyOraclePriceProvider;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type RouteExecutor = Router;
	#[cfg(feature = "runtime-benchmarks")]
	type RouteExecutor = pallet_route_executor::DummyRouter<Runtime>;
	type RouteProvider = Router;
	type MaxPriceDifferenceBetweenBlocks = MaxPriceDifference;
	type MaxSchedulePerBlock = MaxSchedulesPerBlock;
	type MaxNumberOfRetriesOnError = MaxNumberOfRetriesOnError;
	type NativeAssetId = NativeAssetId;
	type MinBudgetInNativeCurrency = MinBudgetInNativeCurrency;
	type MinimumTradingLimit = MinTradingLimit;
	type FeeReceiver = TreasuryAccount;
	type NamedReserveId = NamedReserveId;
	type WeightToFee = WeightToFee;
	type AmmTradeWeights = RouterWeightInfo;
	type WeightInfo = weights::dca::HydraWeight<Runtime>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type NativePriceOracle = AssetFeeOraclePriceProvider<
		NativeAssetId,
		MultiTransactionPayment,
		Router,
		OraclePriceProvider<AssetId, EmaOracle, LRNA>,
		MultiTransactionPayment,
		DCAOraclePeriod,
	>;
	#[cfg(feature = "runtime-benchmarks")]
	type NativePriceOracle = AssetFeeOraclePriceProvider<
		NativeAssetId,
		MultiTransactionPayment,
		Router,
		DummyOraclePriceProvider,
		MultiTransactionPayment,
		DCAOraclePeriod,
	>;
}

// Provides weight info for the router. Router extrinsics can be executed with different AMMs, so we split the router weights into two parts:
// the router extrinsic overhead and the AMM weight.
pub struct RouterWeightInfo;
// Calculates the overhead of Router extrinsics. To do that, we benchmark Router::sell with single LBP trade and subtract the weight of LBP::sell.
// This allows us to calculate the weight of any route by adding the weight of AMM trades to the overhead of a router extrinsic.
impl RouterWeightInfo {
	pub fn sell_and_calculate_sell_trade_amounts_overhead_weight(
		num_of_calc_sell: u32,
		num_of_execute_sell: u32,
	) -> Weight {
		weights::route_executor::HydraWeight::<Runtime>::calculate_and_execute_sell_in_lbp(num_of_calc_sell)
			.saturating_sub(weights::lbp::HydraWeight::<Runtime>::router_execution_sell(
				num_of_calc_sell.saturating_add(num_of_execute_sell),
				num_of_execute_sell,
			))
	}

	pub fn buy_and_calculate_buy_trade_amounts_overhead_weight(
		num_of_calc_buy: u32,
		num_of_execute_buy: u32,
	) -> Weight {
		let router_weight = weights::route_executor::HydraWeight::<Runtime>::calculate_and_execute_buy_in_lbp(
			num_of_calc_buy,
			num_of_execute_buy,
		);
		// Handle this case separately. router_execution_buy provides incorrect weight for the case when only calculate_buy is executed.
		let lbp_weight = if (num_of_calc_buy, num_of_execute_buy) == (1, 0) {
			weights::lbp::HydraWeight::<Runtime>::calculate_buy()
		} else {
			weights::lbp::HydraWeight::<Runtime>::router_execution_buy(
				num_of_calc_buy.saturating_add(num_of_execute_buy),
				num_of_execute_buy,
			)
		};
		router_weight.saturating_sub(lbp_weight)
	}

	pub fn set_route_overweight() -> Weight {
		let number_of_times_calculate_sell_amounts_executed = 5; //4 calculations + in the validation
		let number_of_times_execute_sell_amounts_executed = 0; //We do have it once executed in the validation of the route, but it is without writing to database (as rolled back), and since we pay back successful set_route, we just keep this overhead

		let set_route_overweight = weights::route_executor::HydraWeight::<Runtime>::set_route_for_xyk();

		set_route_overweight.saturating_sub(weights::xyk::HydraWeight::<Runtime>::router_execution_sell(
			number_of_times_calculate_sell_amounts_executed,
			number_of_times_execute_sell_amounts_executed,
		))
	}
}

impl AmmTradeWeights<Trade<AssetId>> for RouterWeightInfo {
	// Used in Router::sell extrinsic, which calls AMM::calculate_sell and AMM::execute_sell
	fn sell_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_sell is executed
		let e = 1; // number of times AMM::execute_sell is executed

		for trade in route {
			weight.saturating_accrue(Self::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_trade_weight(),
					)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_liquidity_changed_weight(),
					),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in Router::buy extrinsic, which calls AMM::calculate_buy and AMM::execute_buy
	fn buy_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_buy is executed
		let e = 1; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_trade_weight(),
					)
					.saturating_add(
						<OmnipoolHookAdapter<RuntimeOrigin, NativeAssetId, LRNA, Runtime> as OmnipoolHooks<
							RuntimeOrigin,
							AccountId,
							AssetId,
							Balance,
						>>::on_liquidity_changed_weight(),
					),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in DCA::schedule extrinsic, which calls Router::calculate_buy_trade_amounts
	fn calculate_buy_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 1; // number of times AMM::calculate_buy is executed
		let e = 0; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(1, 0));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in DCA::on_initialize for Order::Sell, which calls Router::calculate_sell_trade_amounts and Router::sell.
	fn sell_and_calculate_sell_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 2; // number of times AMM::calculate_sell is executed
		let e = 1; // number of times AMM::execute_sell is executed

		for trade in route {
			weight.saturating_accrue(Self::sell_and_calculate_sell_trade_amounts_overhead_weight(1, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_sell(c, e),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_sell(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	// Used in DCA::on_initialize for Order::Buy, which calls 2 * Router::calculate_buy_trade_amounts and Router::buy.
	fn buy_and_calculate_buy_trade_amounts_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();
		let c = 3; // number of times AMM::calculate_buy is executed
		let e = 1; // number of times AMM::execute_buy is executed

		for trade in route {
			weight.saturating_accrue(Self::buy_and_calculate_buy_trade_amounts_overhead_weight(2, 1));

			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_buy(c, e),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_buy(c, e)
					.saturating_add(<Runtime as pallet_xyk::Config>::AMMHandler::on_trade_weight()),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}

	fn set_route_weight(route: &[Trade<AssetId>]) -> Weight {
		let mut weight = Weight::zero();

		//We ignore the calls for AMM:get_liquidty_depth, as the same logic happens in AMM calculation/execution

		//Overweight
		weight.saturating_accrue(Self::set_route_overweight());

		//Add a sell weight as we do a dry-run sell as validation
		weight.saturating_accrue(Self::sell_weight(route));

		//For the stored route we expect a worst case with max number of trades in the most expensive pool which is stableswap
		//We have have two sell calculation for that, normal and inverse
		weights::stableswap::HydraWeight::<Runtime>::router_execution_sell(2, 0)
			.checked_mul(MAX_NUMBER_OF_TRADES.into());

		//Calculate sell amounts for the new route
		for trade in route {
			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_sell(1, 0),
			};
			weight.saturating_accrue(amm_weight);
		}

		//Calculate sell amounts for the inversed new route
		for trade in inverse_route(route.to_vec()) {
			let amm_weight = match trade.pool {
				PoolType::Omnipool => weights::omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::LBP => weights::lbp::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::Stableswap(_) => weights::stableswap::HydraWeight::<Runtime>::router_execution_sell(1, 0),
				PoolType::XYK => weights::xyk::HydraWeight::<Runtime>::router_execution_sell(1, 0),
			};
			weight.saturating_accrue(amm_weight);
		}

		weight
	}
}

parameter_types! {
	pub const DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
}

impl pallet_route_executor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type Currency = FungibleCurrencies<Runtime>;
	type WeightInfo = RouterWeightInfo;
	type AMM = (Omnipool, Stableswap, XYK, LBP);
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type NativeAssetId = NativeAssetId;
}

parameter_types! {
	pub const ExistentialDepositMultiplier: u8 = 5;
}

impl pallet_otc::Config for Runtime {
	type AssetId = AssetId;
	type AssetRegistry = AssetRegistry;
	type Currency = Currencies;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposits = AssetRegistry;
	type ExistentialDepositMultiplier = ExistentialDepositMultiplier;
	type WeightInfo = weights::otc::HydraWeight<Runtime>;
}

// Dynamic fees
parameter_types! {
	pub AssetFeeParams: FeeParams<Permill> = FeeParams{
		min_fee: Permill::from_rational(25u32,10000u32), // 0.25%
		max_fee: Permill::from_rational(5u32,100u32),    // 5%
		decay: FixedU128::from_rational(1,100000),       // 0.001%
		amplification: FixedU128::from(2),               // 2
	};

	pub ProtocolFeeParams: FeeParams<Permill> = FeeParams{
		min_fee: Permill::from_rational(5u32,10000u32),  // 0.05%
		max_fee: Permill::from_rational(1u32,1000u32),   // 0.1%
		decay: FixedU128::from_rational(5,1000000),      // 0.0005%
		amplification: FixedU128::one(),                 // 1
	};

	pub const DynamicFeesOraclePeriod: OraclePeriod = OraclePeriod::Short;
}

impl pallet_dynamic_fees::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type Fee = Permill;
	type AssetId = AssetId;
	type Oracle = OracleAssetVolumeProvider<Runtime, LRNA, DynamicFeesOraclePeriod>;
	type AssetFeeParameters = AssetFeeParams;
	type ProtocolFeeParameters = ProtocolFeeParams;
}

// Stableswap
parameter_types! {
	pub StableswapAmplificationRange: RangeInclusive<NonZeroU16> = RangeInclusive::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(10_000).unwrap());
}

pub struct StableswapAccountIdConstructor<T: frame_system::Config>(PhantomData<T>);

impl<T: frame_system::Config> AccountIdFor<AssetId> for StableswapAccountIdConstructor<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	type AccountId = T::AccountId;

	fn from_assets(asset: &AssetId, identifier: Option<&[u8]>) -> Self::AccountId {
		let name = Self::name(asset, identifier);
		T::AccountId::unchecked_from(<T::Hashing as frame_support::sp_runtime::traits::Hash>::hash(&name[..]))
	}

	fn name(asset: &u32, identifier: Option<&[u8]>) -> Vec<u8> {
		let mut buf = identifier.map_or_else(Vec::new, |v| v.to_vec());
		buf.extend_from_slice(&(asset).to_le_bytes());
		buf
	}
}

use pallet_currencies::fungibles::FungibleCurrencies;

#[cfg(not(feature = "runtime-benchmarks"))]
use hydradx_adapters::price::OraclePriceProviderUsingRoute;

#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::price::PriceProvider;
use pallet_referrals::traits::Convert;
use pallet_referrals::{FeeDistribution, Level};
#[cfg(feature = "runtime-benchmarks")]
use pallet_stableswap::BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
use sp_runtime::DispatchResult;

#[cfg(feature = "runtime-benchmarks")]
pub struct RegisterAsset<T>(PhantomData<T>);

#[cfg(feature = "runtime-benchmarks")]
impl<T: pallet_asset_registry::Config> BenchmarkHelper<AssetId> for RegisterAsset<T> {
	fn register_asset(asset_id: AssetId, decimals: u8) -> DispatchResult {
		let asset_name = asset_id.to_le_bytes().to_vec();
		let name: BoundedVec<u8, RegistryStrLimit> = asset_name
			.clone()
			.try_into()
			.map_err(|_| pallet_asset_registry::Error::<T>::TooLong)?;
		AssetRegistry::register_asset(
			name,
			pallet_asset_registry::AssetType::<AssetId>::Token,
			1,
			Some(asset_id),
			None,
		)?;
		AssetRegistry::set_metadata(RuntimeOrigin::root(), asset_id, asset_name, decimals)?;

		Ok(())
	}
}

impl pallet_stableswap::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BlockNumberProvider = System;
	type AssetId = AssetId;
	type Currency = Currencies;
	type ShareAccountId = StableswapAccountIdConstructor<Runtime>;
	type AssetInspection = AssetRegistry;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type DustAccountHandler = Duster;
	type Hooks = StableswapHooksAdapter<Runtime>;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MinTradingLimit = MinTradingLimit;
	type AmplificationRange = StableswapAmplificationRange;
	type WeightInfo = weights::stableswap::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = RegisterAsset<Runtime>;
}

// Bonds
parameter_types! {
	pub ProtocolFee: Permill = Permill::from_percent(2);
	pub const BondsPalletId: PalletId = PalletId(*b"pltbonds");
}

pub struct AssetTypeWhitelist;
impl Contains<AssetKind> for AssetTypeWhitelist {
	fn contains(t: &AssetKind) -> bool {
		matches!(t, AssetKind::Token | AssetKind::XYK | AssetKind::StableSwap)
	}
}

impl pallet_bonds::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Currency = Currencies;
	type AssetRegistry = AssetRegistry;
	type ExistentialDeposits = AssetRegistry;
	type TimestampProvider = Timestamp;
	type PalletId = BondsPalletId;
	type IssueOrigin = EnsureSigned<AccountId>;
	type AssetTypeWhitelist = AssetTypeWhitelist;
	type ProtocolFee = ProtocolFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = weights::bonds::HydraWeight<Runtime>;
}

// Staking
parameter_types! {
	pub const StakingPalletId: PalletId = PalletId(*b"staking#");
	pub const MinStake: Balance = 1_000 * UNITS;
	pub const PeriodLength: BlockNumber = DAYS;
	pub const TimePointsW:Permill =  Permill::from_percent(100);
	pub const ActionPointsW: Perbill = Perbill::from_percent(20);
	pub const TimePointsPerPeriod: u8 = 1;
	pub const CurrentStakeWeight: u8 = 2;
	pub const UnclaimablePeriods: BlockNumber = 1;
	pub const PointPercentage: FixedU128 = FixedU128::from_rational(2,100);
}

pub struct PointsPerAction;

impl GetByKey<Action, u32> for PointsPerAction {
	fn get(k: &Action) -> u32 {
		match k {
			Action::DemocracyVote => 100_u32,
		}
	}
}

impl pallet_staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = MajorityOfCouncil;
	type AssetId = AssetId;
	type Currency = Currencies;
	type PeriodLength = PeriodLength;
	type PalletId = StakingPalletId;
	type NativeAssetId = NativeAssetId;
	type MinStake = MinStake;
	type TimePointsWeight = TimePointsW;
	type ActionPointsWeight = ActionPointsW;
	type TimePointsPerPeriod = TimePointsPerPeriod;
	type UnclaimablePeriods = UnclaimablePeriods;
	type CurrentStakeWeight = CurrentStakeWeight;
	type PayablePercentage = SigmoidPercentage<PointPercentage, ConstU32<2_000>>;
	type BlockNumberProvider = System;
	type PositionItemId = u128;
	type CollectionId = u128;
	type NFTCollectionId = ConstU128<2222>;
	type Collections = FreezableNFT<Runtime, Self::RuntimeOrigin>;
	type NFTHandler = Uniques;
	type MaxVotes = MaxVotes;
	type ReferendumInfo = pallet_staking::integrations::democracy::ReferendumStatus<Runtime>;
	type MaxPointsPerAction = PointsPerAction;
	type Vesting = VestingInfo<Runtime>;
	type WeightInfo = weights::staking::HydraWeight<Runtime>;

	#[cfg(feature = "runtime-benchmarks")]
	type MaxLocks = MaxLocks;
}

// LBP
pub struct AssetPairAccountId<T: frame_system::Config>(PhantomData<T>);
impl<T: frame_system::Config> AssetPairAccountIdFor<AssetId, T::AccountId> for AssetPairAccountId<T>
where
	T::AccountId: UncheckedFrom<T::Hash> + AsRef<[u8]>,
{
	fn from_assets(asset_a: AssetId, asset_b: AssetId, identifier: &str) -> T::AccountId {
		let mut buf: Vec<u8> = identifier.as_bytes().to_vec();

		if asset_a < asset_b {
			buf.extend_from_slice(&asset_a.to_le_bytes());
			buf.extend_from_slice(&asset_b.to_le_bytes());
		} else {
			buf.extend_from_slice(&asset_b.to_le_bytes());
			buf.extend_from_slice(&asset_a.to_le_bytes());
		}
		T::AccountId::unchecked_from(<T::Hashing as frame_support::sp_runtime::traits::Hash>::hash(&buf[..]))
	}
}

impl pallet_lbp::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Currencies;
	type LockedBalance = MultiCurrencyLockedBalance<Runtime, NativeAssetId>;
	type CreatePoolOrigin = SuperMajorityTechCommittee;
	type LBPWeightFunction = pallet_lbp::LBPWeightFunction;
	type AssetPairAccountId = AssetPairAccountId<Self>;
	type WeightInfo = weights::lbp::HydraWeight<Runtime>;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type BlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
}

parameter_types! {
	pub XYKExchangeFee: (u32, u32) = (3, 1_000);
	pub const DiscountedFee: (u32, u32) = (7, 10_000);
	pub const XYKOracleSourceIdentifier: Source = XYK_SOURCE;
}

impl pallet_xyk::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetRegistry = AssetRegistry;
	type AssetPairAccountId = AssetPairAccountId<Self>;
	type Currency = Currencies;
	type NativeAssetId = NativeAssetId;
	type WeightInfo = weights::xyk::HydraWeight<Runtime>;
	type GetExchangeFee = XYKExchangeFee;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CanCreatePool = pallet_lbp::DisallowWhenLBPPoolRunning<Runtime>;
	type AMMHandler = pallet_ema_oracle::OnActivityHandler<Runtime>;
	type DiscountedFee = DiscountedFee;
	type NonDustableWhitelistHandler = Duster;
	type OracleSource = XYKOracleSourceIdentifier;
}

parameter_types! {
	pub const ReferralsPalletId: PalletId = PalletId(*b"referral");
	pub RegistrationFee: (AssetId,Balance, AccountId)= (NativeAssetId::get(), 222_000_000_000_000, TreasuryAccount::get());
	pub const MaxCodeLength: u32 = 10;
	pub const MinCodeLength: u32 = 4;
	pub const ReferralsOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
	pub const ReferralsSeedAmount: Balance = 10_000_000_000_000;
	pub ReferralsExternalRewardAccount: Option<AccountId> = Some(StakingPalletId::get().into_account_truncating());
}

impl pallet_referrals::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type AssetId = AssetId;
	type Currency = FungibleCurrencies<Runtime>;
	type Convert = ConvertViaOmnipool<Omnipool>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type PriceProvider =
		OraclePriceProviderUsingRoute<Router, OraclePriceProvider<AssetId, EmaOracle, LRNA>, ReferralsOraclePeriod>;
	#[cfg(feature = "runtime-benchmarks")]
	type PriceProvider = ReferralsDummyPriceProvider;
	type RewardAsset = NativeAssetId;
	type PalletId = ReferralsPalletId;
	type RegistrationFee = RegistrationFee;
	type CodeLength = MaxCodeLength;
	type MinCodeLength = MinCodeLength;
	type LevelVolumeAndRewardPercentages = ReferralsLevelVolumeAndRewards;
	type ExternalAccount = ReferralsExternalRewardAccount;
	type SeedNativeAmount = ReferralsSeedAmount;
	type WeightInfo = weights::referrals::HydraWeight<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ReferralsBenchmarkHelper;
}

pub struct ConvertViaOmnipool<SP>(PhantomData<SP>);
impl<SP> Convert<AccountId, AssetId, Balance> for ConvertViaOmnipool<SP>
where
	SP: SpotPriceProvider<AssetId, Price = FixedU128>,
{
	type Error = DispatchError;

	fn convert(
		who: AccountId,
		asset_from: AssetId,
		asset_to: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error> {
		if amount < <Runtime as pallet_omnipool::Config>::MinimumTradingLimit::get() {
			return Err(pallet_referrals::Error::<Runtime>::ConversionMinTradingAmountNotReached.into());
		}
		let price = SP::spot_price(asset_to, asset_from).ok_or(pallet_referrals::Error::<Runtime>::PriceNotFound)?;
		let amount_to_receive = price.saturating_mul_int(amount);
		let min_expected = amount_to_receive
			.saturating_sub(Permill::from_percent(1).mul_floor(amount_to_receive))
			.max(1);
		let balance = Currencies::free_balance(asset_to, &who);
		let r = Omnipool::sell(
			RuntimeOrigin::signed(who.clone()),
			asset_from,
			asset_to,
			amount,
			min_expected,
		);
		if let Err(error) = r {
			if error == pallet_omnipool::Error::<Runtime>::ZeroAmountOut.into() {
				return Err(pallet_referrals::Error::<Runtime>::ConversionZeroAmountReceived.into());
			}
			return Err(error);
		}
		let balance_after = Currencies::free_balance(asset_to, &who);
		let received = balance_after.saturating_sub(balance);
		Ok(received)
	}
}

pub struct ReferralsLevelVolumeAndRewards;

impl GetByKey<Level, (Balance, FeeDistribution)> for ReferralsLevelVolumeAndRewards {
	fn get(k: &Level) -> (Balance, FeeDistribution) {
		let volume = match k {
			Level::Tier0 | Level::None => 0,
			Level::Tier1 => 305 * UNITS,
			Level::Tier2 => 4_583 * UNITS,
			Level::Tier3 => 61_111 * UNITS,
			Level::Tier4 => 763_888 * UNITS,
		};
		let rewards = match k {
			Level::None => FeeDistribution {
				referrer: Permill::zero(),
				trader: Permill::zero(),
				external: Permill::from_percent(50),
			},
			Level::Tier0 => FeeDistribution {
				referrer: Permill::from_percent(5),
				trader: Permill::from_percent(10),
				external: Permill::from_percent(35),
			},
			Level::Tier1 => FeeDistribution {
				referrer: Permill::from_percent(10),
				trader: Permill::from_percent(11),
				external: Permill::from_percent(29),
			},
			Level::Tier2 => FeeDistribution {
				referrer: Permill::from_percent(15),
				trader: Permill::from_percent(12),
				external: Permill::from_percent(23),
			},
			Level::Tier3 => FeeDistribution {
				referrer: Permill::from_percent(20),
				trader: Permill::from_percent(13),
				external: Permill::from_percent(17),
			},
			Level::Tier4 => FeeDistribution {
				referrer: Permill::from_percent(25),
				trader: Permill::from_percent(15),
				external: Permill::from_percent(10),
			},
		};
		(volume, rewards)
	}
}

#[cfg(feature = "runtime-benchmarks")]
use pallet_referrals::BenchmarkHelper as RefBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
pub struct ReferralsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl RefBenchmarkHelper<AssetId, Balance> for ReferralsBenchmarkHelper {
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance) {
		let asset_id: u32 = 1234u32;
		let asset_name = asset_id.to_le_bytes().to_vec();
		let name: BoundedVec<u8, RegistryStrLimit> = asset_name.clone().try_into().unwrap();

		AssetRegistry::register_asset(
			name,
			pallet_asset_registry::AssetType::<AssetId>::Token,
			1_000_000,
			Some(asset_id),
			None,
		)
		.unwrap();
		AssetRegistry::set_metadata(RuntimeOrigin::root(), asset_id, asset_name, 18).unwrap();

		let native_price = FixedU128::from_inner(1201500000000000);
		let asset_price = FixedU128::from_inner(45_000_000_000);

		Currencies::update_balance(
			RuntimeOrigin::root(),
			Omnipool::protocol_account(),
			NativeAssetId::get(),
			1_000_000_000_000_000_000,
		)
		.unwrap();

		Currencies::update_balance(
			RuntimeOrigin::root(),
			Omnipool::protocol_account(),
			asset_id,
			1_000_000_000_000_000_000_000_000,
		)
		.unwrap();

		Omnipool::add_token(
			RuntimeOrigin::root(),
			NativeAssetId::get(),
			native_price,
			Permill::from_percent(10),
			TreasuryAccount::get(),
		)
		.unwrap();

		Omnipool::add_token(
			RuntimeOrigin::root(),
			asset_id,
			asset_price,
			Permill::from_percent(10),
			TreasuryAccount::get(),
		)
		.unwrap();
		(1234, 1_000_000_000_000_000_000)
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub struct ReferralsDummyPriceProvider;

#[cfg(feature = "runtime-benchmarks")]
impl PriceProvider<AssetId> for ReferralsDummyPriceProvider {
	type Price = EmaPrice;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(EmaPrice::one());
		}
		Some(EmaPrice::new(1_000_000_000_000, 2_000_000_000_000_000_000))
	}
}
