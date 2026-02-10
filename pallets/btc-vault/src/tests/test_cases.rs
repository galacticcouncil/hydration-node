use super::*;
use frame_support::{assert_noop, assert_ok, BoundedVec};

#[test]
fn pause_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(BtcVault::pause(RuntimeOrigin::root()));
		assert!(BtcVault::pallet_config().unwrap().paused);
		System::assert_last_event(crate::Event::<Test>::Paused.into());
	});
}

#[test]
fn pause_requires_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			BtcVault::pause(RuntimeOrigin::signed(1)),
			frame_support::error::BadOrigin
		);
	});
}

#[test]
fn unpause_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(BtcVault::pause(RuntimeOrigin::root()));
		assert!(BtcVault::pallet_config().unwrap().paused);

		assert_ok!(BtcVault::unpause(RuntimeOrigin::root()));
		assert!(!BtcVault::pallet_config().unwrap().paused);
		System::assert_last_event(crate::Event::<Test>::Unpaused.into());
	});
}

#[test]
fn unpause_requires_root() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			BtcVault::unpause(RuntimeOrigin::signed(1)),
			frame_support::error::BadOrigin
		);
	});
}

fn insert_pending_deposit(request_id: crate::Bytes32, requester: u64, amount_sats: u64) {
	crate::PendingDeposits::<Test>::insert(
		request_id,
		crate::PendingDepositData {
			requester,
			amount_sats,
			txid: [1u8; 32],
			path: BoundedVec::try_from(b"0xdeadbeef".to_vec()).unwrap(),
		},
	);
}

fn dummy_signature() -> pallet_signet::Signature {
	pallet_signet::Signature {
		big_r: pallet_signet::AffinePoint {
			x: [0u8; 32],
			y: [0u8; 32],
		},
		s: [0u8; 32],
		recovery_id: 0,
	}
}

#[test]
fn claim_deposit_works() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		let amount = 50_000u64;
		insert_pending_deposit(request_id, 1, amount);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_ok!(BtcVault::claim_deposit(
			RuntimeOrigin::signed(1),
			request_id,
			output,
			dummy_signature(),
		));

		assert_eq!(BtcVault::user_balances(1), amount);
		assert!(BtcVault::pending_deposits(request_id).is_none());
		System::assert_last_event(
			crate::Event::<Test>::DepositClaimed {
				request_id,
				claimer: 1,
				amount_sats: amount,
			}
			.into(),
		);
	});
}

#[test]
fn claim_deposit_accumulates_balance() {
	new_test_ext().execute_with(|| {
		let id1 = [1u8; 32];
		let id2 = [2u8; 32];
		insert_pending_deposit(id1, 1, 30_000);
		insert_pending_deposit(id2, 1, 20_000);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_ok!(BtcVault::claim_deposit(
			RuntimeOrigin::signed(1),
			id1,
			output.clone(),
			dummy_signature(),
		));
		assert_eq!(BtcVault::user_balances(1), 30_000);

		assert_ok!(BtcVault::claim_deposit(
			RuntimeOrigin::signed(1),
			id2,
			output,
			dummy_signature(),
		));
		assert_eq!(BtcVault::user_balances(1), 50_000);
	});
}

#[test]
fn claim_deposit_fails_not_found() {
	new_test_ext().execute_with(|| {
		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_noop!(
			BtcVault::claim_deposit(
				RuntimeOrigin::signed(1),
				[99u8; 32],
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::DepositNotFound
		);
	});
}

#[test]
fn claim_deposit_fails_unauthorized_claimer() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		insert_pending_deposit(request_id, 1, 50_000);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_noop!(
			BtcVault::claim_deposit(
				RuntimeOrigin::signed(2),
				request_id,
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::UnauthorizedClaimer
		);
	});
}

#[test]
fn claim_deposit_fails_on_false_output() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		insert_pending_deposit(request_id, 1, 50_000);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![0u8]).unwrap();

		assert_noop!(
			BtcVault::claim_deposit(
				RuntimeOrigin::signed(1),
				request_id,
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::TransferFailed
		);
	});
}

#[test]
fn claim_deposit_fails_on_invalid_output() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		insert_pending_deposit(request_id, 1, 50_000);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![2u8]).unwrap();

		assert_noop!(
			BtcVault::claim_deposit(
				RuntimeOrigin::signed(1),
				request_id,
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::InvalidOutput
		);
	});
}

#[test]
fn create_p2wpkh_script_is_correct() {
	new_test_ext().execute_with(|| {
		let pubkey_hash = [0xAA; 20];
		let script = crate::Pallet::<Test>::create_p2wpkh_script(&pubkey_hash);
		assert_eq!(script.len(), 22);
		assert_eq!(script[0], 0x00);
		assert_eq!(script[1], 0x14);
		assert_eq!(&script[2..], &pubkey_hash);
	});
}

