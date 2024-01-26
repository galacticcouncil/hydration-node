use super::*;

use mock::Registry;
use pretty_assertions::assert_eq;

#[test]
fn blacklist_add_should_work_when_asset_is_not_blacklisted() {
	ExtBuilder::default()
		.with_assets(vec![
			(
				Some(1),
				Some(b"tkn1".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"tkn2".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				false,
			),
			(
				Some(3),
				Some(b"Tkn3".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
		])
		.build()
		.execute_with(|| {
			//Act
			//NOTE: update origin is set to ensure_signed in tests
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), 1));

			//Assert
			assert_last_event!(Event::<Test>::BlacklistAdded { asset_id: 1 }.into());

			assert_eq!(Registry::blacklists(1), Some(()))
		});
}

#[test]
fn blacklist_add_should_fial_when_asset_is_already_blacklisted() {
	let asset_id: u32 = 1;
	ExtBuilder::default()
		.with_assets(vec![
			(
				Some(asset_id),
				Some(b"tkn1".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"tkn2".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				false,
			),
			(
				Some(3),
				Some(b"Tkn3".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
		])
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), 2));
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), asset_id));

			//Act
			//NOTE: update origin is set to ensure_signed in tests
			assert_noop!(
				Registry::blacklist_add(RuntimeOrigin::signed(ALICE), asset_id),
				Error::<Test>::AssetAlreadyBlacklisted
			);
		});
}

#[test]
fn blacklist_add_should_fail_when_asset_is_not_registered() {
	ExtBuilder::default()
		.with_assets(vec![
			(
				Some(1),
				Some(b"tkn1".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"tkn2".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				false,
			),
			(
				Some(3),
				Some(b"Tkn3".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
		])
		.build()
		.execute_with(|| {
			let not_existing_asset = 112_412_u32;

			//Act
			//NOTE: update origin is set to ensure_signed in tests
			assert_noop!(
				Registry::blacklist_add(RuntimeOrigin::signed(ALICE), not_existing_asset),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn blacklist_remove_should_work_when_asset_is_blacklisted() {
	let asset_id: u32 = 1;
	ExtBuilder::default()
		.with_assets(vec![
			(
				Some(asset_id),
				Some(b"tkn1".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"tkn2".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				false,
			),
			(
				Some(3),
				Some(b"Tkn3".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
		])
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), 3));
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), asset_id));

			//Act
			//NOTE: update origin is set to ensure_signed in tests
			assert_ok!(Registry::blacklist_remove(RuntimeOrigin::signed(ALICE), asset_id),);

			//Assert
			assert_last_event!(Event::<Test>::BlacklistRemoved { asset_id }.into());

			assert_eq!(Registry::blacklists(asset_id), None)
		});
}

#[test]
fn blacklist_remove_should_fail_when_asset_is_not_blacklisted() {
	let asset_id: u32 = 1;
	ExtBuilder::default()
		.with_assets(vec![
			(
				Some(asset_id),
				Some(b"tkn1".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
			(
				Some(2),
				Some(b"tkn2".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				false,
			),
			(
				Some(3),
				Some(b"Tkn3".to_vec().try_into().unwrap()),
				UNIT,
				None,
				None,
				None,
				true,
			),
		])
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), 3));
			assert_ok!(Registry::blacklist_add(RuntimeOrigin::signed(ALICE), 2));

			//Act
			//NOTE: update origin is set to ensure_signed in tests
			assert_noop!(
				Registry::blacklist_remove(RuntimeOrigin::signed(ALICE), asset_id),
				Error::<Test>::AssetNotBlacklisted
			);
		});
}
