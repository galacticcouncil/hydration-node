// This file is part of HydraDX.

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

pub use frame_support::{
	parameter_types,
	traits::LockIdentifier,
	weights::{DispatchClass, Pays},
};
use frame_system::{limits, EnsureOneOf, EnsureRoot};
pub mod constants;
use codec::alloc::vec;
pub use constants::{chain::*, currency::*, time::*};
pub use frame_support::PalletId;
use pallet_transaction_payment::Multiplier;
pub use primitives::{fee, Amount, AssetId, Balance};
use sp_core::{
	u32_trait::{_1, _2, _3, _5},
	H256,
};
use sp_runtime::{
	generic,
	traits::{BlakeTwo256, IdentifyAccount, Verify},
	MultiSignature,
};
pub use sp_runtime::{
	transaction_validity::TransactionPriority, FixedPointNumber, Perbill, Percent, Permill, Perquintill,
};

/// An index to a block.
pub type BlockNumber = u32;

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
pub type Hash = H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

/// Opaque, encoded, unchecked extrinsic.
pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

/// Header type.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// Type used for expressing timestamp.
pub type Moment = u64;

pub type CouncilCollective = pallet_collective::Instance1;
pub type TechnicalCollective = pallet_collective::Instance2;

pub type MoreThanHalfCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
>;

pub type TreasuryApproveOrigin = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<_3, _5, AccountId, CouncilCollective>,
>;

pub type MajorityOfCouncil = EnsureOneOf<
	AccountId,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>,
	EnsureRoot<AccountId>,
>;

pub type AllCouncilMembers = EnsureOneOf<
	AccountId,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, CouncilCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

pub type MajorityOfTechnicalCommittee = EnsureOneOf<
	AccountId,
	pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, TechnicalCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

pub type AllTechnicalCommitteeMembers = EnsureOneOf<
	AccountId,
	pallet_collective::EnsureProportionAtLeast<_1, _1, AccountId, TechnicalCollective>,
	frame_system::EnsureRoot<AccountId>,
>;

// During the testnet slashes can be canceled by majority of council or technical committee
pub type SlashCancelOrigin =
	EnsureOneOf<AccountId, MajorityOfTechnicalCommittee, MajorityOfCouncil>;

// frame system
parameter_types! {
	pub const BlockHashCount: BlockNumber = 2400;
	/// Maximum length of block. Up to 5MB.
	pub BlockLength: limits::BlockLength =
		limits::BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 63;
}

// pallet timestamp
parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
	pub const HDXAssetId: AssetId = CORE_ASSET_ID;
}

// pallet balances
parameter_types! {
	pub const ExistentialDeposit: u128 = 0;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

// pallet transaction payment
parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	/// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

// pallet transaction multi payment
parameter_types! {
	pub const MultiPaymentCurrencySetFee: Pays = Pays::No;
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

// pallet amm
parameter_types! {
	pub ExchangeFee: fee::Fee = fee::Fee::default();
}

// pallet claims
parameter_types! {
	pub ClaimMessagePrefix: &'static [u8] = b"I hereby claim all my HDX tokens to wallet:";
}

// pallet authorship
parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

sp_npos_elections::generate_solution_type!(
	#[compact]
	pub struct NposCompactSolution16::<
		VoterIndex = u32,
		TargetIndex = u16,
		Accuracy = sp_runtime::PerU16,
	>(16)
);

pub const MAX_NOMINATIONS: u32 = <NposCompactSolution16 as sp_npos_elections::CompactSolution>::LIMIT as u32;

// pallet staking
parameter_types! {
	pub const MaxNominatorRewardedPerValidator: u32 = 64;
	pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
	pub const MaxIterations: u32 = 10;
	// 0.05%. The higher the value, the more strict solution acceptance becomes.
	pub MinSolutionScoreBump: Perbill = Perbill::from_rational(5u32, 10_000);
}

// pallet democracy
parameter_types! {
	pub const PreimageByteDeposit: Balance = CENTS;
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
}

// pallet election provider multi phase
parameter_types! {
	// phase durations. 1/4 of the last session for each.
	pub const SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
	pub const UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

	pub SolutionImprovementThreshold: Perbill = Perbill::from_rational(1u32, 10_000);

	// miner configs
	pub const MultiPhaseUnsignedPriority: TransactionPriority = StakingUnsignedPriority::get() - 1u64;
	pub const MinerMaxIterations: u32 = 10;
}

// pallet treasury
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(3);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
}

// pallet tips
parameter_types! {
	pub const DataDepositPerByte: Balance = CENTS;
	pub const TipCountdown: BlockNumber = 24 * HOURS;
	pub const TipFindersFee: Percent = Percent::from_percent(1);
	pub const TipReportDepositBase: Balance = 10 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = CENTS;
	pub const MaximumReasonLength: u32 = 1024;
}

// pallet session
parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

// pallet elections phragmen
parameter_types! {
	pub const CandidacyBond: Balance = 5 * DOLLARS;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = CENTS;
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = CENTS;
	pub const ElectionsPhragmenPalletId: LockIdentifier = *b"phrelect";
}

// pallet babe
parameter_types! {
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}

// pallet collective Instance1 - CouncilCollective
parameter_types! {
	pub const CouncilMaxProposals: u32 = 30;
	pub const CouncilMaxMembers: u32 = 13;
}

// pallet collective Instance2 - TechnicalCollective
parameter_types! {
	pub const TechnicalMaxProposals: u32 = 20;
	pub const TechnicalMaxMembers: u32 = 10;
}

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
	pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	/// We prioritize im-online heartbeats over election solution submission.
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
}
