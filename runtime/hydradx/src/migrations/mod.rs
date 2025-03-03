// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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
mod scheduler;

use super::*;

impl cumulus_pallet_xcmp_queue::migration::v5::V5Config for Runtime {
	type ChannelList = ParachainSystem;
}

pub type Migrations = (
	cumulus_pallet_xcmp_queue::migration::v5::MigrateV4ToV5<Runtime>,
	evm::precompiles::erc20_mapping::SetCodeMetadataForErc20Precompile,
	// Async backing migrations
	pallet_dca::migrations::MultiplySchedulesPeriodBy2<Runtime>,
	pallet_staking::migrations::SetSixSecBlocksSince<Runtime>,
	scheduler::MigrateSchedulerTo6sBlocks<Runtime>,
	conviction_voting::MigrateConvictionVotingTo6sBlocks<Runtime>,
);
