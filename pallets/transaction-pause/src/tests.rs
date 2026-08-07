// Originally created by Acala. Modified by GalacticCouncil.

// Copyright (C) 2020-2022 Acala Foundation, GalacticCouncil.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Unit tests for the transaction pause module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeEvent as Event, *};
use sp_runtime::traits::BadOrigin;

const BALANCE_TRANSFER: &<Runtime as frame_system::Config>::RuntimeCall =
	&mock::RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death { dest: ALICE, value: 10 });
const TOKENS_TRANSFER: &<Runtime as frame_system::Config>::RuntimeCall =
	&mock::RuntimeCall::Tokens(orml_tokens::Call::transfer {
		dest: ALICE,
		currency_id: AUSD,
		amount: 10,
	});
const LONG_NAMED_CALL: &<Runtime as frame_system::Config>::RuntimeCall =
	&mock::RuntimeCall::LongNamedPallet(long_named_pallet::Call::some_call_with_an_intentionally_very_long_name {});

const LONG_PALLET_NAME: &[u8] = b"LongNamedPallet";
const LONG_FUNCTION_NAME: &[u8] = b"some_call_with_an_intentionally_very_long_name";

#[test]
fn pause_transaction_work() {
	ExtBuilder.build().execute_with(|| {
		let balances_b_str = BoundedName::try_from(b"Balances".to_vec()).unwrap();
		let transfer_b_str = BoundedName::try_from(b"transfer".to_vec()).unwrap();

		System::set_block_number(1);

		assert_noop!(
			TransactionPause::pause_transaction(RuntimeOrigin::signed(5), b"Balances".to_vec(), b"transfer".to_vec()),
			BadOrigin
		);

		assert_eq!(
			TransactionPause::paused_transactions((balances_b_str.clone(), transfer_b_str.clone())),
			None
		);
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			b"Balances".to_vec(),
			b"transfer".to_vec()
		));
		System::assert_last_event(Event::TransactionPause(crate::Event::TransactionPaused {
			pallet_name_bytes: b"Balances".to_vec(),
			function_name_bytes: b"transfer".to_vec(),
		}));
		assert_eq!(
			TransactionPause::paused_transactions((balances_b_str, transfer_b_str)),
			Some(())
		);

		assert_noop!(
			TransactionPause::pause_transaction(
				RuntimeOrigin::signed(1),
				b"TransactionPause".to_vec(),
				b"pause_transaction".to_vec()
			),
			Error::<Runtime>::CannotPause
		);
		assert_noop!(
			TransactionPause::pause_transaction(
				RuntimeOrigin::signed(1),
				b"TransactionPause".to_vec(),
				b"some_other_call".to_vec()
			),
			Error::<Runtime>::CannotPause
		);
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			b"OtherPallet".to_vec(),
			b"pause_transaction".to_vec()
		));

		assert_noop!(
			TransactionPause::pause_transaction(
				RuntimeOrigin::signed(1),
				vec![1u8; (MAX_STR_LENGTH + 1) as usize],
				b"transfer".to_vec()
			),
			Error::<Runtime>::NameTooLong
		);

		assert_noop!(
			TransactionPause::pause_transaction(
				RuntimeOrigin::signed(1),
				b"Balances".to_vec(),
				vec![1u8; (MAX_STR_LENGTH + 1) as usize],
			),
			Error::<Runtime>::NameTooLong
		);
	});
}

