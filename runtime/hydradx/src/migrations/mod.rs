// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

mod conviction_voting;
mod democracy;
mod scheduler;

use super::*;
use frame_support::parameter_types;
use frame_support::traits::{ConstU32, LockIdentifier};
use frame_system::pallet_prelude::BlockNumberFor;

parameter_types! {
	pub const CouncilPalletName: &'static str = "Council";
	pub const PhragmenElectionPalletName: &'static str = "Elections";
	pub const TipsPalletName: &'static str = "Tips";
	pub const PhragmenElectionPalletId: LockIdentifier = *b"phrelect";
	pub const DataDepositPerByte: Balance = primitives::constants::currency::CENTS;
	pub const TipReportDepositBase: Balance = 10 * primitives::constants::currency::DOLLARS;
}

// Special Config for Gov V1 pallets, allowing us to run migrations for them without
// implementing their configs on [`Runtime`].
pub struct UnlockConfig;
impl pallet_elections_phragmen::migrations::unlock_and_unreserve_all_funds::UnlockConfig for UnlockConfig {
	type Currency = Balances;
	type MaxVotesPerVoter = ConstU32<16>;
	type PalletId = PhragmenElectionPalletId;
	type AccountId = AccountId;
	type DbWeight = <Runtime as frame_system::Config>::DbWeight;
	type PalletName = PhragmenElectionPalletName;
}
impl pallet_tips::migrations::unreserve_deposits::UnlockConfig<()> for UnlockConfig {
	type Currency = Balances;
	type Hash = Hash;
	type DataDepositPerByte = DataDepositPerByte;
	type TipReportDepositBase = TipReportDepositBase;
	type AccountId = AccountId;
	type BlockNumber = BlockNumberFor<Runtime>;
	type DbWeight = <Runtime as frame_system::Config>::DbWeight;
	type PalletName = TipsPalletName;
}

pub type Migrations = (
	// Unlock/unreserve balances from Gov v1 pallets that hold them
	// https://github.com/paritytech/polkadot/issues/6749
	pallet_elections_phragmen::migrations::unlock_and_unreserve_all_funds::UnlockAndUnreserveAllFunds<UnlockConfig>,
	pallet_tips::migrations::unreserve_deposits::UnreserveDeposits<UnlockConfig, ()>,
	// Delete storage key/values for pallets Council, PragmenElection and Tips
	frame_support::migrations::RemovePallet<CouncilPalletName, <Runtime as frame_system::Config>::DbWeight>,
	frame_support::migrations::RemovePallet<PhragmenElectionPalletName, <Runtime as frame_system::Config>::DbWeight>,
	frame_support::migrations::RemovePallet<TipsPalletName, <Runtime as frame_system::Config>::DbWeight>,
);
