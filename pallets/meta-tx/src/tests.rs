// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

use crate::mock::*;
use crate::{Error, Event, Nonces};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::traits::Dispatchable;
use sp_runtime::DispatchError;

const UNITS: Balance = 1_000_000_000_000;

fn transfer_call(to: AccountId, amount: Balance) -> RuntimeCall {
	RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death {
		dest: to,
		value: amount,
	})
}

fn submit(
	relayer: AccountId,
	signer: AccountId,
	call: RuntimeCall,
	nonce: u32,
	deadline: u64,
	signature: sp_runtime::MultiSignature,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
	RuntimeCall::MetaTx(crate::Call::dispatch_meta_tx {
		signer,
		call: Box::new(call),
		nonce,
		deadline,
		signature,
	})
	.dispatch(RuntimeOrigin::signed(relayer))
}

fn signed_submit(
	relayer: AccountId,
	pair: &sp_core::sr25519::Pair,
	call: RuntimeCall,
	nonce: u32,
	deadline: u64,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
	let signer = account_of(pair);
	let payload = MetaTx::signing_payload(&call, &signer, nonce, deadline);
	let signature = sign(pair, &payload);
	submit(relayer, signer, call, nonce, deadline, signature)
}

#[test]
fn dispatch_meta_tx_should_execute_under_signer_origin_when_signature_is_valid() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.with_balance(relayer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let relayer_before = Balances::free_balance(&relayer);

			assert_ok!(signed_submit(
				relayer.clone(),
				&alice,
				transfer_call(recipient.clone(), UNITS),
				0,
				100
			));

			assert_eq!(
				Balances::free_balance(&signer),
				9 * UNITS,
				"the transfer must debit the signer, not the relayer"
			);
			assert_eq!(Balances::free_balance(&recipient), UNITS);
			assert_eq!(
				Balances::free_balance(&relayer),
				relayer_before,
				"the relayer's balance is untouched by the inner call"
			);
			assert_eq!(Nonces::<Test>::get(&signer), 1);

			System::assert_has_event(
				Event::Dispatched {
					signer,
					relayer,
					nonce: 0,
					result: Ok(()),
				}
				.into(),
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_signature_belongs_to_another_account() {
	let alice = keypair("Alice");
	let mallory = keypair("Mallory");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient, UNITS);
			let payload = MetaTx::signing_payload(&call, &signer, 0, 100);
			let signature = sign(&mallory, &payload);

			assert_noop!(
				submit(relayer, signer.clone(), call, 0, 100, signature),
				Error::<Test>::InvalidSignature
			);
			assert_eq!(Nonces::<Test>::get(&signer), 0);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_call_is_swapped_after_signing() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));
	let attacker = account_of(&keypair("Mallory"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let authorised = transfer_call(recipient, UNITS);
			let payload = MetaTx::signing_payload(&authorised, &signer, 0, 100);
			let signature = sign(&alice, &payload);

			let substituted = transfer_call(attacker, 9 * UNITS);

			assert_noop!(
				submit(relayer, signer.clone(), substituted, 0, 100, signature),
				Error::<Test>::InvalidSignature
			);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_deadline_is_changed_after_signing() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient, UNITS);
			let payload = MetaTx::signing_payload(&call, &signer, 0, 50);
			let signature = sign(&alice, &payload);

			assert_noop!(
				submit(relayer, signer, call, 0, 500, signature),
				Error::<Test>::InvalidSignature
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_replayed_with_the_same_signature() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient, UNITS);
			let payload = MetaTx::signing_payload(&call, &signer, 0, 100);
			let signature = sign(&alice, &payload);

			assert_ok!(submit(
				relayer.clone(),
				signer.clone(),
				call.clone(),
				0,
				100,
				signature.clone()
			));
			assert_noop!(
				submit(relayer, signer.clone(), call, 0, 100, signature),
				Error::<Test>::InvalidNonce
			);
			assert_eq!(
				Balances::free_balance(&signer),
				9 * UNITS,
				"the transfer ran exactly once"
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_nonce_is_ahead_of_the_signer() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			assert_noop!(
				signed_submit(relayer, &alice, transfer_call(recipient, UNITS), 7, 100),
				Error::<Test>::InvalidNonce
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_deadline_has_passed() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			System::set_block_number(101);

			assert_noop!(
				signed_submit(relayer, &alice, transfer_call(recipient, UNITS), 0, 100),
				Error::<Test>::Expired
			);
			assert_eq!(Nonces::<Test>::get(&signer), 0);
		});
}

