// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::mock::{expect_events, EmaOracle, ExtBuilder, RuntimeOrigin, System, Test, ACA, ALICE, BOB, DOT, HDX};
use super::SOURCE;
use crate::pallet::{AuthorizedAccounts, ExternalSources, Oracles};
use crate::*;

use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

const EXTERNAL_SOURCE: Source = *b"external";
const ANOTHER_SOURCE: Source = *b"another_";

const HDX_DOT_PAIR: (AssetId, AssetId) = (0, 5);

pub fn new_test_ext() -> sp_io::TestExternalities {
	ExtBuilder::default().build()
}

fn hdx_location() -> polkadot_xcm::VersionedLocation {
	polkadot_xcm::v5::Location::new(
		0,
		polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
	)
	.into_versioned()
}

fn dot_location() -> polkadot_xcm::VersionedLocation {
	polkadot_xcm::v5::Location::parent().into_versioned()
}

#[test]
fn register_external_source_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert!(ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
	});
}

#[test]
fn register_duplicate_source_fails() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_noop!(
			EmaOracle::register_external_source(RuntimeOrigin::root(), EXTERNAL_SOURCE),
			Error::<Test>::SourceAlreadyRegistered
		);
	});
}

#[test]
fn register_external_source_requires_authority() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::register_external_source(RuntimeOrigin::signed(ALICE), EXTERNAL_SOURCE),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_external_source_clears_all_pair_authorizations() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// Authorize ALICE for two different pairs under the same source.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(0, 5),
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(1, 2),
			ALICE
		));
		// And BOB for another pair.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(0, 1),
			BOB
		));

		assert_ok!(EmaOracle::remove_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		assert!(!ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
		// All pair authorizations under the removed source must be gone.
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(0, 5),
			ALICE
		)));
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(1, 2),
			ALICE
		)));
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(0, 1),
			BOB
		)));
	});
}

#[test]
fn remove_external_source_clears_oracle_data_and_accumulator() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		System::set_block_number(3);
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		));

		// In-flight accumulator entry exists pre-finalize.
		let ordered = ordered_pair(HDX_DOT_PAIR.0, HDX_DOT_PAIR.1);
		assert!(Accumulator::<Test>::get().contains_key(&(EXTERNAL_SOURCE, ordered)));

		// Flush to persistent Oracles via on_finalize.
		EmaOracle::on_finalize(3);
		System::set_block_number(4);
		let stored_before: Vec<_> = Oracles::<Test>::iter_prefix((EXTERNAL_SOURCE,)).collect();
		assert!(!stored_before.is_empty(), "Oracles row must exist pre-removal");

		// Also seed an in-flight accumulator entry for a different pair in the new block so we
		// exercise the Accumulator::retain path too.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(1, 2),
			ALICE
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			1,
			2,
			(123, 456),
		));
		assert!(Accumulator::<Test>::get().contains_key(&(EXTERNAL_SOURCE, ordered_pair(1, 2))));

		assert_ok!(EmaOracle::remove_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		// 1. Source registration gone.
		assert!(!ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
		// 2. Authorizations gone (existing coverage, re-asserted here for integration).
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			ordered,
			ALICE
		)));
		// 3. Committed Oracles rows gone — this is the new guarantee.
		assert_eq!(
			Oracles::<Test>::iter_prefix((EXTERNAL_SOURCE,)).count(),
			0,
			"committed Oracles rows must be cleared across every supported period"
		);
		// 4. In-flight accumulator entries for this source also dropped.
		let acc = Accumulator::<Test>::get();
		assert!(
			!acc.keys().any(|(s, _)| *s == EXTERNAL_SOURCE),
			"no accumulator entries for the removed source may survive"
		);
	});
}

