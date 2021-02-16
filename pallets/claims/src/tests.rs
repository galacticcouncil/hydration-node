use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;

fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

#[test]
fn claiming_works() {
	new_test_ext().execute_with(|| {
		// Alice (account id = 42) signs a msg:
		// "I hereby claim all my xHDX tokens to wallet:2a00000000000000"
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		assert_eq!(Currency::free_balance(0, &ALICE), 0);
		assert_eq!(Currency::free_balance(0, &BOB), 0);

		assert_noop!(ClaimsModule::claim(Origin::signed(BOB), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);

		assert_ok!(ClaimsModule::claim(Origin::signed(ALICE), EcdsaSignature(signature)));
		assert_eq!(Currency::free_balance(0, &ALICE), 50_000);
		assert_noop!(ClaimsModule::claim(Origin::signed(ALICE), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);

		assert_eq!(Currency::free_balance(0, &ALICE), 50_000);
		assert_eq!(Currency::free_balance(0, &BOB), 0);
	})
}

#[test]
fn invalid_signature() {
	new_test_ext().execute_with(|| {
		let invalid_signature = hex!["a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"];
		assert_noop!(ClaimsModule::claim(Origin::signed(ALICE), EcdsaSignature(invalid_signature)), Error::<Test>::InvalidEthereumSignature);
	})
}
