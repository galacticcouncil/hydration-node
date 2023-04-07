// This file is part of HydraDX-node.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
#![allow(clippy::match_like_matches_macro)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::{Decode, Encode};
use frame_system::{EnsureRoot, RawOrigin};
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_core::OpaqueMetadata;
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{AccountIdConversion, BlakeTwo256, Block as BlockT, IdentityLookup},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, Perbill, Permill,
};
use sp_std::cmp::Ordering;
use sp_std::convert::From;
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

// A few exports that help ease life for downstream crates.
use frame_support::traits::AsEnsureOriginWithArg;
use frame_support::{
	construct_runtime,
	dispatch::DispatchClass,
	parameter_types,
	traits::{
		ConstU32, Contains, EnsureOrigin, Get, InstanceFilter, NeverEnsureOrigin, PrivilegeCmp, U128CurrencyToVote,
	},
	weights::{
		constants::{BlockExecutionWeight, RocksDbWeight},
		ConstantMultiplier, WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
	},
	BoundedVec,
};
use hydradx_traits::OraclePeriod;
use orml_traits::currency::MutationHooks;
use pallet_transaction_multi_payment::{AddTxAssetOnAccount, DepositAll, RemoveTxAssetOnKilled, TransferFees};
use pallet_transaction_payment::TargetedFeeAdjustment;
use primitives::{CollectionId, ItemId};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::traits::BlockNumberProvider;

use common_runtime::adapters::{EmaOraclePriceAdapter, OmnipoolHookAdapter};
pub use common_runtime::*;
use pallet_currencies::BasicCurrencyAdapter;

mod benchmarking;
mod migrations;
mod xcm;

pub use hex_literal::hex;
/// Import HydraDX pallets
pub use pallet_claims;
pub use pallet_genesis_history;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
	impl_opaque_keys! {
		pub struct SessionKeys {
			pub aura: Aura,
		}
	}
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("hydradx"),
	impl_name: create_runtime_str!("hydradx"),
	authoring_version: 1,
	spec_version: 140,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 0,
};

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

use smallvec::smallvec;

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
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q, // 124
		}]
	}
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a) || pallet_duster::DusterWhitelist::<Runtime>::contains(a)
	}
}

// Relay chain Block number provider.
// Reason why the implementation is different for benchmarks is that it is not possible
// to set or change the block number in a benchmark using parachain system pallet.
// That's why we revert to using the system pallet in the benchmark.
pub struct RelayChainBlockNumberProvider<T>(sp_std::marker::PhantomData<T>);

#[cfg(not(feature = "runtime-benchmarks"))]
impl<T: cumulus_pallet_parachain_system::Config> BlockNumberProvider for RelayChainBlockNumberProvider<T> {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		let maybe_data = cumulus_pallet_parachain_system::Pallet::<T>::validation_data();

		if let Some(data) = maybe_data {
			data.relay_parent_number
		} else {
			Self::BlockNumber::default()
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: frame_system::Config> BlockNumberProvider for RelayChainBlockNumberProvider<T> {
	type BlockNumber = <T as frame_system::Config>::BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		frame_system::Pallet::<T>::current_block_number()
	}
}

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

		// filter transfers of LRNA to the omnipool account
		if let RuntimeCall::Tokens(orml_tokens::Call::transfer { dest, currency_id, .. })
		| RuntimeCall::Tokens(orml_tokens::Call::transfer_keep_alive { dest, currency_id, .. })
		| RuntimeCall::Tokens(orml_tokens::Call::transfer_all { dest, currency_id, .. })
		| RuntimeCall::Currencies(pallet_currencies::Call::transfer { dest, currency_id, .. }) = call
		{
			// Lookup::lookup() is not necessary thanks to IdentityLookup
			if dest == &Omnipool::protocol_account()
				&& *currency_id == <Runtime as pallet_omnipool::Config>::HubAssetId::get()
			{
				return false;
			}
		}

		match call {
			RuntimeCall::PolkadotXcm(_) => false,
			RuntimeCall::OrmlXcm(_) => false,
			RuntimeCall::Uniques(_) => false,
			_ => true,
		}
	}
}

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
}