#[test]
fn remove_external_source_only_touches_the_targeted_source() {
	new_test_ext().execute_with(|| {
		const THIRD_SOURCE: Source = *b"third___";
		const NEIGHBOR_SOURCE: Source = *b"externaL"; // byte-adjacent to EXTERNAL_SOURCE

		let pair_hd = ordered_pair(HDX, DOT);
		let pair_ha = ordered_pair(HDX, ACA);
		let pair_da = ordered_pair(DOT, ACA);
		let pair_auth_only = ordered_pair(4, 5);

		// ─── AMM SOURCE: 2 whitelisted pairs ───
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, DOT)));
		assert_ok!(EmaOracle::add_oracle(RuntimeOrigin::root(), SOURCE, (HDX, ACA)));

		// ─── EXTERNAL_SOURCE: 3 pairs with data + 1 pair auth-only ───
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(HDX, DOT),
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(HDX, ACA),
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(DOT, ACA),
			BOB
		));
		// Auth-only — never submitted to; verifies clearing works for pairs without Oracles rows.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(4, 5),
			ALICE
		));

		// ─── ANOTHER_SOURCE: 2 pairs overlapping with EXTERNAL_SOURCE pairs ───
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE,
			(HDX, DOT),
			BOB
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE,
			(DOT, ACA),
			BOB
		));

		// ─── THIRD_SOURCE: empty (registered but no auths, no data) ───
		assert_ok!(EmaOracle::register_external_source(RuntimeOrigin::root(), THIRD_SOURCE));

		// ─── NEIGHBOR_SOURCE: byte-adjacent to EXTERNAL_SOURCE, on shared pair ───
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			NEIGHBOR_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			NEIGHBOR_SOURCE,
			(HDX, DOT),
			ALICE
		));

		// ─── Block 3: write across every (source, pair) with data ───
		System::set_block_number(3);

		// AMM writes (two pairs).
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			1_000,
			500,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
			Some(2_000_u128),
		));
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			ACA,
			800,
			400,
			2_000,
			1_000,
			Price::new(2_000, 1_000),
			Some(2_000_u128),
		));
		// EXTERNAL_SOURCE writes on 3 pairs (auth-only pair deliberately not written).
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			HDX,
			DOT,
			(3_000, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			HDX,
			ACA,
			(3_500, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(BOB),
			EXTERNAL_SOURCE,
			DOT,
			ACA,
			(4_000, 1_000),
		));
		// ANOTHER_SOURCE writes on 2 pairs.
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(BOB),
			ANOTHER_SOURCE,
			HDX,
			DOT,
			(5_000, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(BOB),
			ANOTHER_SOURCE,
			DOT,
			ACA,
			(5_500, 1_000),
		));
		// NEIGHBOR_SOURCE write on shared pair.
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			NEIGHBOR_SOURCE,
			HDX,
			DOT,
			(7_000, 1_000),
		));

		// Commit to Oracles for every supported period.
		EmaOracle::on_finalize(3);
		System::set_block_number(4);

		// Snapshot the per-source row counts so post-removal assertions can compare exactly.
		let rows = |s: Source| Oracles::<Test>::iter_prefix((s,)).count();
		let source_rows_pre = rows(SOURCE);
		let another_rows_pre = rows(ANOTHER_SOURCE);
		let neighbor_rows_pre = rows(NEIGHBOR_SOURCE);
		assert!(source_rows_pre >= 2, "AMM: expect ≥2 pairs × N periods rows");
		assert!(rows(EXTERNAL_SOURCE) >= 3, "EXTERNAL_SOURCE: 3 data pairs");
		assert!(another_rows_pre >= 2, "ANOTHER_SOURCE: 2 pairs");
		assert!(neighbor_rows_pre >= 1, "NEIGHBOR_SOURCE: 1 pair");

		// ─── Seed in-flight Accumulator entries across all sources in the new block ───
		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			HDX,
			DOT,
			2_000,
			1_000,
			4_000,
			2_000,
			Price::new(2_000, 1_000),
			Some(2_000_u128),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			HDX,
			DOT,
			(9_000, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(BOB),
			EXTERNAL_SOURCE,
			DOT,
			ACA,
			(9_100, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(BOB),
			ANOTHER_SOURCE,
			HDX,
			DOT,
			(11_000, 1_000),
		));
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			NEIGHBOR_SOURCE,
			HDX,
			DOT,
			(13_000, 1_000),
		));
		let acc_before = Accumulator::<Test>::get();
		assert!(acc_before.contains_key(&(SOURCE, pair_hd)));
		assert!(acc_before.contains_key(&(EXTERNAL_SOURCE, pair_hd)));
		assert!(acc_before.contains_key(&(EXTERNAL_SOURCE, pair_da)));
		assert!(acc_before.contains_key(&(ANOTHER_SOURCE, pair_hd)));
		assert!(acc_before.contains_key(&(NEIGHBOR_SOURCE, pair_hd)));

		// ─── ACT ───
		assert_ok!(EmaOracle::remove_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		// ─── ASSERT: EXTERNAL_SOURCE fully wiped across every pair ───
		assert!(!ExternalSources::<Test>::contains_key(EXTERNAL_SOURCE));
		assert_eq!(
			AuthorizedAccounts::<Test>::iter_prefix((EXTERNAL_SOURCE,)).count(),
			0,
			"all 4 authorizations (incl. the data-less (4,5)) must be cleared"
		);
		assert_eq!(
			Oracles::<Test>::iter_prefix((EXTERNAL_SOURCE,)).count(),
			0,
			"all 3 data pairs' Oracles rows must be cleared across every period"
		);
		let acc_after = Accumulator::<Test>::get();
		assert!(!acc_after.contains_key(&(EXTERNAL_SOURCE, pair_hd)));
		assert!(!acc_after.contains_key(&(EXTERNAL_SOURCE, pair_ha)));
		assert!(!acc_after.contains_key(&(EXTERNAL_SOURCE, pair_da)));
		assert!(!acc_after.contains_key(&(EXTERNAL_SOURCE, pair_auth_only)));

		// ─── ASSERT: AMM SOURCE untouched (both pairs) ───
		assert!(WhitelistedAssets::<Test>::get().contains(&(SOURCE, (HDX, DOT))));
		assert!(WhitelistedAssets::<Test>::get().contains(&(SOURCE, (HDX, ACA))));
		assert_eq!(rows(SOURCE), source_rows_pre, "AMM Oracles row count must be unchanged");
		assert!(acc_after.contains_key(&(SOURCE, pair_hd)));

		// ─── ASSERT: ANOTHER_SOURCE untouched (both overlapping pairs) ───
		assert!(ExternalSources::<Test>::contains_key(ANOTHER_SOURCE));
		assert!(AuthorizedAccounts::<Test>::contains_key((ANOTHER_SOURCE, pair_hd, BOB)));
		assert!(AuthorizedAccounts::<Test>::contains_key((ANOTHER_SOURCE, pair_da, BOB)));
		assert_eq!(
			rows(ANOTHER_SOURCE),
			another_rows_pre,
			"ANOTHER_SOURCE Oracles row count must be unchanged"
		);
		assert!(acc_after.contains_key(&(ANOTHER_SOURCE, pair_hd)));

		// ─── ASSERT: NEIGHBOR_SOURCE (byte-adjacent) untouched ───
		assert!(ExternalSources::<Test>::contains_key(NEIGHBOR_SOURCE));
		assert!(AuthorizedAccounts::<Test>::contains_key((
			NEIGHBOR_SOURCE,
			pair_hd,
			ALICE
		)));
		assert_eq!(
			rows(NEIGHBOR_SOURCE),
			neighbor_rows_pre,
			"byte-adjacent NEIGHBOR_SOURCE must be untouched"
		);
		assert!(acc_after.contains_key(&(NEIGHBOR_SOURCE, pair_hd)));

		// ─── ASSERT: THIRD_SOURCE (empty registration) untouched ───
		assert!(ExternalSources::<Test>::contains_key(THIRD_SOURCE));
	});
}

