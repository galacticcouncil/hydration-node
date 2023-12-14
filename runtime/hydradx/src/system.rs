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

use pallet_transaction_multi_payment::{DepositAll, TransferFees};
use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
use primitives::constants::{
	chain::{CORE_ASSET_ID, MAXIMUM_BLOCK_WEIGHT},
	currency::{deposit, CENTS, DOLLARS, MILLICENTS},
	time::{HOURS, SLOT_DURATION},
};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	sp_runtime::{
		traits::{ConstU32, IdentityLookup},
		FixedPointNumber, Perbill, Perquintill, RuntimeDebug,
	},
	traits::{ConstBool, Contains, InstanceFilter},
	weights::{
		constants::{BlockExecutionWeight, RocksDbWeight},
		ConstantMultiplier, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	},
	PalletId,
};
use hydradx_adapters::{OraclePriceProvider, RelayChainBlockNumberProvider};
use scale_info::TypeInfo;

pub struct CallFilter;
impl Contains<RuntimeCall> for CallFilter {
	fn contains(call: &RuntimeCall) -> bool {
		if matches!(
			call,
			RuntimeCall::System(_) | RuntimeCall::Timestamp(_) | RuntimeCall::ParachainSystem(_)
		) {
			// always allow
			// Note: this is done to avoid unnecessary check of paused storage.
			return true;
		}

		if pallet_transaction_pause::PausedTransactionFilter::<Runtime>::contains(call) {
			// if paused, dont allow!
			return false;
		}

		let hub_asset_id = <Runtime as pallet_omnipool::Config>::HubAssetId::get();

		// filter transfers of LRNA and omnipool assets to the omnipool account
		if let RuntimeCall::Tokens(orml_tokens::Call::transfer { dest, currency_id, .. })
		| RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive { dest, currency_id, .. })
		| RuntimeCall::Tokens(orml_tokens::Call::transfer_all { dest, currency_id, .. })
		| RuntimeCall::Currencies(pallet_currencies::Call::transfer { dest, currency_id, .. }) = call
		{
			// Lookup::lookup() is not necessary thanks to IdentityLookup
			if dest == &Omnipool::protocol_account() && (*currency_id == hub_asset_id || Omnipool::exists(*currency_id))
			{
				return false;
			}
		}
		// filter transfers of HDX to the omnipool account
		if let RuntimeCall::Balances(pallet_balances::Call::transfer { dest, .. })
		| RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive { dest, .. })
		| RuntimeCall::Balances(pallet_balances::Call::transfer_all { dest, .. })
		| RuntimeCall::Currencies(pallet_currencies::Call::transfer_native_currency { dest, .. }) = call
		{
			// Lookup::lookup() is not necessary thanks to IdentityLookup
			if dest == &Omnipool::protocol_account() {
				return false;
			}
		}

		// XYK pools with LRNA are not allowed
		if let RuntimeCall::XYK(pallet_xyk::Call::create_pool { asset_a, asset_b, .. }) = call {
			if *asset_a == hub_asset_id || *asset_b == hub_asset_id {
				return false;
			}
		}

		match call {
			RuntimeCall::PolkadotXcm(pallet_xcm::Call::send { .. }) => true,
			RuntimeCall::PolkadotXcm(_) => false,
			RuntimeCall::OrmlXcm(_) => false,
			_ => true,
		}
	}
}

