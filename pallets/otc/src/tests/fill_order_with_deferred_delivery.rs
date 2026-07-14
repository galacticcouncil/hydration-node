// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::tests::mock::*;
use frame_support::{assert_ok, storage::with_storage_layer};
use orml_traits::MultiCurrency;
use pretty_assertions::assert_eq;

// A fresh maker with no genesis endowment, so we control exactly which assets keep its account alive.
const MAKER: AccountId = 99;

// The deferred path pays the maker last (it must source `asset_in` before delivering it), so a maker
// whose only balance is the reserved `asset_out` momentarily hits a zero balance mid-fill. Without a
// provider guard that reaps the account and resets its nonce; this pins that it does not.
#[test]
fn fill_order_with_deferred_delivery_should_preserve_maker_account_when_asset_out_is_sole_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Tokens::deposit(HDX, &MAKER, 10 * ONE));

		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(MAKER),
			DAI,
			HDX,
			5 * ONE,
			10 * ONE,
			false
		));

		// A reap would reset the nonce to 0, so start it non-zero to detect one.
		frame_system::Pallet::<Test>::inc_account_nonce(&MAKER);
		frame_system::Pallet::<Test>::inc_account_nonce(&MAKER);
		let nonce_before = frame_system::Pallet::<Test>::account_nonce(&MAKER);
		assert_eq!(nonce_before, 2);

		let amount_in = 5 * ONE;
		let res = with_storage_layer(|| {
			OTC::fill_order_with_deferred_delivery(0, &BOB, amount_in, |_amount_out_without_fee| {
				Tokens::deposit(DAI, &BOB, amount_in)
			})
		});
		assert_ok!(res);

		// asset_out fully spent, asset_in received, and the account survived intact (nonce preserved).
		assert_eq!(Tokens::free_balance(HDX, &MAKER), 0);
		assert_eq!(Tokens::free_balance(DAI, &MAKER), 5 * ONE);
		assert_eq!(frame_system::Pallet::<Test>::account_nonce(&MAKER), nonce_before);
		assert!(OTC::orders(0).is_none());
	});
}

// The deferred path and `partial_fill_order` must accept a partial fill on the same terms: both check
// the residual asset_out net of the fee the *remaining* order would pay. This near-full fill (rejected
// under the old fee-of-filled basis) is accepted identically by both entry points.
#[test]
fn fill_order_with_deferred_delivery_should_apply_same_remainder_dust_check_as_partial_fill_order() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(OTC::place_order(
			RuntimeOrigin::signed(ALICE),
			DAI,
			HDX,
			100 * ONE,
			20 * ONE,
			true
		));

		let amount_in = 97 * ONE;
		let res = with_storage_layer(|| {
			OTC::fill_order_with_deferred_delivery(0, &BOB, amount_in, |_| Tokens::deposit(DAI, &BOB, amount_in))
		});
		assert_ok!(res);

		let order = OTC::orders(0).unwrap();
		assert_eq!(order.amount_in, 3 * ONE); // 100 - 97
		assert_eq!(order.amount_out, 600_000_000_000); // 20 - 19.4 = 0.6 HDX
	});
}