#[test]
fn remove_nonexistent_source_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::remove_external_source(RuntimeOrigin::root(), EXTERNAL_SOURCE),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn add_authorized_account_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));
		assert!(AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		)));
	});
}

#[test]
fn add_authorized_account_stores_in_ordered_pair_form() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// Pass the pair in reverse order; storage must be keyed by the ordered form.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(5, 0),
			ALICE
		));
		assert!(AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			ordered_pair(0, 5),
			ALICE
		)));
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(5, 0),
			ALICE
		)));
	});
}

#[test]
fn add_account_for_nonexistent_source_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::add_authorized_account(RuntimeOrigin::root(), EXTERNAL_SOURCE, HDX_DOT_PAIR, ALICE),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn remove_authorized_account_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));
		assert_ok!(EmaOracle::remove_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		)));
	});
}

#[test]
fn remove_authorized_account_only_affects_the_given_pair() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// ALICE is authorized for two pairs under the same source.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(0, 5),
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(1, 2),
			ALICE
		));

		// Revoking one pair must leave the other intact.
		assert_ok!(EmaOracle::remove_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(0, 5),
			ALICE
		));
		assert!(!AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(0, 5),
			ALICE
		)));
		assert!(AuthorizedAccounts::<Test>::contains_key((
			EXTERNAL_SOURCE,
			(1, 2),
			ALICE
		)));
	});
}

