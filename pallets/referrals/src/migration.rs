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
use hex_literal::hex;
use sp_core::crypto::AccountId32;

pub fn preregister_parachain_codes<T: Config>() -> Weight
where
	<T as frame_system::Config>::AccountId: From<AccountId32>,
{
	let mut weight: Weight = Weight::zero();

	let accounts: [(&str, Option<AccountId32>); 12] = [
		(
			"MOONBEAM",
			Some(AccountId32::from(hex!["7369626cd4070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"ASSETHUB",
			Some(AccountId32::from(hex!["7369626ce8030000000000000000000000000000000000000000000000000000"])),
		),
		(
			"INTERLAY",
			Some(AccountId32::from(hex!["7369626cf0070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"CENTRIFUGE",
			Some(AccountId32::from(hex!["7369626cef070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"ASTAR",
			Some(AccountId32::from(hex!["7369626cd6070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"BIFROST",
			Some(AccountId32::from(hex!["7369626cee070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"ZEITGEIST",
			Some(AccountId32::from(hex!["7369626c2c080000000000000000000000000000000000000000000000000000"])),
		),
		(
			"PHALA",
			Some(AccountId32::from(hex!["7369626cf3070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"UNIQUE",
			Some(AccountId32::from(hex!["7369626cf5070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"NODLE",
			Some(AccountId32::from(hex!["7369626cea070000000000000000000000000000000000000000000000000000"])),
		),
		(
			"SUBSOCIAL",
			Some(AccountId32::from(hex!["7369626c35080000000000000000000000000000000000000000000000000000"])),
		),
		(
			"POLKADOT",
			Some(AccountId32::from(hex!["506172656e740000000000000000000000000000000000000000000000000000"])),
		),
	];
	for (code, maybe_who) in accounts.into_iter() {
		let code: ReferralCode<T::CodeLength> = ReferralCode::<T::CodeLength>::truncate_from(code.as_bytes().to_vec());
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
