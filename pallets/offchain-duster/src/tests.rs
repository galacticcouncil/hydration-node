use crate::mock::{Duster, ExtBuilder, Origin, Tokens, ALICE, DUSTER, TREASURY};
use frame_support::assert_ok;
use orml_traits::MultiCurrency;

#[test]
fn dust_account_works() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);
		});
}