#[test]
fn set_external_oracle_happy_path() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		System::set_block_number(3);

		let res = EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		);
		assert_eq!(res, Ok(Pays::No.into()));

		// Verify the entry is in the accumulator
		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn set_external_oracle_unauthorized_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(100, 99),
			),
			Error::<Test>::NotAuthorized
		);
	});
}

// Core DDoS protection invariant: an account authorized for pair A must NOT be able to push
// updates for pair B under the same source. This is the test that prevents the regression
// the refactor was introduced to fix.
#[test]
fn authorized_account_cannot_update_unauthorized_pair() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// ALICE is authorized ONLY for (0, 1), not for (0, 5).
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(0, 1),
			ALICE
		));

		System::set_block_number(3);

		// Attempting to update (hdx, dot) = (0, 5) must fail.
		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(100, 99),
			),
			Error::<Test>::NotAuthorized
		);
	});
}

#[test]
fn set_external_oracle_zero_price_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(0, 100),
			),
			Error::<Test>::PriceIsZero
		);

		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(100, 0),
			),
			Error::<Test>::PriceIsZero
		);
	});
}

#[test]
fn set_external_oracle_unregistered_source_rejected() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(100, 99),
			),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn external_sources_bypass_whitelist() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		// Use INSUFFICIENT_ASSET which is normally excluded by the whitelist
		System::set_block_number(3);

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		));

		// Verify the entry is in the accumulator (bypasses whitelist)
		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn multiple_sources_in_same_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			ANOTHER_SOURCE,
			HDX_DOT_PAIR,
			BOB
		));

		System::set_block_number(3);

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		));

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(BOB),
			ANOTHER_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(200, 99),
		));

		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
		assert!(acc.contains_key(&(ANOTHER_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn amm_trades_are_limited_to_max_unique_entries() {
	new_test_ext().execute_with(|| {
		//Arrange
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();

		//Act - fill the accumulator to max
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}

		//Assert - accumulator is full, next AMM trade fails
		assert_eq!(Accumulator::<Test>::get().len(), max_entries as usize);
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(
				SOURCE,
				2 * max_entries,
				2 * max_entries + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			)
			.map_err(|(_w, e)| e),
			Error::<Test>::TooManyUniqueEntries
		);
	});
}

#[test]
fn soft_limit_only_applies_to_non_external_sources() {
	new_test_ext().execute_with(|| {
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();

		// Fill the accumulator to max with AMM trades
		for i in 0..max_entries {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}

		// Non-external source should fail when accumulator is full
		assert_noop!(
			OnActivityHandler::<Test>::on_trade(
				SOURCE,
				2 * max_entries,
				2 * max_entries + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			)
			.map_err(|(_w, e)| e),
			Error::<Test>::TooManyUniqueEntries
		);

		// But external sources should still be able to insert beyond the limit
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		));

		// Accumulator has more entries than MaxUniqueEntries
		assert_eq!(Accumulator::<Test>::get().len(), (max_entries + 1) as usize);
	});
}

#[test]
fn external_entries_do_not_block_amm_new_pair_trades() {
	new_test_ext().execute_with(|| {
		let max_entries = <<Test as crate::Config>::MaxUniqueEntries as Get<u32>>::get();

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			100,
			101,
			1_000,
			1_000,
			2_000,
			2_000,
			Price::new(2_000, 2_000),
			Some(1_000_u128),
		));
		assert_eq!(Accumulator::<Test>::get().len(), 1);

		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		for i in 0..(max_entries - 1) {
			assert_ok!(OnActivityHandler::<Test>::on_trade(
				EXTERNAL_SOURCE,
				i,
				i + 1,
				1_000,
				1_000,
				2_000,
				2_000,
				Price::new(2_000, 2_000),
				Some(1_000_u128),
			));
		}
		assert_eq!(Accumulator::<Test>::get().len(), max_entries as usize);

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			100,
			101,
			1_000,
			1_000,
			2_000,
			2_000,
			Price::new(2_000, 2_000),
			Some(1_000_u128),
		));

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			0,
			1,
			1_000,
			1_000,
			2_000,
			2_000,
			Price::new(2_000, 2_000),
			Some(1_000_u128),
		));

		assert_ok!(OnActivityHandler::<Test>::on_trade(
			SOURCE,
			2 * max_entries,
			2 * max_entries + 1,
			1_000,
			1_000,
			2_000,
			2_000,
			Price::new(2_000, 2_000),
			Some(1_000_u128),
		));

		assert_eq!(Accumulator::<Test>::get().len(), (max_entries + 2) as usize);
	});
}

