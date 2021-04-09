#![cfg_attr(not(feature = "std"), no_std)]

pub mod constants;
pub use constants::{currency::*, time::*};
pub use frame_support::{
	parameter_types,
	traits::LockIdentifier,
	weights::{
		constants::{BlockExecutionWeight, WEIGHT_PER_SECOND},
		DispatchClass, Pays, Weight,
	},
};
pub use frame_system::limits;
pub use primitives::{fee, Amount, AssetId, Balance, Moment, CORE_ASSET_ID};
pub use sp_runtime::{
	generic,
	traits::{IdentifyAccount, Verify},
	transaction_validity::TransactionPriority,
	ModuleId, MultiSignature, Perbill, Percent, Permill,
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
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

pub const HYDRADX_AUTHORING_VERSION: u32 = 1;
pub const HYDRADX_SPEC_VERSION: u32 = 6;
pub const HYDRADX_IMPL_VERSION: u32 = 0;
pub const HYDRADX_TRANSACTION_VERSION: u32 = 1;

/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
/// by  Operational  extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
/// We allow for 2 seconds of compute with a 6 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;

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
}

// pallet transaction payment
parameter_types! {
	pub const TransactionByteFee: Balance = 1;
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

// pallet election provider multi phase
parameter_types! {
	// phase durations. 1/4 of the last session for each.
	pub const SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
	pub const UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

	pub SolutionImprovementThreshold: Perbill = Perbill::from_rational_approximation(1u32, 10_000);

	// miner configs
	pub const MultiPhaseUnsignedPriority: TransactionPriority = StakingUnsignedPriority::get() - 1u64;
	pub const MinerMaxIterations: u32 = 10;
}

// pallet treasury
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = DOLLARS;
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(50);
	pub const DataDepositPerByte: Balance = CENTS;
	pub const TipCountdown: BlockNumber = DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = DOLLARS;
	pub const TipReportDepositPerByte: Balance = CENTS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
}

// pallet session
parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

// pallet elections phragmen
parameter_types! {
	pub const CandidacyBond: Balance = 10 * DOLLARS;
	pub const VotingBond: Balance = DOLLARS;
	pub const TermDuration: BlockNumber = 7 * DAYS;
	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 7;
	pub const ElectionsPhragmenModuleId: LockIdentifier = *b"phrelect";
}

// pallet babe
parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}

// pallet collective Instance1
parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const CouncilMaxProposals: u32 = 100;
	pub const ProposalVotesRequired: u32 = 3;
	pub const ProposalMininumDeposit: Balance = 0;
}

parameter_types! {
	pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_SLOTS as _;
	pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
	/// We prioritize im-online heartbeats over election solution submission.
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
}