// Configure FRAME pallets to include in runtime.

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
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = IdentityLookup<AccountId>;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
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
	type SystemWeightInfo = common_runtime::weights::system::HydraWeight<Runtime>;
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::timestamp::HydraWeight<Runtime>;
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = Treasury;
	type ExistentialDeposit = NativeExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = weights::balances::HydraWeight<Runtime>;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
}

/// Parameterized slow adjusting fee updated based on
/// https://w3f-research.readthedocs.io/en/latest/polkadot/overview/2-token-economics.html?highlight=token%20economics#-2.-slow-adjusting-mechanism
pub type SlowAdjustingFeeUpdate<R> =
	TargetedFeeAdjustment<R, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier, MaximumMultiplier>;

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransferFees<Currencies, MultiTransactionPayment, DepositAll<Runtime>>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

// Parachain Config

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

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = TreasuryApproveOrigin;
	type RejectOrigin = MoreThanHalfCouncil;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type WeightInfo = weights::treasury::HydraWeight<Runtime>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = NeverEnsureOrigin<Balance>;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorSelection,);
}

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = MoreThanHalfCouncil;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = weights::collator_selection::HydraWeight<Runtime>;
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

impl pallet_preimage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::preimage::HydraWeight<Runtime>;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
	fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
		if left == right {
			return Some(Ordering::Equal);
		}

		match (left, right) {
			// Root is greater than anything.
			(OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
			// Check which one has more yes votes.
			(
				OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
				OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
			) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
			// For every other origin we don't care, as they are not used for `ScheduleOrigin`.
			_ => None,
		}
	}
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
	pub const NoPreimagePostponement: Option<u32> = Some(5 * MINUTES);
}
impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = MoreThanHalfCouncil;
	type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::scheduler::HydraWeight<Runtime>;
	type Preimages = Preimage;
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

impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = common_runtime::weights::council::HydraWeight<Runtime>;
}

impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = common_runtime::weights::technical_comittee::HydraWeight<Runtime>;
}

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = MoreThanHalfCouncil;
	type ExternalMajorityOrigin = MoreThanHalfCouncil;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = AllCouncilMembers;
	type FastTrackOrigin = MoreThanHalfTechCommittee;
	type InstantOrigin = AllTechnicalCommitteeMembers;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = MajorityOfCouncil;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = AllTechnicalCommitteeMembers;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
	type WeightInfo = weights::democracy::HydraWeight<Runtime>;
	type MaxProposals = MaxProposals;
	type VoteLockingPeriod = VoteLockingPeriod;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
}

impl pallet_elections_phragmen::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = ElectionsPhragmenPalletId;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = ();
	// Set to () if defined in chain spec
	type CurrencyToVote = U128CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type LoserCandidate = Treasury;
	type KickedMember = Treasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type MaxCandidates = MaxElectionCandidates;
	type MaxVoters = MaxElectionVoters;
	type WeightInfo = ();
}

impl pallet_tips::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type Tippers = Elections;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type WeightInfo = ();
}

/// ORML Configurations
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
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = DustRemovalWhitelist;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type CurrencyHooks = CurrencyHooks;
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
	fn successful_origin() -> RuntimeOrigin {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		RuntimeOrigin::from(RawOrigin::Signed(zero_account_id))
	}
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

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

/// HydraDX Pallets configurations

impl pallet_claims::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Prefix = ClaimMessagePrefix;
	type WeightInfo = weights::claims::HydraWeight<Runtime>;
	type CurrencyBalance = Balance;
}

impl pallet_genesis_history::Config for Runtime {}

parameter_types! {
	pub TreasuryAccount: AccountId = Treasury::account_id();
}

impl pallet_transaction_multi_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AcceptedCurrencyOrigin = SuperMajorityTechCommittee;
	type Currencies = Currencies;
	type SpotPriceProvider = Omnipool;
	type WeightInfo = weights::transaction_multi_payment::HydraWeight<Runtime>;
	type WithdrawFeeForSetCurrency = MultiPaymentCurrencySetFee;
	type WeightToFee = WeightToFee;
	type NativeAssetId = NativeAssetId;
	type FeeReceiver = TreasuryAccount;
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct AssetLocation(pub polkadot_xcm::v3::MultiLocation);