#[test]
fn account_can_update_only_explicitly_authorized_pairs_in_one_block() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		// Build several distinct locations that the mock converter resolves to distinct asset IDs.
		let loc_0 = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(0)].into()),
		)
		.into_versioned(); // → asset 0
		let loc_1 = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(1)].into()),
		)
		.into_versioned(); // → asset 1
		let loc_2 = polkadot_xcm::v5::Location::new(
			0,
			polkadot_xcm::v5::Junctions::X1([polkadot_xcm::v5::Junction::GeneralIndex(2)].into()),
		)
		.into_versioned(); // → asset 2
		let loc_dot = polkadot_xcm::v5::Location::parent().into_versioned(); // → asset 5

		// ALICE is authorized for exactly three pairs: (0, 1), (0, 2), (2, 5).
		// She is NOT authorized for (0, 5), so that update must fail.
		for pair in &[(0_u32, 1_u32), (0, 2), (2, 5)] {
			assert_ok!(EmaOracle::add_authorized_account(
				RuntimeOrigin::root(),
				EXTERNAL_SOURCE,
				*pair,
				ALICE
			));
		}

		System::set_block_number(3);

		// Three authorized pairs land in the accumulator in the same block.
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(loc_0.clone()),
			Box::new(loc_1.clone()),
			(100, 99),
		));
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(loc_0.clone()),
			Box::new(loc_2.clone()),
			(200, 99),
		));
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(loc_2.clone()),
			Box::new(loc_dot.clone()),
			(300, 99),
		));

		// The unauthorized pair (0, 5) is rejected — this is the DDoS mitigation.
		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(loc_0),
				Box::new(loc_dot),
				(400, 99),
			),
			Error::<Test>::NotAuthorized
		);

		let acc = Accumulator::<Test>::get();
		assert_eq!(acc.len(), 3);
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 1))));
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 2))));
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(2, 5))));
		// The rejected pair did NOT land in the accumulator.
		assert!(!acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn set_external_oracle_accepts_reversed_location_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR, // canonical (0, 5)
			ALICE
		));

		System::set_block_number(3);

		// Call with (dot, hdx) instead of (hdx, dot).
		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(dot_location()),
			Box::new(hdx_location()),
			(100, 99),
		));

		// Accumulator stores in ordered_pair form regardless of call-site order.
		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(0, 5))));
	});
}

#[test]
fn add_authorized_account_requires_external_oracle_origin() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_noop!(
			EmaOracle::add_authorized_account(RuntimeOrigin::signed(ALICE), EXTERNAL_SOURCE, HDX_DOT_PAIR, BOB),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_authorized_account_requires_external_oracle_origin() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));
		assert_noop!(
			EmaOracle::remove_authorized_account(RuntimeOrigin::signed(BOB), EXTERNAL_SOURCE, HDX_DOT_PAIR, ALICE),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_external_source_requires_external_oracle_origin() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_noop!(
			EmaOracle::remove_external_source(RuntimeOrigin::signed(ALICE), EXTERNAL_SOURCE),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn remove_account_for_nonexistent_source_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::remove_authorized_account(RuntimeOrigin::root(), EXTERNAL_SOURCE, HDX_DOT_PAIR, ALICE),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn authorized_account_events_carry_pair_in_ordered_form() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1); // events are only recorded on blocks >= 1

		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		// Intentionally pass the pair reversed so we prove ordering normalization happens
		// before the event is emitted.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(5, 0),
			ALICE
		));
		expect_events(vec![crate::Event::AuthorizedAccountAdded {
			source: EXTERNAL_SOURCE,
			pair: ordered_pair(0, 5),
			account: ALICE,
		}
		.into()]);

		assert_ok!(EmaOracle::remove_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			(5, 0),
			ALICE
		));
		expect_events(vec![crate::Event::AuthorizedAccountRemoved {
			source: EXTERNAL_SOURCE,
			pair: ordered_pair(0, 5),
			account: ALICE,
		}
		.into()]);
	});
}

