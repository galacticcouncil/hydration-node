#![cfg_attr(not(feature = "std"), no_std)]

pub use primitives::constants::{chain::*, currency::*, time::*};
pub use frame_support::{
	parameter_types,
	traits::LockIdentifier,
	weights::{DispatchClass, Pays},
};
pub use frame_system::limits;
pub use primitives::{fee, AccountId, AccountIndex, Amount, AssetId, Balance, BlockNumber, DigestItem,
	Hash, Index, Moment, Signature};
pub use sp_runtime::{
	transaction_validity::TransactionPriority,
	ModuleId, Perbill, Percent, Permill,
};

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
