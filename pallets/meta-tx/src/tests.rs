// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

use crate::mock::*;
use crate::{Error, Event, Nonces};
use frame_support::dispatch::{DispatchErrorWithPostInfo, GetDispatchInfo, PostDispatchInfo};
use frame_support::pallet_prelude::{Get, Pays, Weight};
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::evm::InspectEvmAccounts;
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

fn verification_weight() -> Weight {
	<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::dispatch_meta_tx()
}

fn rejected(error: Error<Test>) -> DispatchErrorWithPostInfo {
	DispatchErrorWithPostInfo {
		post_info: PostDispatchInfo {
			actual_weight: Some(verification_weight()),
			pays_fee: Pays::Yes,
		},
		error: error.into(),
	}
}

fn dispatched_result() -> sp_runtime::DispatchResult {
	System::events()
		.into_iter()
		.rev()
		.find_map(|record| match record.event {
			RuntimeEvent::MetaTx(Event::Dispatched { result, .. }) => Some(result),
			_ => None,
		})
		.expect("an admitted meta transaction always emits Dispatched; qed")
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
				rejected(Error::<Test>::InvalidSignature)
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
				rejected(Error::<Test>::InvalidSignature)
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
				submit(relayer, signer, call, 0, 60, signature),
				rejected(Error::<Test>::InvalidSignature)
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
				rejected(Error::<Test>::InvalidNonce)
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
				rejected(Error::<Test>::InvalidNonce)
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
				rejected(Error::<Test>::Expired)
			);
			assert_eq!(Nonces::<Test>::get(&signer), 0);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_deadline_is_beyond_the_maximum() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer, 10 * UNITS)
		.build()
		.execute_with(|| {
			let max: u64 = <Test as crate::Config>::MaxDeadline::get();
			let too_far = System::block_number() + max + 1;

			assert_noop!(
				signed_submit(relayer, &alice, transfer_call(recipient, UNITS), 0, too_far),
				rejected(Error::<Test>::DeadlineTooFar)
			);
		});
}

#[test]
fn dispatch_meta_tx_should_succeed_when_deadline_is_exactly_the_maximum() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer, 10 * UNITS)
		.build()
		.execute_with(|| {
			let max: u64 = <Test as crate::Config>::MaxDeadline::get();
			let deadline = System::block_number() + max;

			assert_ok!(signed_submit(
				relayer,
				&alice,
				transfer_call(recipient.clone(), UNITS),
				0,
				deadline
			));
			assert_eq!(Balances::free_balance(&recipient), UNITS);
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
fn dispatch_meta_tx_should_report_the_inner_error_in_the_event_when_the_inner_call_fails() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient.clone(), 1_000 * UNITS);
			let expected = call
				.clone()
				.dispatch(RuntimeOrigin::signed(signer.clone()))
				.expect_err("the transfer exceeds the signer's balance; qed")
				.error;

			assert_ok!(signed_submit(relayer, &alice, call, 0, 100));

			assert_eq!(
				dispatched_result(),
				Err(expected),
				"the relayer MUST be able to read the inner call's own error"
			);
			assert_eq!(Balances::free_balance(&recipient), 0);
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
			let call = transfer_call(recipient, 1_000 * UNITS);

			assert_ok!(signed_submit(relayer, &alice, call, 0, 100));

			assert_eq!(
				Nonces::<Test>::get(&signer),
				1,
				"a consumed nonce is what stops a failed intent from being replayed"
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_replayed_after_the_inner_call_failed() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient.clone(), 1_000 * UNITS);
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
			assert!(dispatched_result().is_err());

			Balances::make_free_balance_be(&signer, 10_000 * UNITS);

			assert_noop!(
				submit(relayer, signer, call, 0, 100, signature),
				rejected(Error::<Test>::InvalidNonce)
			);
			assert_eq!(
				Balances::free_balance(&recipient),
				0,
				"the same bytes MUST NOT execute once conditions turn favourable"
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
			assert!(
				dispatched_result().is_err(),
				"a failing leg MUST be visible to the relayer, not a silent success"
			);

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

#[test]
fn dispatch_meta_tx_should_refund_the_inner_call_weight_when_signature_is_invalid() {
	let alice = keypair("Alice");
	let mallory = keypair("Mallory");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let heavy = RuntimeCall::Utility(pallet_utility::Call::batch_all {
				calls: vec![transfer_call(recipient, UNITS); 100],
			});
			let payload = MetaTx::signing_payload(&heavy, &signer, 0, 100);
			let signature = sign(&mallory, &payload);

			let declared = RuntimeCall::MetaTx(crate::Call::dispatch_meta_tx {
				signer: signer.clone(),
				call: Box::new(heavy.clone()),
				nonce: 0,
				deadline: 100,
				signature: signature.clone(),
			})
			.get_dispatch_info()
			.call_weight;

			let outcome = submit(relayer, signer, heavy, 0, 100, signature);
			let err = outcome.expect_err("an invalid signature must be rejected; qed");

			assert_eq!(err.post_info.actual_weight, Some(verification_weight()));
			assert!(
				verification_weight().all_lt(declared),
				"the relayer MUST NOT be charged for an inner call that never ran"
			);
		});
}