/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	/// Block weights base values and limits.
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have an extra reserved space, so that they
			// are included even if block reachd `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT,
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub ExtrinsicBaseWeight: Weight = frame_support::weights::constants::ExtrinsicBaseWeight::get();
	pub const BlockHashCount: BlockNumber = 2400;
	/// Maximum length of block. Up to 5MB.
	pub BlockLength: frame_system::limits::BlockLength =
		frame_system::limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u16 = 63;
}

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = CallFilter;
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	/// The ubiquitous origin type.
	type RuntimeOrigin = RuntimeOrigin;
	/// The aggregated dispatch type that is available for extrinsics.
	type RuntimeCall = RuntimeCall;
	/// The index type for storing how many extrinsics an account has signed.
	type Nonce = Index;
	/// The index type for blocks.
	type Block = Block;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = IdentityLookup<AccountId>;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// The weight of the overhead invoked on the block import process, independent of the
	/// extrinsics included in that block.
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = weights::system::HydraWeight<Runtime>;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
	pub const NativeAssetId : AssetId = CORE_ASSET_ID;
}
impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::timestamp::HydraWeight<Runtime>;
}

parameter_types! {
	pub ReservedXcmpWeight: Weight = BlockWeights::get().max_block / 4;
	pub ReservedDmpWeight: Weight = BlockWeights::get().max_block / 4;
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = pallet_relaychain_info::OnValidationDataHandler<Runtime>;
	type SelfParaId = ParachainInfo;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpMessageHandler = DmpQueue;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases;
}

parameter_types! {
	pub const MaxAuthorities: u32 = 50;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 0;
	pub const MaxInvulnerables: u32 = 50;
}

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = MoreThanHalfCouncil;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = weights::collator_selection::HydraWeight<Runtime>;
	type MinEligibleCollators = ConstU32<4>;
}

parameter_types! {
	pub const Period: u32 = 4 * HOURS;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	// We wrap the session manager to give out rewards.
	type SessionManager = CollatorRewards;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <opaque::SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type WeightInfo = ();
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::utility::HydraWeight<Runtime>;
}

parameter_types! {
	pub const BasicDeposit: Balance = 5 * DOLLARS;
	pub const FieldDeposit: Balance = DOLLARS;
	pub const SubAccountDeposit: Balance = 5 * DOLLARS;
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = Treasury;
	type ForceOrigin = MoreThanHalfCouncil;
	type RegistrarOrigin = MoreThanHalfCouncil;
	type WeightInfo = weights::identity::HydraWeight<Runtime>;
}

/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum ProxyType {
	Any,
	CancelProxy,
	Governance,
	Transfer,
	Liquidity,
	LiquidityMining,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::CancelProxy => matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })),
			ProxyType::Governance => matches!(
				c,
				RuntimeCall::Democracy(..)
					| RuntimeCall::Council(..)
					| RuntimeCall::TechnicalCommittee(..)
					| RuntimeCall::Elections(..)
					| RuntimeCall::Treasury(..)
					| RuntimeCall::Tips(..)
					| RuntimeCall::Utility(..)
			),
			// Transfer group doesn't include cross-chain transfers
			ProxyType::Transfer => matches!(
				c,
				RuntimeCall::Balances(..) | RuntimeCall::Currencies(..) | RuntimeCall::Tokens(..)
			),
			ProxyType::Liquidity => matches!(
				c,
				RuntimeCall::Omnipool(pallet_omnipool::Call::add_liquidity { .. })
					| RuntimeCall::Omnipool(pallet_omnipool::Call::remove_liquidity { .. })
			),
			ProxyType::LiquidityMining => matches!(
				c,
				RuntimeCall::OmnipoolLiquidityMining(pallet_omnipool_liquidity_mining::Call::deposit_shares { .. })
					| RuntimeCall::OmnipoolLiquidityMining(
						pallet_omnipool_liquidity_mining::Call::redeposit_shares { .. }
					) | RuntimeCall::OmnipoolLiquidityMining(pallet_omnipool_liquidity_mining::Call::claim_rewards { .. })
					| RuntimeCall::OmnipoolLiquidityMining(
						pallet_omnipool_liquidity_mining::Call::withdraw_shares { .. }
					)
			),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			_ => false,
		}
	}
}

