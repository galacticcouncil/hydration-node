// Tests to be written here

use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn create_asset() {
	new_test_ext().execute_with(|| {
		assert_ok!(AssetRegistryModule::create_asset(b"HDX".to_vec()));
		assert_ok!(AssetRegistryModule::create_asset(b"DOT".to_vec()));
		assert_ok!(AssetRegistryModule::create_asset(b"BTC".to_vec()));

		assert_noop!(
			AssetRegistryModule::create_asset(b"BTC".to_vec()),
			Error::<Test>::IdAlreadyTaken
		);

		assert_eq!(AssetRegistryModule::asset_ids(b"DOT".to_vec()).unwrap(), 1u32);
		assert_eq!(AssetRegistryModule::asset_ids(b"AAA".to_vec()).is_none(), true);
	});
}