#[test]
fn dispatch_meta_tx_should_fail_when_the_signers_nonce_cannot_be_advanced() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			Nonces::<Test>::insert(&signer, u32::MAX);

			assert_noop!(
				signed_submit(relayer, &alice, transfer_call(recipient, UNITS), u32::MAX, 100),
				rejected(Error::<Test>::NonceExhausted)
			);
			assert_eq!(
				Nonces::<Test>::get(&signer),
				u32::MAX,
				"a saturating nonce MUST NOT leave the intent replayable"
			);
		});
}

fn evm_submit(
	relayer: AccountId,
	pair: &sp_core::ecdsa::Pair,
	call: RuntimeCall,
	nonce: u32,
	deadline: u64,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
	let from = evm_address_of(pair);
	let payload = MetaTx::evm_signing_payload(&call, from, nonce, deadline);
	let (v, r, s) = evm_sign(pair, &payload);

	RuntimeCall::MetaTx(crate::Call::dispatch_evm_meta_tx {
		from,
		call: Box::new(call),
		nonce,
		deadline,
		v,
		r,
		s,
	})
	.dispatch(RuntimeOrigin::signed(relayer))
}

fn evm_rejected(error: Error<Test>) -> DispatchErrorWithPostInfo {
	DispatchErrorWithPostInfo {
		post_info: PostDispatchInfo {
			actual_weight: Some(<<Test as crate::Config>::WeightInfo as crate::WeightInfo>::dispatch_evm_meta_tx()),
			pays_fee: Pays::Yes,
		},
		error: error.into(),
	}
}

#[test]
fn dispatch_evm_meta_tx_should_execute_under_the_evm_address_account_when_signature_is_valid() {
	let alice = evm_keypair("AliceEvm");
	let signer = MockEvmAccounts::account_id(evm_address_of(&alice));
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.with_balance(relayer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let relayer_before = Balances::free_balance(&relayer);

			assert_ok!(evm_submit(
				relayer.clone(),
				&alice,
				transfer_call(recipient.clone(), UNITS),
				0,
				100
			));

			assert_eq!(
				Balances::free_balance(&signer),
				9 * UNITS,
				"an EVM signature must move the EVM account's own funds"
			);
			assert_eq!(Balances::free_balance(&recipient), UNITS);
			assert_eq!(Balances::free_balance(&relayer), relayer_before);
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
fn dispatch_evm_meta_tx_should_fail_when_signature_belongs_to_another_key() {
	let alice = evm_keypair("AliceEvm");
	let mallory = evm_keypair("MalloryEvm");
	let from = evm_address_of(&alice);
	let signer = MockEvmAccounts::account_id(from);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let call = transfer_call(recipient, UNITS);
			let payload = MetaTx::evm_signing_payload(&call, from, 0, 100);
			let (v, r, s) = evm_sign(&mallory, &payload);

			assert_noop!(
				RuntimeCall::MetaTx(crate::Call::dispatch_evm_meta_tx {
					from,
					call: Box::new(call),
					nonce: 0,
					deadline: 100,
					v,
					r,
					s,
				})
				.dispatch(RuntimeOrigin::signed(relayer)),
				evm_rejected(Error::<Test>::InvalidEvmSignature)
			);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
		});
}

