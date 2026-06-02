// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0
//
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
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;

/// Seeds `pallet_referrals::RewardPerShare` for the switch from the balance-proportional
/// payout to the MasterChef-style accumulator. Without it the accumulator stays at its
/// `ValueQuery` default of `0` on a chain that already holds `TotalShares > 0` and a funded
/// pot, so every `claim_rewards` computes `0` and pre-upgrade rewards strand.
pub struct InitReferralsAccumulator;

impl OnRuntimeUpgrade for InitReferralsAccumulator {
	fn on_runtime_upgrade() -> Weight {
		// Idempotent: the accumulator only ever grows from its default, so a non-zero value
		// means seeding already happened — don't re-seed (would clobber accrued rewards).
		if !pallet_referrals::RewardPerShare::<Runtime>::get().is_zero() {
			return <Runtime as frame_system::Config>::DbWeight::get().reads(1);
		}
		pallet_referrals::migration::migrate_to_accumulator::<Runtime>()
	}
}
