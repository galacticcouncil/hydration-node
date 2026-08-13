use crate::polkadot_test_net::*;

use frame_support::assert_ok;
use frame_support::dispatch::GetDispatchInfo;
use hydradx_runtime::{AccountId, Currencies, MetaTx, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, System};
use orml_traits::MultiCurrency;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_core::{sr25519, Encode, Pair};
use sp_runtime::traits::{DispatchTransaction, Dispatchable, IdentifyAccount, TransactionExtension};
use sp_runtime::{MultiSignature, MultiSigner};
use xcm_emulator::TestExt;

fn keypair(name: &str) -> sr25519::Pair {
	sr25519::Pair::from_string(&format!("//{}", name), None).expect("static seed is valid; qed")
}

fn account_of(pair: &sr25519::Pair) -> AccountId {
	MultiSigner::from(pair.public()).into_account()
}

fn signed_meta_tx(pair: &sr25519::Pair, call: RuntimeCall, deadline: u32) -> RuntimeCall {
	let signer = account_of(pair);
	let nonce = MetaTx::nonce(&signer);
	let payload = MetaTx::signing_payload(&call, &signer, nonce, deadline);
	let signature = MultiSignature::from(pair.sign(&payload));

	RuntimeCall::MetaTx(pallet_meta_tx::Call::dispatch_meta_tx {
		signer,
		call: Box::new(call),
		nonce,
		deadline,
		signature,
	})
}

/// Submit `call` through the full signed-extension pipeline so real fees are charged.
fn submit_as(relayer: &AccountId, call: RuntimeCall) -> frame_support::dispatch::DispatchResultWithPostInfo {
	let info = call.get_dispatch_info();
	let len = call.encoded_size();

	let pre = ChargeTransactionPayment::<Runtime>::from(0)
		.validate_and_prepare(Some(relayer.clone()).into(), &call, &info, len, 0)
		.expect("fee pre-dispatch must succeed");
	let (pre_data, _) = pre;

	let result = call.dispatch(RuntimeOrigin::signed(relayer.clone()));

	let mut post = result.unwrap_or_else(|e| e.post_info);
	assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
		pre_data,
		&info,
		&mut post,
		len,
		&Ok(())
	));

	result
}

fn remark(tag: &[u8]) -> RuntimeCall {
	RuntimeCall::System(frame_system::Call::remark_with_event { remark: tag.to_vec() })
}

fn remarked_by(who: &AccountId) -> bool {
	System::events().iter().any(
		|record| matches!(&record.event, RuntimeEvent::System(frame_system::Event::Remarked { sender, .. }) if sender == who),
	)
}

#[test]
fn meta_tx_should_be_fully_sponsored_when_signer_has_zero_balance() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxSigner");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();

		assert_eq!(
			Currencies::free_balance(HDX, &signer),
			0,
			"the signer must start with nothing"
		);
		let relayer_before = Currencies::free_balance(HDX, &relayer);

		assert_ok!(submit_as(&relayer, signed_meta_tx(&pair, remark(b"sponsored"), 1_000)));

		assert!(
			remarked_by(&signer),
			"the inner call MUST execute under the zero-balance signer's own origin"
		);
		assert_eq!(
			Currencies::free_balance(HDX, &signer),
			0,
			"the signer MUST NOT be charged anything"
		);
		assert!(
			Currencies::free_balance(HDX, &relayer) < relayer_before,
			"the relayer MUST bear the whole cost"
		);
		assert_eq!(MetaTx::nonce(&signer), 1);
	});
}

#[test]
fn meta_tx_should_execute_every_leg_when_signer_batches_calls() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxBatcher");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();
		let recipient: AccountId = BOB.into();

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			signer.clone(),
			HDX,
			(10 * UNITS) as i128,
		));
		let recipient_before = Currencies::free_balance(HDX, &recipient);
		let relayer_before = Currencies::free_balance(HDX, &relayer);

		let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
			calls: vec![
				RuntimeCall::Currencies(pallet_currencies::Call::transfer {
					dest: recipient.clone(),
					currency_id: HDX,
					amount: UNITS,
				}),
				RuntimeCall::Currencies(pallet_currencies::Call::transfer {
					dest: recipient.clone(),
					currency_id: HDX,
					amount: 2 * UNITS,
				}),
			],
		});

		assert_ok!(submit_as(&relayer, signed_meta_tx(&pair, batch, 1_000)));

		assert_eq!(
			Currencies::free_balance(HDX, &recipient) - recipient_before,
			3 * UNITS,
			"both legs must run under the signer's origin"
		);
		assert_eq!(
			Currencies::free_balance(HDX, &signer),
			7 * UNITS,
			"the signer pays only what the legs transfer, no fee"
		);
		assert!(Currencies::free_balance(HDX, &relayer) < relayer_before);
	});
}

