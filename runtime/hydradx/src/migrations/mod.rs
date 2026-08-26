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

use crate::Runtime;

pub mod circuit_breaker;
pub mod conviction_voting;
pub mod dynamic_fees;
pub mod parameters;
pub mod referenda;
pub mod scheduler;
pub mod stableswap;

// New migrations which need to be cleaned up after every Runtime upgrade
pub type UnreleasedSingleBlockMigrations = (
	parameters::SetTwoSecBlocksSince<Runtime>,
	pallet_dca::migrations::MultiplySchedulesPeriodBy3<Runtime>,
	circuit_breaker::MigrateCircuitBreakerTo2sBlocks<Runtime>,
	dynamic_fees::MigrateDynamicFeeDecayTo2sBlocks<Runtime>,
	stableswap::MigrateStableswapMaxPegUpdateTo2sBlocks<Runtime>,
	pallet_stableswap::migrations::v2::MigrateV1ToV2<Runtime>,
	scheduler::MigrateSchedulerTo2sBlocks<Runtime>,
	referenda::MigrateReferendaTo2sBlocks<Runtime>,
	conviction_voting::MigrateConvictionVotingTo2sBlocks<Runtime>,
);

// These migrations can run on every runtime upgrade
pub type PermanentSingleBlockMigrations = pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>;

pub type SingleBlockMigrationsList = (PermanentSingleBlockMigrations, UnreleasedSingleBlockMigrations);

// Multi-block migrations executed by pallet-migrations
#[cfg(not(feature = "runtime-benchmarks"))]
pub type MultiBlockMigrationsList = ();
