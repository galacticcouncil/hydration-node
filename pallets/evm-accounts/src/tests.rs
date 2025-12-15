// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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

use super::*;
use frame_support::pallet_prelude::InvalidTransaction;
use frame_support::pallet_prelude::TransactionSource;
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_support::sp_runtime::traits::ValidateUnsigned;
use frame_support::sp_runtime::traits::Zero;
use frame_support::sp_runtime::DispatchError;
use frame_support::unsigned::TransactionValidityError;
use frame_support::{assert_noop, assert_ok, assert_storage_noop};
use hex_literal::hex;
use mock::*;
use orml_traits::MultiCurrency;

#[test]
fn eth_address_should_convert_to_truncated_address_when_not_bound() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let evm_address = H160::from(hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"]);
		let truncated_address =
			AccountId::from(hex!["45544800222222ff7be76052e023ec1a306fcca8f9659d800000000000000000"]);

		assert_eq!(EVMAccounts::truncated_account_id(evm_address), truncated_address);

		// Act & Assert
		assert_eq!(EVMAccounts::bound_account_id(evm_address), None);
		assert_eq!(EVMAccounts::account_id(evm_address), truncated_address);
	});
}

#[test]
fn eth_address_should_convert_to_full_address_when_bound() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE),));

		// Assert
		let evm_address = EVMAccounts::evm_address(&ALICE);

		assert_eq!(EVMAccounts::bound_account_id(evm_address), Some(ALICE));

		assert_eq!(EVMAccounts::account_id(evm_address), ALICE);

		expect_events(vec![Event::Bound {
			account: ALICE,
			address: evm_address,
		}
		.into()]);
	});
}

#[test]
fn bind_evm_address_should_increase_sufficients() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		assert!(System::sufficients(&ALICE).is_zero());

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE)));

		// Assert
		assert_eq!(System::sufficients(&ALICE), 1);
	});
}

#[test]
fn evm_address_is_reversible_from_account_id() {
	ExtBuilder::default().build().execute_with(|| {
		let evm_address = H160::from(hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"]);
		assert_eq!(
			EVMAccounts::evm_address(&EVMAccounts::account_id(evm_address)),
			evm_address
		);
	});
}

#[test]
fn account_id_is_reversible_from_evm_address() {
	ExtBuilder::default().build().execute_with(|| {
		let evm_address = H160::from(hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"]);
		assert_eq!(
			EVMAccounts::account_id(EVMAccounts::evm_address(&EVMAccounts::account_id(evm_address))),
			EVMAccounts::account_id(evm_address)
		);
	});
}

#[test]
fn account_id_is_reversible_from_bound_evm_address() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE)));
		assert_eq!(EVMAccounts::account_id(EVMAccounts::evm_address(&ALICE)), ALICE);
	});
}

#[test]
fn bound_evm_address_is_reversible_from_account_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE)));
		assert_eq!(
			EVMAccounts::evm_address(&EVMAccounts::account_id(EVMAccounts::evm_address(&ALICE))),
			EVMAccounts::evm_address(&ALICE)
		);
	});
}

#[test]
fn bind_address_should_fail_when_nonce_is_not_zero() {
	ExtBuilder::default()
		.with_non_zero_nonce(ALICE)
		.build()
		.execute_with(|| {
			assert_noop!(
				EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE)),
				Error::<Test>::TruncatedAccountAlreadyUsed
			);
		});
}

#[test]
fn bind_address_should_fail_when_binding_evm_truncated_account() {
	ExtBuilder::default().build().execute_with(|| {
		let evm_address = H160::from(hex!["222222ff7Be76052e023Ec1a306fCca8F9659D80"]);
		let account_id = EVMAccounts::account_id(evm_address);
		assert_noop!(
			EVMAccounts::bind_evm_address(RuntimeOrigin::signed(account_id)),
			Error::<Test>::TruncatedAccountAlreadyUsed
		);
	});
}

#[test]
fn bind_address_should_fail_when_already_bound() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE),));
		assert_noop!(
			EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE)),
			Error::<Test>::AddressAlreadyBound
		);
	});
}