#[test]
fn meta_tx_should_fail_when_replayed_by_the_relayer() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxReplay");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();

		let call = signed_meta_tx(&pair, remark(b"once"), 1_000);

		assert_ok!(submit_as(&relayer, call.clone()));
		assert_eq!(MetaTx::nonce(&signer), 1);

		let replay = call.dispatch(RuntimeOrigin::signed(relayer));
		assert!(replay.is_err(), "a consumed intent MUST NOT run twice");
		assert_eq!(MetaTx::nonce(&signer), 1);
	});
}

#[test]
fn meta_tx_should_fail_when_deadline_has_passed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxExpired");
		let relayer: AccountId = ALICE.into();

		let deadline = System::block_number();
		let call = signed_meta_tx(&pair, remark(b"stale"), deadline);
		System::set_block_number(deadline + 1);

		let outcome = call.dispatch(RuntimeOrigin::signed(relayer));
		assert!(outcome.is_err(), "an expired intent MUST be rejected");
		assert!(!remarked_by(&account_of(&pair)));
	});
}

#[test]
fn meta_tx_should_respect_the_runtime_call_filter() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxFiltered");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();
		let recipient: AccountId = BOB.into();

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			signer.clone(),
			HDX,
			(10 * UNITS) as i128,
		));
		let recipient_before = Currencies::free_balance(HDX, &recipient);

		assert_ok!(hydradx_runtime::TransactionPause::pause_transaction(
			RuntimeOrigin::root(),
			b"Currencies".to_vec(),
			b"transfer".to_vec(),
		));

		let transfer = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: recipient.clone(),
			currency_id: HDX,
			amount: UNITS,
		});
		let outcome = submit_as(&relayer, signed_meta_tx(&pair, transfer, 1_000));
		assert!(
			outcome.is_err(),
			"a filtered call MUST surface to the relayer as an extrinsic error"
		);

		assert_eq!(
			Currencies::free_balance(HDX, &recipient),
			recipient_before,
			"a paused call MUST NOT execute just because it was signed"
		);
		assert_eq!(
			MetaTx::nonce(&signer),
			0,
			"a rejected intent reverts its nonce and stays valid until its deadline"
		);
	});
}

#[test]
fn meta_tx_nonce_should_be_independent_of_the_system_nonce() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxNonce");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();

		let system_nonce_before = System::account_nonce(&signer);

		assert_ok!(submit_as(&relayer, signed_meta_tx(&pair, remark(b"a"), 1_000)));
		assert_ok!(submit_as(&relayer, signed_meta_tx(&pair, remark(b"b"), 1_000)));

		assert_eq!(MetaTx::nonce(&signer), 2);
		assert_eq!(
			System::account_nonce(&signer),
			system_nonce_before,
			"meta transactions MUST NOT disturb the signer's ordinary nonce"
		);
	});
}

#[test]
fn meta_tx_should_fail_when_signer_sources_an_evm_call_from_another_address() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let pair = keypair("MetaTxEvm");
		let signer = account_of(&pair);
		let relayer: AccountId = ALICE.into();

		let evm_call = RuntimeCall::Dispatcher(pallet_dispatcher::Call::dispatch_evm_call {
			call: Box::new(RuntimeCall::EVM(pallet_evm::Call::call {
				source: sp_core::H160::repeat_byte(0xaa),
				target: sp_core::H160::repeat_byte(0xbb),
				input: vec![0x06, 0xfd, 0xde, 0x03],
				value: sp_core::U256::zero(),
				gas_limit: 100_000,
				max_fee_per_gas: sp_core::U256::from(26_663_905u128),
				max_priority_fee_per_gas: None,
				nonce: None,
				access_list: vec![],
				authorization_list: vec![],
			})),
		});

		let outcome = submit_as(&relayer, signed_meta_tx(&pair, evm_call, 1_000));

		assert!(
			outcome.is_err(),
			"a signer MUST NOT be able to source an EVM call from an address it does not control"
		);
		assert_eq!(MetaTx::nonce(&signer), 0);
	});
}
