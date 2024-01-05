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

use super::*;
use frame_support::{traits::Get, weights::Weight};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::{IdentifyAccount, Verify};

const PARACHAIN_CODES: [(&str, &str); 1] = [("BALLS", "Account")];

pub fn preregister_parachain_codes<T: Config>() -> Weight
where
	<T as frame_system::Config>::AccountId: From<AccountId32>,
{
	let mut weight: Weight = Weight::zero();
	for (code, account_id) in PARACHAIN_CODES.into_iter() {
		let code: ReferralCode<T::CodeLength> = ReferralCode::<T::CodeLength>::truncate_from(code.as_bytes().to_vec());
		let maybe_who: Option<AccountId32> =
			<<sp_runtime::MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId::try_from(
				account_id.as_bytes(),
			)
			.ok();

		if let Some(who) = maybe_who {
			let who: T::AccountId = who.into();
			if !ReferralCodes::<T>::contains_key(code.clone()) {
				ReferralCodes::<T>::insert(&code, &who);
				ReferralAccounts::<T>::insert(&who, code);
				Referrer::<T>::insert(&who, (Level::default(), Balance::zero()));
				weight = weight.saturating_add(T::DbWeight::get().writes(3));
			}
		}
	}
	weight
}