#[test]
fn decode_success_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(crate::Pallet::<Test>::decode_success(&[1u8]).unwrap(), true);
		assert_eq!(crate::Pallet::<Test>::decode_success(&[0u8]).unwrap(), false);
		assert!(crate::Pallet::<Test>::decode_success(&[2u8]).is_err());
		assert!(crate::Pallet::<Test>::decode_success(&[]).is_err());
	});
}

#[test]
fn ensure_not_paused_works() {
	new_test_ext().execute_with(|| {
		assert!(crate::Pallet::<Test>::ensure_not_paused().is_ok());

		assert_ok!(BtcVault::pause(RuntimeOrigin::root()));
		assert!(crate::Pallet::<Test>::ensure_not_paused().is_err());

		assert_ok!(BtcVault::unpause(RuntimeOrigin::root()));
		assert!(crate::Pallet::<Test>::ensure_not_paused().is_ok());
	});
}

// ========= Withdrawal tests =========

fn insert_pending_withdrawal(request_id: crate::Bytes32, requester: u64, amount_sats: u64) {
	crate::PendingWithdrawals::<Test>::insert(
		request_id,
		crate::PendingWithdrawalData {
			requester,
			amount_sats,
		},
	);
}

#[test]
fn complete_withdraw_btc_success() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		let amount = 50_000u64;
		crate::UserBalances::<Test>::insert(1u64, 100_000u64);
		insert_pending_withdrawal(request_id, 1, amount);

		// borsh-encoded true
		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_ok!(BtcVault::complete_withdraw_btc(
			RuntimeOrigin::signed(1),
			request_id,
			output,
			dummy_signature(),
		));

		// Balance should remain unchanged (already decremented at withdrawal time)
		assert_eq!(BtcVault::user_balances(1), 100_000);
		assert!(crate::PendingWithdrawals::<Test>::get(request_id).is_none());
		System::assert_last_event(
			crate::Event::<Test>::WithdrawalCompleted {
				request_id,
				requester: 1,
				amount_sats: amount,
			}
			.into(),
		);
	});
}

#[test]
fn complete_withdraw_btc_refund_on_error() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		let amount = 50_000u64;
		let initial_balance = 100_000u64;
		crate::UserBalances::<Test>::insert(1u64, initial_balance);
		insert_pending_withdrawal(request_id, 1, amount);

		// Error prefix output
		let mut error_output = vec![0xde, 0xad, 0xbe, 0xef];
		error_output.extend_from_slice(b"some error message");
		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(error_output).unwrap();

		assert_ok!(BtcVault::complete_withdraw_btc(
			RuntimeOrigin::signed(1),
			request_id,
			output,
			dummy_signature(),
		));

		// Balance should be refunded
		assert_eq!(BtcVault::user_balances(1), initial_balance + amount);
		assert!(crate::PendingWithdrawals::<Test>::get(request_id).is_none());
		System::assert_last_event(
			crate::Event::<Test>::WithdrawalFailed {
				request_id,
				requester: 1,
				amount_sats: amount,
			}
			.into(),
		);
	});
}

#[test]
fn complete_withdraw_btc_refund_on_false() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		let amount = 50_000u64;
		let initial_balance = 100_000u64;
		crate::UserBalances::<Test>::insert(1u64, initial_balance);
		insert_pending_withdrawal(request_id, 1, amount);

		// borsh-encoded false
		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![0u8]).unwrap();

		assert_ok!(BtcVault::complete_withdraw_btc(
			RuntimeOrigin::signed(1),
			request_id,
			output,
			dummy_signature(),
		));

		assert_eq!(BtcVault::user_balances(1), initial_balance + amount);
		System::assert_last_event(
			crate::Event::<Test>::WithdrawalFailed {
				request_id,
				requester: 1,
				amount_sats: amount,
			}
			.into(),
		);
	});
}

#[test]
fn complete_withdraw_btc_fails_not_found() {
	new_test_ext().execute_with(|| {
		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_noop!(
			BtcVault::complete_withdraw_btc(
				RuntimeOrigin::signed(1),
				[99u8; 32],
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::WithdrawalNotFound
		);
	});
}

#[test]
fn complete_withdraw_btc_fails_unauthorized() {
	new_test_ext().execute_with(|| {
		let request_id = [42u8; 32];
		insert_pending_withdrawal(request_id, 1, 50_000);

		let output: BoundedVec<u8, frame_support::traits::ConstU32<{ crate::MAX_SERIALIZED_OUTPUT_LENGTH }>> =
			BoundedVec::try_from(vec![1u8]).unwrap();

		assert_noop!(
			BtcVault::complete_withdraw_btc(
				RuntimeOrigin::signed(2),
				request_id,
				output,
				dummy_signature(),
			),
			crate::Error::<Test>::UnauthorizedClaimer
		);
	});
}