impl Default for AssetLocation {
	fn default() -> Self {
		AssetLocation(polkadot_xcm::v3::MultiLocation::default())
	}
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

impl pallet_relaychain_info::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RelaychainBlockNumberProvider = RelayChainBlockNumberProvider<Runtime>;
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
	type ExcludedCollators = ExcludedCollators;
	type RewardCurrencyId = NativeAssetId;
	// We wrap the ` SessionManager` implementation of `CollatorSelection` to get the collatrs that
	// we hand out rewards to.
	type SessionManager = CollatorSelection;
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
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	pub const LRNA: AssetId = 1;
	pub const StableAssetId: AssetId = 2;
	pub ProtocolFee: Permill = Permill::from_rational(5u32,10000u32);
	pub AssetFee: Permill = Permill::from_rational(25u32,10000u32);
	pub const MinTradingLimit : Balance = 1_000_000u128;
	pub const MinPoolLiquidity: Balance = 1_000_000u128;
	pub const MaxInRatio: Balance = 3u128;
	pub const MaxOutRatio: Balance = 3u128;
	pub const OmnipoolCollectionId: CollectionId = 1337u128;
	pub const EmaOracleSpotPriceLastBlock: OraclePeriod = OraclePeriod::LastBlock;
	pub const EmaOracleSpotPriceShort: OraclePeriod = OraclePeriod::Short;
	pub const OmnipoolMaxAllowedPriceDifference: Permill = Permill::from_percent(1);
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
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
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
}

impl pallet_transaction_pause::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = SuperMajorityTechCommittee;
	type WeightInfo = weights::transaction_pause::HydraWeight<Runtime>;
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

// constants need to be in scope to use as types
use pallet_ema_oracle::MAX_PERIODS;
use pallet_omnipool::traits::EnsurePriceWithin;

