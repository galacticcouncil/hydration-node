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
		// Account 42 signs a msg:
		// "I hereby claim all my xHDX tokens to wallet:2a00000000000000"
		let signature = hex!["5b2b46b0162f4b4431f154c4b9fc5ba923690b98b0c2063720799da54cb35a354304102ede62977ba556f0b03e67710522d4b7523547c62fcdc5acea59c99aa41b"];

		assert_noop!(Claims::claim(Origin::signed(142), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);
		assert_ok!(Claims::claim(Origin::signed(42), EcdsaSignature(signature)));
		assert_noop!(Claims::claim(Origin::signed(42), EcdsaSignature(signature)), Error::<Test>::NoClaimOrAlreadyClaimed);
	})
}