#[test]
fn dispatch_meta_tx_should_succeed_when_submitted_on_the_deadline_block() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer, 10 * UNITS)
		.build()
		.execute_with(|| {
			System::set_block_number(100);
			assert_ok!(signed_submit(relayer, &alice, transfer_call(recipient, UNITS), 0, 100));
		});
}

#[test]
fn dispatch_meta_tx_should_consume_the_nonce_when_the_inner_call_fails() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), UNITS)
		.build()
		.execute_with(|| {
			assert_ok!(signed_submit(
				relayer.clone(),
				&alice,
				transfer_call(recipient.clone(), 1_000 * UNITS),
				0,
				100
			));

			assert_eq!(
				Nonces::<Test>::get(&signer),
				1,
				"a failed intent MUST NOT stay replayable"
			);
			assert_eq!(Balances::free_balance(&recipient), 0);

			let dispatched = System::events().into_iter().any(|record| {
				matches!(
					record.event,
					RuntimeEvent::MetaTx(Event::Dispatched { result: Err(_), .. })
				)
			});
			assert!(
				dispatched,
				"the inner failure is reported in the event, not as an extrinsic error"
			);
		});
}

#[test]
fn dispatch_meta_tx_should_execute_every_leg_when_inner_call_is_batch_all() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let first = account_of(&keypair("First"));
	let second = account_of(&keypair("Second"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
				calls: vec![
					transfer_call(first.clone(), UNITS),
					transfer_call(second.clone(), 2 * UNITS),
				],
			});

			assert_ok!(signed_submit(relayer, &alice, batch, 0, 100));

			assert_eq!(Balances::free_balance(&first), UNITS);
			assert_eq!(Balances::free_balance(&second), 2 * UNITS);
			assert_eq!(Balances::free_balance(&signer), 7 * UNITS);
		});
}

#[test]
fn dispatch_meta_tx_should_roll_back_every_leg_when_one_leg_of_batch_all_fails() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let first = account_of(&keypair("First"));
	let second = account_of(&keypair("Second"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
				calls: vec![
					transfer_call(first.clone(), UNITS),
					transfer_call(second.clone(), 1_000 * UNITS),
				],
			});

			assert_ok!(signed_submit(relayer, &alice, batch, 0, 100));

			assert_eq!(Balances::free_balance(&first), 0, "batch_all is all-or-nothing");
			assert_eq!(Balances::free_balance(&second), 0);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
			assert_eq!(Nonces::<Test>::get(&signer), 1);
		});
}

#[test]
fn dispatch_meta_tx_should_reject_an_unsigned_origin() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient, UNITS);
			let payload = MetaTx::signing_payload(&call, &signer, 0, 100);
			let signature = sign(&alice, &payload);

			let outcome = RuntimeCall::MetaTx(crate::Call::dispatch_meta_tx {
				signer,
				call: Box::new(call),
				nonce: 0,
				deadline: 100,
				signature,
			})
			.dispatch(RuntimeOrigin::none());

			assert_eq!(outcome.unwrap_err().error, DispatchError::BadOrigin);
		});
}

#[test]
fn signing_payload_should_change_when_any_bound_field_changes() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let other = account_of(&keypair("Bob"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default().build().execute_with(|| {
		let call = transfer_call(recipient.clone(), UNITS);
		let base = MetaTx::signing_payload(&call, &signer, 0, 100);

		assert_ne!(base, MetaTx::signing_payload(&call, &other, 0, 100), "signer is bound");
		assert_ne!(base, MetaTx::signing_payload(&call, &signer, 1, 100), "nonce is bound");
		assert_ne!(
			base,
			MetaTx::signing_payload(&call, &signer, 0, 101),
			"deadline is bound"
		);
		assert_ne!(
			base,
			MetaTx::signing_payload(&transfer_call(recipient, 2 * UNITS), &signer, 0, 100),
			"call is bound"
		);
	});
}

#[test]
fn nonces_should_advance_independently_per_signer() {
	let alice = keypair("Alice");
	let bob = keypair("Bob");
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(account_of(&alice), 10 * UNITS)
		.with_balance(account_of(&bob), 10 * UNITS)
		.build()
		.execute_with(|| {
			assert_ok!(signed_submit(
				relayer.clone(),
				&alice,
				transfer_call(recipient.clone(), UNITS),
				0,
				100
			));
			assert_ok!(signed_submit(
				relayer.clone(),
				&bob,
				transfer_call(recipient.clone(), UNITS),
				0,
				100
			));
			assert_ok!(signed_submit(relayer, &alice, transfer_call(recipient, UNITS), 1, 100));

			assert_eq!(Nonces::<Test>::get(account_of(&alice)), 2);
			assert_eq!(Nonces::<Test>::get(account_of(&bob)), 1);
		});
}