#[test]
fn unpause_transaction_work() {
	ExtBuilder.build().execute_with(|| {
		let balances_b_str = BoundedName::try_from(b"Balances".to_vec()).unwrap();
		let transfer_b_str = BoundedName::try_from(b"transfer".to_vec()).unwrap();

		System::set_block_number(1);

		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			b"Balances".to_vec(),
			b"transfer".to_vec()
		));
		assert_eq!(
			TransactionPause::paused_transactions((balances_b_str.clone(), transfer_b_str.clone())),
			Some(())
		);

		assert_noop!(
			TransactionPause::unpause_transaction(RuntimeOrigin::signed(5), b"Balances".to_vec(), b"transfer".to_vec()),
			BadOrigin
		);

		assert_ok!(TransactionPause::unpause_transaction(
			RuntimeOrigin::signed(1),
			b"Balances".to_vec(),
			b"transfer".to_vec()
		));
		System::assert_last_event(Event::TransactionPause(crate::Event::TransactionUnpaused {
			pallet_name_bytes: b"Balances".to_vec(),
			function_name_bytes: b"transfer".to_vec(),
		}));
		assert_eq!(
			TransactionPause::paused_transactions((balances_b_str, transfer_b_str)),
			None
		);

		assert_noop!(
			TransactionPause::unpause_transaction(
				RuntimeOrigin::signed(1),
				vec![1u8; (MAX_STR_LENGTH + 1) as usize],
				b"transfer".to_vec()
			),
			Error::<Runtime>::NameTooLong
		);

		assert_noop!(
			TransactionPause::unpause_transaction(
				RuntimeOrigin::signed(1),
				b"Balances".to_vec(),
				vec![1u8; (MAX_STR_LENGTH + 1) as usize],
			),
			Error::<Runtime>::NameTooLong
		);
	});
}

#[test]
fn paused_transaction_filter_work() {
	ExtBuilder.build().execute_with(|| {
		assert!(!PausedTransactionFilter::<Runtime>::contains(BALANCE_TRANSFER));
		assert!(!PausedTransactionFilter::<Runtime>::contains(TOKENS_TRANSFER));
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			b"Balances".to_vec(),
			b"transfer_allow_death".to_vec()
		));
		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			b"Tokens".to_vec(),
			b"transfer".to_vec()
		));
		assert!(PausedTransactionFilter::<Runtime>::contains(BALANCE_TRANSFER));
		assert!(PausedTransactionFilter::<Runtime>::contains(TOKENS_TRANSFER));
		assert_ok!(TransactionPause::unpause_transaction(
			RuntimeOrigin::signed(1),
			b"Balances".to_vec(),
			b"transfer_allow_death".to_vec()
		));
		assert_ok!(TransactionPause::unpause_transaction(
			RuntimeOrigin::signed(1),
			b"Tokens".to_vec(),
			b"transfer".to_vec()
		));
		assert!(!PausedTransactionFilter::<Runtime>::contains(BALANCE_TRANSFER));
		assert!(!PausedTransactionFilter::<Runtime>::contains(TOKENS_TRANSFER));
	});
}

#[test]
fn pause_transaction_should_succeed_when_function_name_exceeds_forty_bytes() {
	ExtBuilder.build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(LONG_FUNCTION_NAME.len(), 46);

		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			LONG_PALLET_NAME.to_vec(),
			LONG_FUNCTION_NAME.to_vec()
		));

		System::assert_last_event(Event::TransactionPause(crate::Event::TransactionPaused {
			pallet_name_bytes: LONG_PALLET_NAME.to_vec(),
			function_name_bytes: LONG_FUNCTION_NAME.to_vec(),
		}));

		assert_eq!(
			TransactionPause::paused_transactions((
				BoundedName::try_from(LONG_PALLET_NAME.to_vec()).unwrap(),
				BoundedName::try_from(LONG_FUNCTION_NAME.to_vec()).unwrap()
			)),
			Some(())
		);
	});
}

#[test]
fn paused_transaction_filter_should_block_call_when_function_name_exceeds_forty_bytes() {
	ExtBuilder.build().execute_with(|| {
		assert!(!PausedTransactionFilter::<Runtime>::contains(LONG_NAMED_CALL));

		assert_ok!(TransactionPause::pause_transaction(
			RuntimeOrigin::signed(1),
			LONG_PALLET_NAME.to_vec(),
			LONG_FUNCTION_NAME.to_vec()
		));

		assert!(PausedTransactionFilter::<Runtime>::contains(LONG_NAMED_CALL));

		assert_ok!(TransactionPause::unpause_transaction(
			RuntimeOrigin::signed(1),
			LONG_PALLET_NAME.to_vec(),
			LONG_FUNCTION_NAME.to_vec()
		));

		assert!(!PausedTransactionFilter::<Runtime>::contains(LONG_NAMED_CALL));
	});
}