parameter_types! {
	pub SupportedPeriods: BoundedVec<OraclePeriod, ConstU32<MAX_PERIODS>> = BoundedVec::truncate_from(vec![
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

type OmnipoolLiquidityMiningInstance = warehouse_liquidity_mining::Instance1;
impl warehouse_liquidity_mining::Config<OmnipoolLiquidityMiningInstance> for Runtime {
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
	type RuntimeEvent = RuntimeEvent;
}

impl pallet_omnipool_liquidity_mining::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type CreateOrigin = AllTechnicalCommitteeMembers;
	type PalletId = OmniLMPalletId;
	type NFTCollectionId = OmnipoolLMCollectionId;
	type NFTHandler = Uniques;
	type LiquidityMiningHandler = OmnipoolWarehouseLM;
	type WeightInfo = ();
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

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system exclude_parts { Origin } = 1,
		Timestamp: pallet_timestamp = 3,
		//NOTE: 5 - is used by Scheduler which must be after cumulus_pallet_parachain_system
		Balances: pallet_balances = 7,
		TransactionPayment: pallet_transaction_payment exclude_parts { Config } = 9,
		Treasury: pallet_treasury = 11,
		Utility: pallet_utility = 13,
		Preimage: pallet_preimage = 15,
		Identity: pallet_identity = 17,
		Democracy: pallet_democracy exclude_parts { Config } = 19,
		Elections: pallet_elections_phragmen = 21,
		Council: pallet_collective::<Instance1> = 23,
		TechnicalCommittee: pallet_collective::<Instance2> = 25,
		Tips: pallet_tips = 27,
		Proxy: pallet_proxy = 29,
		Multisig: pallet_multisig = 31,
		Uniques: pallet_uniques = 32,

		// HydraDX related modules
		AssetRegistry: pallet_asset_registry = 51,
		Claims: pallet_claims = 53,
		GenesisHistory: pallet_genesis_history = 55,
		CollatorRewards: pallet_collator_rewards = 57,
		Omnipool: pallet_omnipool = 59,
		TransactionPause: pallet_transaction_pause = 60,
		Duster: pallet_duster = 61,
		OmnipoolWarehouseLM: warehouse_liquidity_mining::<Instance1> = 62,
		OmnipoolLiquidityMining: pallet_omnipool_liquidity_mining = 63,
		OTC: pallet_otc = 64,
		CircuitBreaker: pallet_circuit_breaker = 65,

		// ORML related modules
		Tokens: orml_tokens = 77,
		Currencies: pallet_currencies = 79,
		Vesting: orml_vesting = 81,

		// Parachain
		ParachainSystem: cumulus_pallet_parachain_system exclude_parts { Config } = 103,
		ParachainInfo: parachain_info = 105,

		//NOTE: Scheduler must be after ParachainSystem otherwise RelayChainBlockNumberProvider
		//will return 0 as current block number when used with Scheduler(democracy).
		Scheduler: pallet_scheduler = 5,

		PolkadotXcm: pallet_xcm = 107,
		CumulusXcm: cumulus_pallet_xcm = 109,
		XcmpQueue: cumulus_pallet_xcmp_queue exclude_parts { Call } = 111,
		DmpQueue: cumulus_pallet_dmp_queue = 113,

		// ORML XCM
		OrmlXcm: orml_xcm = 135,
		XTokens: orml_xtokens = 137,
		UnknownTokens: orml_unknown_tokens = 139,

		// Collator support
		Authorship: pallet_authorship = 161,
		CollatorSelection: pallet_collator_selection = 163,
		Session: pallet_session = 165,
		Aura: pallet_aura = 167,
		AuraExt: cumulus_pallet_aura_ext = 169,

		// Warehouse - let's allocate indices 100+ for warehouse pallets
		RelayChainInfo: pallet_relaychain_info = 201,
		EmaOracle: pallet_ema_oracle = 202,
		MultiTransactionPayment: pallet_transaction_multi_payment = 203,
	}
);

/// The address format for describing accounts.
pub type Address = AccountId;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	pallet_transaction_multi_payment::CurrencyBalanceCheck<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsReversedWithSystemFirst,
	(
		pallet_preimage::migration::v1::Migration<Runtime>,
		pallet_democracy::migrations::v1::Migration<Runtime>,
		pallet_multisig::migrations::v1::MigrateToV1<Runtime>,
		DmpQueue,
		XcmpQueue,
		ParachainSystem,
		migrations::OnRuntimeUpgradeMigration,
		migrations::MigrateRegistryLocationToV3<Runtime>,
	),
>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}

		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			//log::info!("try-runtime::on_runtime_upgrade.");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, BlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}


	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}

		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}

		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use orml_benchmarking::list_benchmark as orml_list_benchmark;
			use frame_system_benchmarking::Pallet as SystemBench;

			let mut list = Vec::<BenchmarkList>::new();

			list_benchmark!(list, extra, frame_system, SystemBench::<Runtime>);
			list_benchmark!(list, extra, pallet_balances, Balances);
			list_benchmark!(list, extra, pallet_collator_selection, CollatorSelection);
			list_benchmark!(list, extra, pallet_timestamp, Timestamp);
			list_benchmark!(list, extra, pallet_treasury, Treasury);
			list_benchmark!(list, extra, pallet_preimage, Preimage);
			list_benchmark!(list, extra, pallet_scheduler, Scheduler);
			list_benchmark!(list, extra, pallet_identity, Identity);
			list_benchmark!(list, extra, pallet_tips, Tips);
			list_benchmark!(list, extra, pallet_proxy, Proxy);
			list_benchmark!(list, extra, pallet_utility, Utility);
			list_benchmark!(list, extra, pallet_democracy, Democracy);
			list_benchmark!(list, extra, council, Council);
			list_benchmark!(list, extra, tech, TechnicalCommittee);
			list_benchmark!(list, extra, pallet_omnipool, Omnipool);
			list_benchmark!(list, extra, pallet_omnipool_liquidity_mining, OmnipoolLiquidityMining);
			list_benchmark!(list, extra, pallet_circuit_breaker, CircuitBreaker);

			list_benchmark!(list, extra, pallet_asset_registry, AssetRegistry);
			list_benchmark!(list, extra, pallet_claims, Claims);
			list_benchmark!(list, extra, pallet_ema_oracle, EmaOracle);

			list_benchmark!(list, extra, cumulus_pallet_xcmp_queue, XcmpQueue);
			list_benchmark!(list, extra, pallet_transaction_pause, TransactionPause);

			list_benchmark!(list, extra, pallet_otc, OTC);

			orml_list_benchmark!(list, extra, pallet_currencies, benchmarking::currencies);
			orml_list_benchmark!(list, extra, orml_tokens, benchmarking::tokens);
			orml_list_benchmark!(list, extra, orml_vesting, benchmarking::vesting);
			orml_list_benchmark!(list, extra, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_list_benchmark!(list, extra, pallet_duster, benchmarking::duster);

			let storage_info = AllPalletsWithSystem::storage_info();

			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};
			use orml_benchmarking::add_benchmark as orml_add_benchmark;
			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			// Substrate pallets
			add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
			add_benchmark!(params, batches, pallet_balances, Balances);
			add_benchmark!(params, batches, pallet_collator_selection, CollatorSelection);
			add_benchmark!(params, batches, pallet_timestamp, Timestamp);
			add_benchmark!(params, batches, pallet_treasury, Treasury);
			add_benchmark!(params, batches, pallet_preimage, Preimage);
			add_benchmark!(params, batches, pallet_scheduler, Scheduler);
			add_benchmark!(params, batches, pallet_identity, Identity);
			add_benchmark!(params, batches, pallet_tips, Tips);
			add_benchmark!(params, batches, pallet_proxy, Proxy);
			add_benchmark!(params, batches, pallet_utility, Utility);
			add_benchmark!(params, batches, pallet_democracy, Democracy);
			add_benchmark!(params, batches, council, Council);
			add_benchmark!(params, batches, tech, TechnicalCommittee);
			add_benchmark!(params, batches, pallet_omnipool, Omnipool);
			add_benchmark!(params, batches, pallet_omnipool_liquidity_mining, OmnipoolLiquidityMining);
			add_benchmark!(params, batches, pallet_circuit_breaker, CircuitBreaker);

			add_benchmark!(params, batches, pallet_asset_registry, AssetRegistry);
			add_benchmark!(params, batches, pallet_claims, Claims);
			add_benchmark!(params, batches, pallet_ema_oracle, EmaOracle);

			add_benchmark!(params, batches, cumulus_pallet_xcmp_queue, XcmpQueue);
			add_benchmark!(params, batches, pallet_transaction_pause, TransactionPause);

			add_benchmark!(params, batches, pallet_otc, OTC);

			orml_add_benchmark!(params, batches, pallet_currencies, benchmarking::currencies);
			orml_add_benchmark!(params, batches, orml_tokens, benchmarking::tokens);
			orml_add_benchmark!(params, batches, orml_vesting, benchmarking::vesting);
			orml_add_benchmark!(params, batches, pallet_transaction_multi_payment, benchmarking::multi_payment);
			orml_add_benchmark!(params, batches, pallet_duster, benchmarking::duster);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}
}

struct CheckInherents;

impl cumulus_pallet_parachain_system::CheckInherents<Block> for CheckInherents {
	fn check_inherents(
		block: &Block,
		relay_state_proof: &cumulus_pallet_parachain_system::RelayChainStateProof,
	) -> sp_inherents::CheckInherentsResult {
		let relay_chain_slot = relay_state_proof
			.read_slot()
			.expect("Could not read the relay chain slot from the proof");

		let inherent_data = cumulus_primitives_timestamp::InherentDataProvider::from_relay_chain_slot_and_duration(
			relay_chain_slot,
			sp_std::time::Duration::from_secs(6),
		)
		.create_inherent_data()
		.expect("Could not create the timestamp inherent data");

		inherent_data.check_extrinsics(block)
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
	CheckInherents = CheckInherents,
}
