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

pub mod remove_fee_fallbacks;

// New migrations which need to be cleaned up after every Runtime upgrade
pub type UnreleasedSingleBlockMigrations = (
	pallet_stableswap::migrations::v2::MigrateV1ToV2<Runtime>,
	remove_fee_fallbacks::RetireInsufficientEdPool,
);

// These migrations can run on every runtime upgrade
pub type PermanentSingleBlockMigrations = pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>;

pub type SingleBlockMigrationsList = (PermanentSingleBlockMigrations, UnreleasedSingleBlockMigrations);

// Multi-block migrations executed by pallet-migrations.
// One-shot entries stay listed until the next release: `pallet-migrations` records completed ids,
// so a finished migration is skipped rather than re-run.
#[cfg(not(feature = "runtime-benchmarks"))]
pub type MultiBlockMigrationsList = (remove_fee_fallbacks::PurgeUnsupportedFeeCurrencies,);
