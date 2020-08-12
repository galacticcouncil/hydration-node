use crate::{Error, mock::*};
use frame_support::{assert_ok, assert_noop};

#[test]
fn mints() {
	new_test_ext().execute_with(|| {
		assert_ok!(TemplateModule::mint(Origin::signed(1), 1, 1));
	});
}
