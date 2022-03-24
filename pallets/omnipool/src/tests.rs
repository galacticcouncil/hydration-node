use crate::mock::new_test_ext;
use crate::mock::*;
use frame_support::assert_ok;
use sp_runtime::FixedU128;

#[test]
fn add_token_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(Omnipool::add_token(Origin::root(), 100, 100, FixedU128::from_inner(1)));
	});
}