#[test]
fn dispatch_evm_meta_tx_should_fail_when_call_is_swapped_after_signing() {
	let alice = evm_keypair("AliceEvm");
	let from = evm_address_of(&alice);
	let signer = MockEvmAccounts::account_id(from);
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));
	let attacker = account_of(&keypair("Mallory"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let authorised = transfer_call(recipient, UNITS);
			let payload = MetaTx::evm_signing_payload(&authorised, from, 0, 100);
			let (v, r, s) = evm_sign(&alice, &payload);

			assert_noop!(
				RuntimeCall::MetaTx(crate::Call::dispatch_evm_meta_tx {
					from,
					call: Box::new(transfer_call(attacker, 9 * UNITS)),
					nonce: 0,
					deadline: 100,
					v,
					r,
					s,
				})
				.dispatch(RuntimeOrigin::signed(relayer)),
				evm_rejected(Error::<Test>::InvalidEvmSignature)
			);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
		});
}

#[test]
fn dispatch_evm_meta_tx_should_fail_when_replayed_with_the_same_signature() {
	let alice = evm_keypair("AliceEvm");
	let signer = MockEvmAccounts::account_id(evm_address_of(&alice));
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			assert_ok!(evm_submit(
				relayer.clone(),
				&alice,
				transfer_call(recipient.clone(), UNITS),
				0,
				100
			));
			assert_noop!(
				evm_submit(relayer, &alice, transfer_call(recipient, UNITS), 0, 100),
				evm_rejected(Error::<Test>::InvalidNonce)
			);
			assert_eq!(Balances::free_balance(&signer), 9 * UNITS);
		});
}

#[test]
fn dispatch_evm_meta_tx_should_roll_back_every_leg_when_one_leg_of_batch_all_fails() {
	let alice = evm_keypair("AliceEvm");
	let signer = MockEvmAccounts::account_id(evm_address_of(&alice));
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
					transfer_call(first.clone(), UNITS),
				],
			});

			assert_ok!(evm_submit(relayer, &alice, batch, 0, 100));
			assert!(
				dispatched_result().is_err(),
				"a failing leg MUST be visible to the relayer"
			);

			assert_eq!(Balances::free_balance(&first), 0, "the loop is all-or-nothing");
			assert_eq!(Balances::free_balance(&second), 0);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
			assert_eq!(Nonces::<Test>::get(&signer), 1);
		});
}

#[test]
fn dispatch_evm_meta_tx_should_restore_the_previous_evm_fee_payer_after_the_inner_call() {
	let alice = evm_keypair("AliceEvm");
	let signer = MockEvmAccounts::account_id(evm_address_of(&alice));
	let relayer = account_of(&keypair("Relayer"));
	let recipient = account_of(&keypair("Recipient"));
	let incumbent = account_of(&keypair("Incumbent"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			FeePayer::set(&Some(incumbent.clone()));

			assert_ok!(evm_submit(relayer, &alice, transfer_call(recipient, UNITS), 0, 100));

			assert_eq!(
				FeePayer::get(),
				Some(incumbent),
				"an outer fee payer MUST survive a nested meta transaction"
			);
		});
}

#[test]
fn evm_signing_payload_should_change_when_any_bound_field_changes() {
	let alice = evm_keypair("AliceEvm");
	let bob = evm_keypair("BobEvm");
	let recipient = account_of(&keypair("Recipient"));

	ExtBuilder::default().build().execute_with(|| {
		let call = transfer_call(recipient.clone(), UNITS);
		let from = evm_address_of(&alice);
		let base = MetaTx::evm_signing_payload(&call, from, 0, 100);

		assert_ne!(base, MetaTx::evm_signing_payload(&call, evm_address_of(&bob), 0, 100));
		assert_ne!(base, MetaTx::evm_signing_payload(&call, from, 1, 100));
		assert_ne!(base, MetaTx::evm_signing_payload(&call, from, 0, 101));
		assert_ne!(
			base,
			MetaTx::evm_signing_payload(&transfer_call(recipient, 2 * UNITS), from, 0, 100)
		);
	});
}

