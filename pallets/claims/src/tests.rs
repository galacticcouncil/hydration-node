// This file is part of HydraDX.
// Copyright (C) 2020-2021  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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

use crate::mock::*;
use crate::{
	error_to_invalid, Claims, EcdsaSignature, Error, EthereumAddress, SignedExtension, ValidTransaction, ValidateClaim,
};
use frame_support::dispatch::DispatchInfo;
use frame_support::{assert_err, assert_noop, assert_ok};
use hex_literal::hex;
use sp_std::marker::PhantomData;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder.build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

#[test]
fn claiming_works() {
	new_test_ext().execute_with(|| {
		// Alice (account id = 42) signs a msg:
		// "I hereby claim all my xHDX tokens to wallet:2a00000000000000"
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		let alice_initial_balance = Balances::free_balance(ALICE);

		// Signature not consistent with origin
		assert_noop!(ClaimsPallet::claim(RuntimeOrigin::signed(BOB), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);

		assert_ok!(ClaimsPallet::claim(RuntimeOrigin::signed(ALICE), EcdsaSignature(signature)));

		assert_eq!(Balances::free_balance(ALICE), alice_initial_balance + CLAIM_AMOUNT);
	})
}

#[test]
fn invalid_signature_fail() {
	new_test_ext().execute_with(|| {
		let invalid_signature = hex!["a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"];
		assert_noop!(ClaimsPallet::claim(RuntimeOrigin::signed(ALICE), EcdsaSignature(invalid_signature)), Error::<Test>::InvalidEthereumSignature);
	})
}

#[test]
fn claim_cant_overflow() {
	new_test_ext().execute_with(|| {
		// Charlie (account id = 44) signs a msg:
		// "I hereby claim all my HDX tokens to wallet:2c00000000000000"
		let signature = hex!["c8da07e0f0946c10ad9bf7fe6aafbea11a6e4a8b7ce2f5fc506dd2e024a2c56442d3c35cd8428238ac84feef02c1a6d55ccfd216e7e3d64a897ef364fc6e8ff61b"];
		let charlie_eth_addr = EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]);

		assert_eq!(Claims::<Test>::get(charlie_eth_addr), CLAIM_AMOUNT);
		assert_eq!(Balances::free_balance(CHARLIE), primitives::Balance::MAX - 2);

		assert_noop!(ClaimsPallet::claim(RuntimeOrigin::signed(CHARLIE), EcdsaSignature(signature)), Error::<Test>::BalanceOverflow);

		assert_eq!(Claims::<Test>::get(charlie_eth_addr), CLAIM_AMOUNT);
		assert_eq!(Balances::free_balance(CHARLIE), primitives::Balance::MAX - 2);
	})
}

#[test]
fn zeroize_claimed_balance_works() {
	new_test_ext().execute_with(|| {
		// Alice (account id = 42) signs a msg:
		// "I hereby claim all my HDX tokens to wallet:2a00000000000000"
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];
		let alice_eth_addr = EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]);

		assert_eq!(Claims::<Test>::get(alice_eth_addr), CLAIM_AMOUNT);
		assert_ok!(ClaimsPallet::claim(RuntimeOrigin::signed(ALICE), EcdsaSignature(signature)));
		assert_eq!(Claims::<Test>::get(alice_eth_addr), 0);
	})
}

#[test]
fn double_claim_fail() {
	new_test_ext().execute_with(|| {
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		assert_ok!(ClaimsPallet::claim(RuntimeOrigin::signed(ALICE), EcdsaSignature(signature)));
		assert_noop!(ClaimsPallet::claim(RuntimeOrigin::signed(ALICE), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);
	})
}

#[test]
fn unsigned_claim_fail() {
	new_test_ext().execute_with(|| {
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];
		assert_err!(
			ClaimsPallet::claim(RuntimeOrigin::none(), EcdsaSignature(signature)),
			sp_runtime::traits::BadOrigin,
		);
	});
}

#[test]
fn signed_extention_success() {
	new_test_ext().execute_with(|| {
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		let call: &<Test as frame_system::Config>::RuntimeCall = &RuntimeCall::ClaimsPallet(crate::Call::claim{ethereum_signature: EcdsaSignature(signature)});
		let info = DispatchInfo::default();

		assert_eq!(
			ValidateClaim::<Test>(PhantomData).validate(&ALICE, call, &info, 150),
			Ok(ValidTransaction::default())
		);
	});
}

#[test]
fn signed_extention_invalid_sig() {
	new_test_ext().execute_with(|| {
		let invalid_signature = hex!["a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"];

		let call: &<Test as frame_system::Config>::RuntimeCall = &RuntimeCall::ClaimsPallet(crate::Call::claim{ethereum_signature: EcdsaSignature(invalid_signature)});
		let info = DispatchInfo::default();

		assert_eq!(
			ValidateClaim::<Test>(PhantomData).validate(&ALICE, call, &info, 150),
			error_to_invalid(Error::<Test>::InvalidEthereumSignature).into()
		);
	});
}

#[test]
fn signed_extention_no_claim_error() {
	new_test_ext().execute_with(|| {
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		let call: &<Test as frame_system::Config>::RuntimeCall = &RuntimeCall::ClaimsPallet(crate::Call::claim{ethereum_signature: EcdsaSignature(signature)});
		let info = DispatchInfo::default();

		assert_eq!(
			ValidateClaim::<Test>(PhantomData).validate(&BOB, call, &info, 150),
			error_to_invalid(Error::<Test>::NoClaimOrAlreadyClaimed).into()
		);
	});
}
