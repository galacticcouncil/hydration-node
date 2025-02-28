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

use super::*;
use primitives::constants::{
	currency::DOLLARS,
	time::{DAYS, HOURS},
};

use frame_support::{
	parameter_types,
	traits::{ConstU32, EitherOfDiverse},
};
use frame_system::{EnsureRoot, EnsureSigned};

#[cfg(test)]
mod tests {
	use super::{EnactmentPeriod, VoteLockingPeriod};

	#[test]
	fn democracy_periods() {
		// Make sure VoteLockingPeriod > EnactmentPeriod
		assert!(VoteLockingPeriod::get() > EnactmentPeriod::get());
	}
}

pub type MoreThanHalfTechCommittee = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 2>,
	EnsureRoot<AccountId>,
>;

pub type AllTechnicalCommitteeMembers = EitherOfDiverse<
	pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
	EnsureRoot<AccountId>,
>;

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

impl pallet_democracy::Config for Runtime {
	type WeightInfo = weights::pallet_democracy::HydraWeight<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Scheduler = Scheduler;
	type Preimages = Preimage;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MinimumDeposit = MinimumDeposit;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type CooloffPeriod = CooloffPeriod;
	type MaxVotes = MaxVotes;
	type MaxProposals = MaxProposals;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = EnsureRoot<AccountId>;
	type ExternalMajorityOrigin = EnsureRoot<AccountId>;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = EnsureRoot<AccountId>;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type FastTrackOrigin = MoreThanHalfTechCommittee;
	type InstantOrigin = AllTechnicalCommitteeMembers;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EnsureRoot<AccountId>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = AllTechnicalCommitteeMembers;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type PalletsOrigin = OriginCaller;
	type Slash = Treasury;
	type DemocracyHooks = pallet_staking::integrations::democracy_legacy::LegacyStakingDemocracy<Runtime>;
	// Any single technical committee member may remove a vote.
	type VoteRemovalOrigin = frame_system::EnsureSignedBy<TechCommAccounts, AccountId>;
}
