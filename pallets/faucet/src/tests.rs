use super::*;
use crate::mock::{Faucet, Currency, ExtBuilder, Origin, ALICE, HDX};
use frame_support::{assert_ok};

#[test]
fn mints() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Faucet::mint(Origin::signed(ALICE), HDX, 1000));
		assert_eq!(Currency::free_balance(HDX, &ALICE), 2000);
	});
}
