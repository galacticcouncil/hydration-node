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
	inspect::MultiInspectAdapter, EmaOraclePriceAdapter, FreezableNFT, OmnipoolHookAdapter, OracleAssetVolumeProvider,
	OraclePriceProviderAdapterForOmnipool, PriceAdjustmentAdapter, VestingInfo,
};
use hydradx_adapters::{RelayChainBlockHashProvider, RelayChainBlockNumberProvider};
use hydradx_traits::{AssetKind, OraclePeriod, Source};
use pallet_currencies::BasicCurrencyAdapter;
use pallet_omnipool::traits::EnsurePriceWithin;
use pallet_otc::NamedReserveIdentifier;
use pallet_transaction_multi_payment::{AddTxAssetOnAccount, RemoveTxAssetOnKilled};
use primitives::constants::time::DAYS;
use primitives::constants::{
	chain::OMNIPOOL_SOURCE,
	currency::{NATIVE_EXISTENTIAL_DEPOSIT, UNITS},
};

use frame_support::{
	parameter_types,
	sp_runtime::traits::One,
	sp_runtime::{FixedU128, Perbill, Permill},
	traits::{AsEnsureOriginWithArg, ConstU32, Contains, EnsureOrigin, NeverEnsureOrigin},
	BoundedVec, PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use orml_traits::currency::MutationHooks;
use orml_traits::GetByKey;
use pallet_dynamic_fees::types::FeeParams;
use pallet_staking::types::Action;
use pallet_staking::SigmoidPercentage;

parameter_types! {
	pub const NativeExistentialDeposit: u128 = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = Treasury;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = NativeExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = weights::balances::HydraWeight<Runtime>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
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
	pub const StableAssetId: AssetId = 2;
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
	type StableCoinAssetId = StableAssetId;
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
	type OmnipoolHooks = OmnipoolHookAdapter<Self::RuntimeOrigin, LRNA, Runtime>;
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
	/// 20 seems a decent upper bound for the forseeable future.
	type MaxUniqueEntries = ConstU32<20>;
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

parameter_types! {
	pub MinBudgetInNativeCurrency: Balance = 1000 * UNITS;
	pub MaxSchedulesPerBlock: u32 = 20;
	pub MaxPriceDifference: Permill = Permill::from_rational(15u32, 1000u32);
	pub NamedReserveId: NamedReserveIdentifier = *b"dcaorder";
	pub MaxNumberOfRetriesOnError: u8 = 3;
}

impl pallet_dca::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type TechnicalOrigin = SuperMajorityTechCommittee;
	type Currencies = Currencies;
	type RelayChainBlockHashProvider = RelayChainBlockHashProviderAdapter<Runtime>;
	type RandomnessProvider = DCA;
	type OraclePriceProvider = OraclePriceProviderAdapterForOmnipool<AssetId, EmaOracle, LRNA>;
	type SpotPriceProvider = Omnipool;
	type MaxPriceDifferenceBetweenBlocks = MaxPriceDifference;
	type MaxSchedulePerBlock = MaxSchedulesPerBlock;
	type MaxNumberOfRetriesOnError = MaxNumberOfRetriesOnError;
	type NativeAssetId = NativeAssetId;
	type MinBudgetInNativeCurrency = MinBudgetInNativeCurrency;
	type MinimumTradingLimit = MinTradingLimit;
	type FeeReceiver = TreasuryAccount;
	type NamedReserveId = NamedReserveId;
	type WeightToFee = WeightToFee;
	type WeightInfo = weights::dca::HydraWeight<Runtime>;
}

parameter_types! {
	pub const MaxNumberOfTrades: u8 = 5;
}

impl pallet_route_executor::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type MaxNumberOfTrades = MaxNumberOfTrades;
	type Currency = MultiInspectAdapter<AccountId, AssetId, Balance, Balances, Tokens, NativeAssetId>;
	type AMM = Omnipool;
	type WeightInfo = weights::route_executor::HydraWeight<Runtime>;
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
		min_fee: Permill::from_rational(25u32,10000u32),
		max_fee: Permill::from_rational(4u32,1000u32),
		decay: FixedU128::from_rational(5,1000000),
		amplification: FixedU128::one(),
	};

	pub ProtocolFeeParams: FeeParams<Permill> = FeeParams{
		min_fee: Permill::from_rational(5u32,10000u32),
		max_fee: Permill::from_rational(1u32,1000u32),
		decay: FixedU128::from_rational(5,1000000),
		amplification: FixedU128::one(),
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
	type AssetId = AssetId;
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
	//This value is only for rococo, it should be 1 day in prod
	pub const PeriodLength: BlockNumber = 1;
	pub const TimePointsW:Permill =  Permill::from_percent(100);
	pub const ActionPointsW: Perbill = Perbill::from_parts(4_400);
	pub const TimePointsPerPeriod: u8 = 1;
	pub const CurrentStakeWeight: u8 = 2;
	pub const UnclaimablePeriods: BlockNumber = 1;
	pub const PointPercentage: FixedU128 = FixedU128::from_rational(2,100);
	pub const OneHDX: Balance = primitives::constants::currency::UNITS;
}

pub struct ActionMultiplier;

impl GetByKey<Action, u32> for ActionMultiplier {
	fn get(k: &Action) -> u32 {
		match k {
			Action::DemocracyVote => 1u32,
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
	type ActionMultiplier = ActionMultiplier;
	type Vesting = VestingInfo<Runtime>;
	type RewardedVoteUnit = OneHDX;
	type WeightInfo = weights::staking::HydraWeight<Runtime>;
}