#[test]
fn set_external_oracle_rejected_after_source_removed() {
	new_test_ext().execute_with(|| {
		// Arrange: source + authorization, and a baseline successful update.
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		System::set_block_number(3);

		assert_ok!(EmaOracle::set_external_oracle(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			Box::new(hdx_location()),
			Box::new(dot_location()),
			(100, 99),
		));

		// Act: governance removes the entire source.
		assert_ok!(EmaOracle::remove_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		// Assert: the same caller, pair, and price now hits the source gate first.
		assert_noop!(
			EmaOracle::set_external_oracle(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				Box::new(hdx_location()),
				Box::new(dot_location()),
				(100, 99),
			),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn set_external_oracle_by_ids_happy_path() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		System::set_block_number(3);

		let res = EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR.0,
			HDX_DOT_PAIR.1,
			(100, 99),
		);
		assert_eq!(res, Ok(Pays::No.into()));

		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(HDX_DOT_PAIR.0, HDX_DOT_PAIR.1))));
	});
}

#[test]
fn set_external_oracle_by_ids_unauthorized_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));

		assert_noop!(
			EmaOracle::set_external_oracle_by_ids(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				HDX_DOT_PAIR.0,
				HDX_DOT_PAIR.1,
				(100, 99),
			),
			Error::<Test>::NotAuthorized
		);
	});
}

#[test]
fn set_external_oracle_by_ids_unknown_asset_id_returns_not_authorized() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// Authorize ALICE only for the real pair.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		const BOGUS_ASSET_ID: AssetId = 99_999;
		assert_noop!(
			EmaOracle::set_external_oracle_by_ids(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				HDX_DOT_PAIR.0,
				BOGUS_ASSET_ID,
				(100, 99),
			),
			Error::<Test>::NotAuthorized
		);
	});
}

#[test]
fn set_external_oracle_by_ids_zero_price_rejected() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		assert_noop!(
			EmaOracle::set_external_oracle_by_ids(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				HDX_DOT_PAIR.0,
				HDX_DOT_PAIR.1,
				(0, 100),
			),
			Error::<Test>::PriceIsZero
		);

		assert_noop!(
			EmaOracle::set_external_oracle_by_ids(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				HDX_DOT_PAIR.0,
				HDX_DOT_PAIR.1,
				(100, 0),
			),
			Error::<Test>::PriceIsZero
		);
	});
}

#[test]
fn set_external_oracle_by_ids_unregistered_source_rejected() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			EmaOracle::set_external_oracle_by_ids(
				RuntimeOrigin::signed(ALICE),
				EXTERNAL_SOURCE,
				HDX_DOT_PAIR.0,
				HDX_DOT_PAIR.1,
				(100, 99),
			),
			Error::<Test>::SourceNotFound
		);
	});
}

#[test]
fn set_external_oracle_by_ids_accepts_reversed_id_order() {
	new_test_ext().execute_with(|| {
		assert_ok!(EmaOracle::register_external_source(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE
		));
		// Authorize the ordered pair (HDX_DOT_PAIR.0, HDX_DOT_PAIR.1) where .0 < .1.
		assert_ok!(EmaOracle::add_authorized_account(
			RuntimeOrigin::root(),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR,
			ALICE
		));

		System::set_block_number(3);

		// Caller passes the pair in reversed order; ordered_pair() normalizes it so auth still matches.
		assert_ok!(EmaOracle::set_external_oracle_by_ids(
			RuntimeOrigin::signed(ALICE),
			EXTERNAL_SOURCE,
			HDX_DOT_PAIR.1,
			HDX_DOT_PAIR.0,
			(100, 99),
		));

		let acc = Accumulator::<Test>::get();
		assert!(acc.contains_key(&(EXTERNAL_SOURCE, ordered_pair(HDX_DOT_PAIR.0, HDX_DOT_PAIR.1))));
	});
}