parameter_types! {
	pub ProxyDepositBase: Balance = deposit(1, 8);
	pub ProxyDepositFactor: Balance = deposit(0, 33);
	pub const MaxProxies: u16 = 32;
	pub AnnouncementDepositBase: Balance = deposit(1, 8);
	pub AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = weights::proxy::HydraWeight<Runtime>;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

parameter_types! {
	pub DepositBase: Balance = deposit(1, 88);
	pub DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

impl pallet_genesis_history::Config for Runtime {}

/// Parameterized slow adjusting fee updated based on
/// https://w3f-research.readthedocs.io/en/latest/polkadot/overview/2-token-economics.html?highlight=token%20economics#-2.-slow-adjusting-mechanism
pub type SlowAdjustingFeeUpdate<R> =
	TargetedFeeAdjustment<R, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier, MaximumMultiplier>;

pub struct WeightToFee;

impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, MAXIMUM_BLOCK_WEIGHT]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT
		let p = CENTS; // 1_000_000_000_000
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time()); // 7_919_840_000
		smallvec::smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q, // 124
		}]
	}
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	/// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(6, 100_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000u128);
	/// The maximum amount of the multiplier.
	pub MaximumMultiplier: Multiplier = Multiplier::saturating_from_integer(4);
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransferFees<Currencies, DepositAll<Runtime>, TreasuryAccount>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

impl pallet_transaction_multi_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AcceptedCurrencyOrigin = SuperMajorityTechCommittee;
	type Currencies = Currencies;
	type RouteProvider = Router;
	type OraclePriceProvider = OraclePriceProvider<AssetId, EmaOracle, LRNA>;
	type WeightInfo = weights::payment::HydraWeight<Runtime>;
	type WeightToFee = WeightToFee;
	type NativeAssetId = NativeAssetId;
}

impl pallet_relaychain_info::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RelaychainBlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
}

parameter_types! {
	pub const RewardPerCollator: Balance = 455_371_584_699_000; // 83333 HDX / 183 sessions
	//GalacticCouncil collators
	pub ExcludedCollators: Vec<AccountId> = vec![
		// 5G3t6yhAonQHGUEqrByWQPgP9R8fcSSL6Vujphc89ysdTpKF
		hex!["b0502e92d738d528922e8963b8a58a3c7c3b693db51b0972a6981836d67b8835"].into(),
		// 5CVBHPAjhcVVAvL3AYpa9MB6kWDwoJbBwu7q4MqbhKwNnrV4
		hex!["12aa36d6c1b055b9a7ab5d39f4fd9a9fe42912163c90e122fb7997e890a53d7e"].into(),
		// 5DFGmHjpxS6Xveg4YDw2hSp62JJ9h8oLCkeZUAoVR7hVtQ3k
		hex!["344b7693389189ad0be0c83630b02830a568f7cb0f2d4b3483bcea323cc85f70"].into(),
		// 5H178NL4DLM9DGgAgZz1kbrX2TReP3uPk7svPtsg1VcYnuXH
		hex!["da6e859211b1140369a73af533ecea4e4c0e985ad122ac4c663cc8b81d4fcd12"].into(),
		// 5Ca1iV2RNV253FzYJo12XtKJMPWCjv5CsPK9HdmwgJarD1sJ
		hex!["165a3c2eb21341bf170fd1fa728bd9a7d02b7dc3b4968a46f2b1d494ee8c2b5d"].into(),
	];
}

impl pallet_collator_rewards::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = AssetId;
	type Currency = Currencies;
	type RewardPerCollator = RewardPerCollator;
	type RewardCurrencyId = NativeAssetId;
	type ExcludedCollators = ExcludedCollators;
	// We wrap the ` SessionManager` implementation of `CollatorSelection` to get the collators that
	// we hand out rewards to.
	type SessionManager = CollatorSelection;
	type MaxCandidates = MaxInvulnerables;
}

impl pallet_transaction_pause::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = SuperMajorityTechCommittee;
	type WeightInfo = weights::transaction_pause::HydraWeight<Runtime>;
}
