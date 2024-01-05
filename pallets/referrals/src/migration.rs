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

pub const PARACHAIN_CODES: [(&str, &str); 12] = [
	("MOONBEAM", "7LCt6dFmtiRrwZv2YyEgQWW3GxsGX3Krmgzv9Xj7GQ9tG2j8"),
	("ASSETHUB", "7LCt6dFqtxzdKVB2648jWW9d85doiFfLSbZJDNAMVJNxh5rJ"),
	("INTERLAY", "7LCt6dFsW7xwUutdYad3oeQ1zfQvZ9THXbBupWLqpd72bmnM"),
	("CENTRIFUGE", "7LCt6dFsJVukxnxpix9KcTkwu2kWQnXARsy6BuBHEL54NcS6"),
	("ASTAR", "7LCt6dFnHxYDyomeCEC8nsnBUEC6omC6y7SZQk4ESzDpiDYo"),
	("BIFROST", "7LCt6dFs6sraSg31uKfbRH7soQ66GRb3LAkGZJ1ie3369crq"),
	("ZEITGEIST", "7LCt6dFCEKr7CctCKBb6CcQdV9iHDue3JcpxkkFCqJZbk3Xk"),
	("PHALA", "7LCt6dFt6z8V3Gg41U4EPCKEHZQAzEFepirNiKqXbWCwHECN"),
	("UNIQUE", "7LCt6dFtWEEr5WXfej1gmZbNUpj1Gx7u29J1yYAen6GsjQTj"),
	("NODLE", "7LCt6dFrJPdrNCKncokgeYZbQsSRgyrYwKrz2sMUGruDF9gJ"),
	("SUBSOCIAL", "7LCt6dFE2vLjshEThqtdwGAGMqg2XA39C1pMSCjG9wsKnR2Q"),
	("POLKADOT", "7KQx4f7yU3hqZHfvDVnSfe6mpgAT8Pxyr67LXHV6nsbZo3Tm"),
];

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
				weight.saturating_accrue(T::DbWeight::get().writes(3));
			}
		}
	}
	weight
}