fn nested_batch(first: AccountId, second: AccountId) -> RuntimeCall {
	RuntimeCall::Utility(pallet_utility::Call::batch_all {
		calls: vec![
			transfer_call(first.clone(), UNITS),
			RuntimeCall::Utility(pallet_utility::Call::batch_all {
				calls: vec![transfer_call(second.clone(), UNITS), transfer_call(second, 2 * UNITS)],
			}),
			transfer_call(first, UNITS),
		],
	})
}

#[test]
fn nested_batch_all_should_be_rejected_when_the_signer_dispatches_it_directly() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let first = account_of(&keypair("First"));
	let second = account_of(&keypair("Second"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let outcome = nested_batch(first.clone(), second.clone()).dispatch(RuntimeOrigin::signed(signer.clone()));

			assert_eq!(
				outcome.expect_err("utility bans nested batch_all; qed").error,
				DispatchError::from(frame_system::Error::<Test>::CallFiltered),
				"pallet_utility adds a filter to the origin it hands each leg"
			);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
		});
}

#[test]
fn dispatch_meta_tx_should_reject_a_nested_batch_all_exactly_as_a_direct_dispatch_would() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let first = account_of(&keypair("First"));
	let second = account_of(&keypair("Second"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			assert_ok!(signed_submit(
				relayer,
				&alice,
				nested_batch(first.clone(), second.clone()),
				0,
				100
			));

			assert_eq!(
				dispatched_result(),
				Err(DispatchError::from(frame_system::Error::<Test>::CallFiltered)),
				"a meta transaction MUST behave exactly as the signer dispatching the call themselves"
			);
			assert_eq!(Balances::free_balance(&first), 0);
			assert_eq!(Balances::free_balance(&second), 0);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
			assert_eq!(Nonces::<Test>::get(&signer), 1);
		});
}

#[test]
fn dispatch_meta_tx_should_not_roll_back_when_the_inner_call_is_utility_batch() {
	let alice = keypair("Alice");
	let signer = account_of(&alice);
	let relayer = account_of(&keypair("Relayer"));
	let first = account_of(&keypair("First"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			let batch = RuntimeCall::Utility(pallet_utility::Call::batch {
				calls: vec![
					transfer_call(first.clone(), UNITS),
					transfer_call(first.clone(), 1_000 * UNITS),
				],
			});

			assert_ok!(signed_submit(relayer, &alice, batch, 0, 100));

			assert_eq!(
				Balances::free_balance(&first),
				UNITS,
				"utility::batch reports Ok on a failing leg, so nothing above it can unwind — relayers MUST use batch_all"
			);
			assert_eq!(Nonces::<Test>::get(&signer), 1);
		});
}

#[test]
fn dispatch_evm_meta_tx_should_reject_a_nested_batch_all_exactly_as_a_direct_dispatch_would() {
	let alice = evm_keypair("AliceEvm");
	let signer = MockEvmAccounts::account_id(evm_address_of(&alice));
	let relayer = account_of(&keypair("Relayer"));
	let first = account_of(&keypair("First"));
	let second = account_of(&keypair("Second"));

	ExtBuilder::default()
		.with_balance(signer.clone(), 10 * UNITS)
		.build()
		.execute_with(|| {
			assert_ok!(evm_submit(
				relayer,
				&alice,
				nested_batch(first.clone(), second.clone()),
				0,
				100
			));

			assert_eq!(
				dispatched_result(),
				Err(DispatchError::from(frame_system::Error::<Test>::CallFiltered)),
				"the EVM path MUST inherit the same call filtering as the substrate path"
			);
			assert_eq!(Balances::free_balance(&first), 0);
			assert_eq!(Balances::free_balance(&second), 0);
			assert_eq!(Balances::free_balance(&signer), 10 * UNITS);
			assert_eq!(Nonces::<Test>::get(&signer), 1);
		});
}