#[test]
fn add_contract_deployer_should_store_address_in_the_storage() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let evm_address = EVMAccounts::evm_address(&ALICE);
		assert!(!EVMAccounts::can_deploy_contracts(evm_address));

		// Act
		assert_ok!(EVMAccounts::add_contract_deployer(RuntimeOrigin::root(), evm_address));

		// Assert
		assert!(EVMAccounts::can_deploy_contracts(evm_address));
		expect_events(vec![Event::DeployerAdded { who: evm_address }.into()]);

		// adding the address again should be ok
		assert_ok!(EVMAccounts::add_contract_deployer(RuntimeOrigin::root(), evm_address));
	});
}

#[test]
fn remove_contract_deployer_should_remove_address_from_the_storage() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let evm_address = EVMAccounts::evm_address(&ALICE);
		assert_ok!(EVMAccounts::add_contract_deployer(RuntimeOrigin::root(), evm_address));
		assert!(EVMAccounts::can_deploy_contracts(evm_address));

		// Act
		assert_ok!(EVMAccounts::remove_contract_deployer(
			RuntimeOrigin::root(),
			evm_address
		));

		// Assert
		assert!(!EVMAccounts::can_deploy_contracts(evm_address));
		expect_events(vec![Event::DeployerRemoved { who: evm_address }.into()]);

		// removing the address again should be ok
		assert_ok!(EVMAccounts::remove_contract_deployer(
			RuntimeOrigin::root(),
			evm_address
		));
	});
}

#[test]
fn renounce_contract_deployer_should_remove_address_from_the_storage() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let evm_address = EVMAccounts::evm_address(&ALICE);
		assert_ok!(EVMAccounts::add_contract_deployer(RuntimeOrigin::root(), evm_address));
		assert!(EVMAccounts::can_deploy_contracts(evm_address));

		// Act
		assert_ok!(EVMAccounts::renounce_contract_deployer(RuntimeOrigin::signed(ALICE)));

		// Assert
		assert!(!EVMAccounts::can_deploy_contracts(evm_address));
		expect_events(vec![Event::DeployerRemoved { who: evm_address }.into()]);

		// ronouncing the address again should be ok
		assert_ok!(EVMAccounts::renounce_contract_deployer(RuntimeOrigin::signed(ALICE)));
	});
}

#[test]
fn verify_signed_message_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		let signature = sign_message::<Test>(pair, &account, HDX);
		let msg = EVMAccounts::create_claim_account_message(&account, HDX);

		// Assert
		assert!(signature.verify(msg.as_slice(), &account.clone().into()));
	});
}

#[test]
fn verify_signed_message_should_fail_if_different_account() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		let signature = sign_message::<Test>(pair, &account, HDX);
		let msg = EVMAccounts::create_claim_account_message(&account, HDX);

		// Assert
		assert_eq!(signature.verify(msg.as_slice(), &ALICE.into()), false);
	});
}

#[test]
fn verify_signed_message_should_fail_if_different_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		let signature = sign_message::<Test>(pair, &account, HDX);
		let msg = EVMAccounts::create_claim_account_message(&account, 1);

		// Assert
		assert_eq!(signature.verify(msg.as_slice(), &account.into()), false);
	});
}

#[test]
fn verify_signed_message_should_fail_if_different_message() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange & Act
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		let signature = sign_message::<Test>(pair, &account, HDX);

		// Assert
		assert_eq!(signature.verify("wrong message".as_bytes(), &account.into()), false);
	});
}

#[test]
fn claim_account_should_bound_address() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DOT, &account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DOT);

		// Act
		assert_ok!(EVMAccounts::claim_account(
			RuntimeOrigin::none(),
			account.clone(),
			DOT,
			signature
		));

		// Assert
		let evm_address = EVMAccounts::evm_address(&account);

		assert_eq!(EVMAccounts::bound_account_id(evm_address), Some(account.clone()));
		assert_eq!(EVMAccounts::account_id(evm_address), account);
	});
}

#[test]
fn claim_account_should_increase_sufficients() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DOT, &account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DOT);

		assert!(System::sufficients(&account).is_zero());

		// Act
		assert_ok!(EVMAccounts::claim_account(
			RuntimeOrigin::none(),
			account.clone(),
			DOT,
			signature
		));

		// Assert
		assert_eq!(System::sufficients(&account), 1);
	});
}

#[test]
fn claim_account_should_set_fee_payment_currency() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DOT, &account, INITIAL_BALANCE));
		assert_eq!(FeeCurrencyMock::get(&account), HDX);

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DOT);

		// Act
		assert_ok!(EVMAccounts::claim_account(
			RuntimeOrigin::none(),
			account.clone(),
			DOT,
			signature
		));

		// Assert
		assert_eq!(FeeCurrencyMock::get(&account), DOT);
	});
}

