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

pub mod adapters;
pub mod weights;

use codec::alloc::vec;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::traits::{Contains, EitherOfDiverse, LockIdentifier};
use frame_support::{parameter_types, weights::Pays, PalletId, RuntimeDebug};
use frame_system::EnsureRoot;
use hydradx_traits::oracle::{OraclePeriod, Source};
pub use pallet_transaction_payment::Multiplier;
pub use primitives::constants::{chain::*, currency::*, time::*};
pub use primitives::{Amount, AssetId, Balance, BlockNumber, CollectionId};
use scale_info::TypeInfo;
use sp_runtime::{
	generic,
	traits::{AccountIdConversion, BlakeTwo256, IdentifyAccount, Verify},
	DispatchError, FixedPointNumber, MultiSignature, Perbill, Percent, Permill, Perquintill,
};
use sp_std::prelude::*;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

use pallet_dca::types::NamedReserveIdentifier;
/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

use self::adapters::OMNIPOOL_SOURCE;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

pub type CouncilCollective = pallet_collective::Instance1;
pub type TechnicalCollective = pallet_collective::Instance2;

pub type TreasuryApproveOrigin = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
>;

pub type MoreThanHalfCouncil = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;

pub type MajorityOfCouncil = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
	EnsureRoot<AccountId>,
>;

pub type AllCouncilMembers = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
	EnsureRoot<AccountId>,
>;

pub type MoreThanHalfTechCommittee = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 2>,
	EnsureRoot<AccountId>,
>;

pub type SuperMajorityTechCommittee = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
	EnsureRoot<AccountId>,
>;

pub type AllTechnicalCommitteeMembers = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
	EnsureRoot<AccountId>,
>;

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![
		TreasuryPalletId::get().into_account_truncating(),
		VestingPalletId::get().into_account_truncating(),
	]
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a)
	}
}

pub struct CircuitBreakerWhitelist;

impl Contains<AccountId> for CircuitBreakerWhitelist {
	fn contains(a: &AccountId) -> bool {
		<PalletId as AccountIdConversion<AccountId>>::into_account_truncating(&TreasuryPalletId::get()) == *a
	}
}

// frame system
parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	/// Maximum length of block. Up to 5MB.
	pub BlockLength: frame_system::limits::BlockLength =
		frame_system::limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u16 = 63;
}

// pallet timestamp
parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
	pub const NativeAssetId : AssetId = CORE_ASSET_ID;
}

// pallet balances
parameter_types! {
	pub const NativeExistentialDeposit: u128 = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

// pallet aura
parameter_types! {
	pub const MaxAuthorities: u32 = 50;
}

// pallet transaction payment
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
}

// pallet treasury
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(3);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 =  100;
}

// pallet authorship
parameter_types! {
	pub const UncleGenerations: u32 = 0;
}

// pallet collator selection
parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 0;
	pub const MinCandidates: u32 = 0;
	pub const MaxInvulnerables: u32 = 50;
}

// pallet session
parameter_types! {
	pub const Period: u32 = 4 * HOURS;
	pub const Offset: u32 = 0;
}

// pallet preimage
parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub PreimageBaseDeposit: Balance = deposit(2, 64);
	pub PreimageByteDeposit: Balance = deposit(0, 1);
}

