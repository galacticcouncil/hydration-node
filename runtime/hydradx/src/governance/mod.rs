// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

// Gov V1 (to be deprecated)
pub mod old;

pub mod origins;
mod tracks;

use super::*;
use crate::governance::{
	origins::{ReferendumCanceller, ReferendumKiller, Spender, WhitelistedCaller},
	tracks::TracksInfo,
};
use crate::old::{MoreThanHalfCouncil, TreasuryApproveOrigin};
use frame_support::{
	parameter_types,
	sp_runtime::Permill,
	traits::{EitherOf, NeverEnsureOrigin},
	PalletId,
};
use primitives::constants::{currency::DOLLARS, time::DAYS};
use sp_arithmetic::Perbill;
use sp_core::ConstU32;

use frame_system::{EnsureRoot, EnsureRootWithSuccess};

parameter_types! {
	pub const TechnicalMaxProposals: u32 = 20;
	pub const TechnicalMaxMembers: u32 = 10;
	pub const TechnicalMotionDuration: BlockNumber = 5 * DAYS;
  pub MaxProposalWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
}

pub type TechnicalCollective = pallet_collective::Instance2;
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::technical_committee::HydraWeight<Runtime>;
	type MaxProposalWeight = MaxProposalWeight;
	type SetMembersOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub TreasuryAccount: AccountId = Treasury::account_id();
	pub const ProposalBond: Permill = Permill::from_percent(3);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaxApprovals: u32 =  100;
}

impl pallet_treasury::Config for Runtime {
	type Currency = Balances;
	// TODO origin
	type ApproveOrigin = TreasuryApproveOrigin;
	// TODO origin
	type RejectOrigin = MoreThanHalfCouncil;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	type BurnDestination = ();
	type WeightInfo = weights::treasury::HydraWeight<Runtime>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	// TODO origin
	type SpendOrigin = NeverEnsureOrigin<Balance>;
}

parameter_types! {
	pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_conviction_voting::Config for Runtime {
	type WeightInfo = weights::conviction_voting::WeightInfo<Self>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MaxVotes = ConstU32<512>;
	type MaxTurnout = frame_support::traits::tokens::currency::ActiveIssuanceOf<Balances, Self::AccountId>;
	type Polls = Referenda;
}

parameter_types! {
	pub const MaxBalance: Balance = Balance::max_value();
}
pub type TreasurySpender = EitherOf<EnsureRootWithSuccess<AccountId, MaxBalance>, Spender>;

impl pallet_whitelist::Config for Runtime {
	type WeightInfo = weights::whitelist::WeightInfo<Self>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	// TODO WhitelistOrigin
	type WhitelistOrigin = EnsureRoot<Self::AccountId>;
	type DispatchWhitelistedOrigin = EitherOf<EnsureRoot<Self::AccountId>, WhitelistedCaller>;
	type Preimages = Preimage;
}

parameter_types! {
	pub const AlarmInterval: BlockNumber = 1;
	pub const SubmissionDeposit: Balance = 1 * DOLLARS;
	pub const UndecidingTimeout: BlockNumber = 14 * DAYS;
}

impl pallet_referenda::Config for Runtime {
	type WeightInfo = weights::referenda::WeightInfo<Self>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type Scheduler = Scheduler;
	type Currency = Balances;
	type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
	type CancelOrigin = EitherOf<EnsureRoot<AccountId>, ReferendumCanceller>;
	type KillOrigin = EitherOf<EnsureRoot<AccountId>, ReferendumKiller>;
	type Slash = Treasury;
	type Votes = pallet_conviction_voting::VotesOf<Runtime>;
	type Tally = pallet_conviction_voting::TallyOf<Runtime>;
	type SubmissionDeposit = SubmissionDeposit;
	type MaxQueued = ConstU32<100>;
	type UndecidingTimeout = UndecidingTimeout;
	type AlarmInterval = AlarmInterval;
	type Tracks = TracksInfo;
	type Preimages = Preimage;
}

impl origins::pallet_custom_origins::Config for Runtime {}
