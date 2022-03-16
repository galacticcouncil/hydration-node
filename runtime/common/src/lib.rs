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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use frame_system::EnsureRoot;
use frame_support::{parameter_types, weights::Pays, PalletId, RuntimeDebug};
use frame_support::traits::{Contains, EnsureOneOf, LockIdentifier};
pub use pallet_transaction_payment::Multiplier;
pub use primitives::constants::{chain::*, currency::*, time::*};
pub use primitives::{Amount, AssetId, Balance, BlockNumber};
use sp_runtime::{
	generic,
	traits::{AccountIdConversion, BlakeTwo256, IdentifyAccount, Verify},
	FixedPointNumber, MultiSignature, Perbill, Permill, Perquintill, Percent,
};
use sp_core::{
	u32_trait::{_1, _2, _3},
};
use sp_std::prelude::*;
use codec::alloc::vec;

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

/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

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

pub type MoreThanHalfCouncil = EnsureOneOf<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
>;

pub type MajorityOfCouncil = EnsureOneOf<
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>,
	EnsureRoot<AccountId>,
>;

pub type AllCouncilMembers = EnsureOneOf<
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

pub type MoreThanHalfTechCommittee = EnsureOneOf<
	pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, TechnicalCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

pub type AllTechnicalCommitteeMembers = EnsureOneOf<
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![TreasuryPalletId::get().into_account()]
}

pub struct DustRemovalWhitelist;

impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a)
	}
}

// frame system
parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
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
	pub const MaxAuthorities: u32 = 32;
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
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 10 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 50 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 3 * DAYS;
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
	pub const MaxCandidates: u32 = 20;
	pub const MinCandidates: u32 = 4;
	pub const MaxInvulnerables: u32 = 10;
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
	pub const EnactmentPeriod: BlockNumber = 6 * DAYS;
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
	pub MinVestedTransfer: Balance = NativeExistentialDeposit::get();
	pub const MaxVestingSchedules: u32 = 100;
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
	pub ProxyDepositBase: Balance = 4 * DOLLARS + 480 * MILLICENTS;
	pub ProxyDepositFactor: Balance = 1_980 * MILLICENTS;
	pub const MaxProxies: u16 = 32;
	pub AnnouncementDepositBase: Balance = 4 * DOLLARS + 480 * MILLICENTS;
	pub AnnouncementDepositFactor: Balance = 3_960 * MILLICENTS;
	pub const MaxPending: u16 = 32;
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
}