// pallet identity
parameter_types! {
	pub const BasicDeposit: Balance = 5 * DOLLARS;
	pub const FieldDeposit: Balance = DOLLARS;
	pub const SubAccountDeposit: Balance = 5 * DOLLARS;
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

// pallet collective Instance1 - CouncilCollective
parameter_types! {
	pub const CouncilMaxProposals: u32 = 30;
	pub const CouncilMaxMembers: u32 = 13;
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
}

// pallet collective Instance2 - TechnicalCollective
parameter_types! {
	pub const TechnicalMaxProposals: u32 = 20;
	pub const TechnicalMaxMembers: u32 = 10;
	pub const TechnicalMotionDuration: BlockNumber = 5 * DAYS;
}

// pallet democracy
parameter_types! {
	pub const LaunchPeriod: BlockNumber = 3 * DAYS;
	pub const VotingPeriod: BlockNumber = 3 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
	pub const MinimumDeposit: Balance = 1000 * DOLLARS;
	pub const EnactmentPeriod: BlockNumber = 24 * HOURS;
	// Make sure VoteLockingPeriod > EnactmentPeriod
	pub const VoteLockingPeriod: BlockNumber = 6 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

// pallet elections_phragmen
parameter_types! {
	// Bond for candidacy into governance
	pub const CandidacyBond: Balance = 5 * DOLLARS;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = CENTS;
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = CENTS;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 15;
	pub const ElectionsPhragmenPalletId: LockIdentifier = *b"phrelect";
	pub const MaxElectionCandidates: u32 = 1_000;
	pub const MaxElectionVoters: u32 = 10_000;
}

// pallet tips
parameter_types! {
	pub const DataDepositPerByte: Balance = CENTS;
	pub const TipCountdown: BlockNumber = 2 * HOURS;
	pub const TipFindersFee: Percent = Percent::from_percent(1);
	pub const TipReportDepositBase: Balance = 10 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = CENTS;
	pub const MaximumReasonLength: u32 = 1024;
}

// pallet vesting
parameter_types! {
	pub MinVestedTransfer: Balance = 100;
	pub const MaxVestingSchedules: u32 = 100;
	pub const VestingPalletId: PalletId = PalletId(*b"py/vstng");
}

// pallet proxy
/// The type used to represent the kinds of proxying allowed.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum ProxyType {
	Any,
	CancelProxy,
	Governance,
	Transfer,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
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

// pallet multisig
parameter_types! {
	pub DepositBase: Balance = deposit(1, 88);
	pub DepositFactor: Balance = deposit(0, 32);
	pub const MaxSignatories: u16 = 100;
}

// pallet claims
parameter_types! {
	pub ClaimMessagePrefix: &'static [u8] = b"I hereby claim all my HDX tokens to wallet:";
}

// pallet transaction multi payment
parameter_types! {
	pub const MultiPaymentCurrencySetFee: Pays = Pays::Yes;
}

// pallet asset registry
parameter_types! {
	pub const RegistryStrLimit: u32 = 32;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

// pallet circuit breaker
parameter_types! {
	pub const DefaultMaxNetTradeVolumeLimitPerBlock: (u32, u32) = (5_000, 10_000);	// 50%
	pub const DefaultMaxLiquidityLimitPerBlock: Option<(u32, u32)> = Some((500, 10_000));	// 5%
}

// pallet duster
parameter_types! {
	pub const DustingReward: u128 = 0;
}

// omnipool's warehouse pallet liquidity mining
parameter_types! {
	pub const OmniWarehouseLMPalletId: PalletId = PalletId(*b"OmniWhLM");
	#[derive(PartialEq, Eq)]
	pub const MaxEntriesPerDeposit: u8 = 5; //NOTE: Rebenchmark when this change, TODO:
	pub const MaxYieldFarmsPerGlobalFarm: u8 = 50; //NOTE: Includes deleted/destroyed farms, TODO:
	pub const MinPlannedYieldingPeriods: BlockNumber = 14_440;  //1d with 6s blocks, TODO:
	pub const MinTotalFarmRewards: Balance = NATIVE_EXISTENTIAL_DEPOSIT * 100; //TODO:
}

// omnipool's liquidity mining
parameter_types! {
	pub const OmniLMPalletId: PalletId = PalletId(*b"Omni//LM");
	pub const OmnipoolLMCollectionId: CollectionId = 2584_u128;
	pub const OmnipoolLMOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
	pub const OmnipoolLMOracleSource: Source = OMNIPOOL_SOURCE;
}

// pallet dca
parameter_types! {
	pub StorageBondInNativeCurrency: Balance = 100 * UNITS;
	pub MaxSchedulesPerBlock: u32 = 20;
	pub SlippageLimitPercentage: Permill = Permill::from_percent(5);
	pub MaxPriceDifference: Permill = Permill::from_percent(10);
	pub NamedReserveId: NamedReserveIdentifier = *b"dcaorder";
}