#[test]
fn claim_account_should_fail_for_signed_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(HDX, &account, INITIAL_BALANCE));
		assert!(System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, HDX);

		// Act & Assert
		assert_noop!(
			EVMAccounts::claim_account(RuntimeOrigin::signed(account.clone()), account, HDX, signature),
			DispatchError::BadOrigin
		);
	});
}

#[test]
fn claim_account_should_fail_if_account_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(HDX, &account, INITIAL_BALANCE));
		assert!(System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, HDX);

		// Act & Assert
		assert_noop!(
			EVMAccounts::claim_account(RuntimeOrigin::none(), account, HDX, signature),
			Error::<Test>::AccountAlreadyExists
		);
	});
}

#[test]
fn claim_account_should_fail_if_signature_is_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, 1);

		// Act & Assert
		assert_noop!(
			EVMAccounts::claim_account(RuntimeOrigin::none(), account, HDX, signature),
			Error::<Test>::InvalidSignature
		);
	});
}

#[test]
fn claim_account_should_fail_if_not_enough_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, HDX);

		// Act & Assert
		assert_noop!(
			EVMAccounts::claim_account(RuntimeOrigin::none(), account, HDX, signature),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn validate_unsigned_should_pass_if_correct_call() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DOT, &account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DOT);

		let call = Call::claim_account {
			account,
			asset_id: DOT,
			signature,
		};

		// Act & Assert
		assert_storage_noop!({
			let res = EVMAccounts::validate_unsigned(TransactionSource::Local, &call);
			assert_ok!(res);
		});
	});
}

#[test]
fn validate_unsigned_should_fail_if_account_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DOT, &account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		assert!(System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DOT);

		let call = Call::claim_account {
			account,
			asset_id: DOT,
			signature,
		};

		// Act & Assert
		assert_storage_noop!({
			let res = EVMAccounts::validate_unsigned(TransactionSource::Local, &call);
			assert_noop!(res, TransactionValidityError::Invalid(InvalidTransaction::Call));
		});
	});
}

#[test]
fn validate_unsigned_should_pass_if_signature_is_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		// use different asset to create invalid signature
		let signature = sign_message::<Test>(pair, &account, HDX);

		let call = Call::claim_account {
			account,
			asset_id: HDX,
			signature,
		};

		// Act & Assert
		assert_storage_noop!({
			let res = EVMAccounts::validate_unsigned(TransactionSource::Local, &call);
			assert_noop!(res, TransactionValidityError::Invalid(InvalidTransaction::Call));
		});
	});
}

#[test]
fn validate_unsigned_should_fail_if_asset_is_not_valid_fee_payment_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();

		assert_ok!(Currencies::deposit(DAI, &account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(account.clone());
		assert!(!System::account_exists(&account));

		let signature = sign_message::<Test>(pair, &account, DAI);

		let call = Call::claim_account {
			account,
			asset_id: DAI,
			signature,
		};

		// Act & Assert
		assert_storage_noop!({
			let res = EVMAccounts::validate_unsigned(TransactionSource::Local, &call);
			assert_noop!(res, TransactionValidityError::Invalid(InvalidTransaction::Call));
		});
	});
}

#[test]
fn validate_unsigned_should_fail_if_account_is_truncated() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let account = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account();
		let evm_address = EVMAccounts::evm_address(&account);
		let truncated_account = EVMAccounts::truncated_account_id(evm_address);

		assert_ok!(Currencies::deposit(DOT, &truncated_account, INITIAL_BALANCE));

		// Remove account from the system pallet, but keep DOT balance in the tokens pallet
		frame_system::Account::<Test>::remove(truncated_account.clone());
		assert!(!System::account_exists(&truncated_account));

		let signature = sign_message::<Test>(pair, &truncated_account, DOT);

		let call = Call::claim_account {
			account: truncated_account,
			asset_id: DOT,
			signature,
		};

		// Act & Assert
		assert_storage_noop!({
			let res = EVMAccounts::validate_unsigned(TransactionSource::Local, &call);
			assert_noop!(res, TransactionValidityError::Invalid(InvalidTransaction::Call));
		});
	});
}
