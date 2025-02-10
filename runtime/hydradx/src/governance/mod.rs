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
pub mod tracks;

use super::*;
use crate::governance::{
	origins::{
		EconomicParameters, GeneralAdmin, ReferendumCanceller, ReferendumKiller, Spender, Treasurer, WhitelistedCaller,
	},
	tracks::TracksInfo,
};
use frame_support::{
	parameter_types,
	sp_runtime::Permill,
	traits::{tokens::UnityAssetBalanceConversion, EitherOf},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureRootWithSuccess};
use pallet_collective::EnsureProportionAtLeast;
use primitives::constants::{currency::DOLLARS, time::DAYS};
use sp_arithmetic::Perbill;
use sp_core::ConstU32;
use sp_runtime::traits::IdentityLookup;

pub type TechCommitteeMajority = EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 2>;
pub type TechCommitteeSuperMajority = EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>;

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
	type WeightInfo = weights::pallet_collective_technical_committee::HydraWeight<Runtime>;
	type MaxProposalWeight = MaxProposalWeight;
	type SetMembersOrigin = EitherOf<EnsureRoot<Self::AccountId>, GeneralAdmin>;
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
	pub const TreasuryPayoutPeriod: u32 = 30 * DAYS;
}

pub struct PayFromTreasuryAccount;

impl frame_support::traits::tokens::Pay for PayFromTreasuryAccount {
	type Balance = Balance;
	type Beneficiary = AccountId;
	type AssetKind = ();
	type Id = ();
	type Error = sp_runtime::DispatchError;

	#[cfg(not(feature = "runtime-benchmarks"))]
	fn pay(
		who: &Self::Beneficiary,
		_asset_kind: Self::AssetKind,
		amount: Self::Balance,
	) -> Result<Self::Id, Self::Error> {
		let _ = <Balances as frame_support::traits::fungible::Mutate<_>>::transfer(
			&TreasuryAccount::get(),
			who,
			amount,
			frame_support::traits::tokens::Preservation::Expendable,
		)?;
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn pay(
		who: &Self::Beneficiary,
		_asset_kind: Self::AssetKind,
		amount: Self::Balance,
	) -> Result<Self::Id, Self::Error> {
		// In case of benchmarks, we adjust the value by multiplying it by 1_000_000_000_000, otherwise it fails with BelowMinimum limit error, because
		// treasury benchmarks uses only 100 as the amount.
		let _ = <Balances as frame_support::traits::fungible::Mutate<_>>::transfer(
			&TreasuryAccount::get(),
			who,
			amount * 1_000_000_000_000,
			frame_support::traits::tokens::Preservation::Expendable,
		)?;
		Ok(())
	}

	fn check_payment(_id: Self::Id) -> frame_support::traits::tokens::PaymentStatus {
		frame_support::traits::tokens::PaymentStatus::Success
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_successful(_: &Self::Beneficiary, _: Self::AssetKind, amount: Self::Balance) {
		<Balances as frame_support::traits::fungible::Mutate<_>>::mint_into(
			&TreasuryAccount::get(),
			amount * 1_000_000_000_000,
		)
		.unwrap();
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_concluded(_: Self::Id) {}
}

impl pallet_treasury::Config for Runtime {
	type Currency = Balances;
	type ApproveOrigin = EitherOf<EnsureRoot<AccountId>, Treasurer>;
	type RejectOrigin = EitherOf<EnsureRoot<AccountId>, Treasurer>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type PalletId = TreasuryPalletId;
	type BurnDestination = ();
	type WeightInfo = weights::pallet_treasury::HydraWeight<Runtime>;
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type SpendOrigin = TreasurySpender;
	#[cfg(feature = "runtime-benchmarks")]
	type SpendOrigin =
		frame_system::EnsureWithSuccess<EnsureRoot<AccountId>, AccountId, crate::benches::BenchmarkMaxBalance>;
	type AssetKind = (); // set to () to support only the native currency
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<AccountId>;
	type Paymaster = PayFromTreasuryAccount;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = TreasuryPayoutPeriod;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = (); // default impl is enough because we support only the native currency
}

parameter_types! {
	pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_conviction_voting::Config for Runtime {
	type WeightInfo = weights::pallet_conviction_voting::HydraWeight<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MaxVotes = ConstU32<25>;
	type MaxTurnout = frame_support::traits::tokens::currency::ActiveIssuanceOf<Balances, Self::AccountId>;
	type Polls = Referenda;
	type VotingHooks = pallet_staking::integrations::conviction_voting::StakingConvictionVoting<Runtime>;
	// Any single technical committee member may remove a vote.
	type VoteRemovalOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
}

parameter_types! {
	pub const MaxBalance: Balance = Balance::max_value();
}
pub type TreasurySpender = EitherOf<EnsureRootWithSuccess<AccountId, MaxBalance>, Spender>;

impl pallet_whitelist::Config for Runtime {
	type WeightInfo = weights::pallet_whitelist::HydraWeight<Runtime>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type WhitelistOrigin = EitherOf<EnsureRoot<Self::AccountId>, TechCommitteeMajority>;
	type DispatchWhitelistedOrigin = EitherOf<EnsureRoot<Self::AccountId>, WhitelistedCaller>;
	type Preimages = Preimage;
}

parameter_types! {
	pub const AlarmInterval: BlockNumber = 1;
	pub const SubmissionDeposit: Balance = DOLLARS;
	pub const UndecidingTimeout: BlockNumber = 14 * DAYS;
}

impl pallet_referenda::Config for Runtime {
	type WeightInfo = weights::pallet_referenda::HydraWeight<Runtime>;
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

parameter_types! {
	pub const AaveManagerAccount: AccountId = AccountId::new(hex!("aa7e0000000000000000000000000000000aa7e0000000000000000000000000"));
}

impl pallet_dispatcher::Config for Runtime {
	type WeightInfo = weights::pallet_dispatcher::HydraWeight<Runtime>;
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type TreasuryManagerOrigin = EitherOf<EnsureRoot<AccountId>, Treasurer>;
	type AaveManagerOrigin = EitherOf<EnsureRoot<AccountId>, EconomicParameters>;
	type TreasuryAccount = TreasuryAccount;
	type DefaultAaveManagerAccount = AaveManagerAccount;
}
