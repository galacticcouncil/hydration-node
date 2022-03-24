use crate::mock::new_test_ext;

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with(|| {});
}
