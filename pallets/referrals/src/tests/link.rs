use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn link_code_should_work_when_code_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE,
		));
		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(ALICE), b"BALLS69".to_vec()));
	});
}

#[test]
fn link_code_should_fail_when_code_is_too_long() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(ALICE), b"TOOMANYBALLS69".to_vec(),),
			Error::<Test>::InvalidCode
		);
	});
}

#[test]
fn link_code_should_fail_when_code_does_not_exist() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(ALICE), b"BALLS69".to_vec(),),
			Error::<Test>::InvalidCode
		);
	});
}

#[test]
fn link_code_should_link_correctly_when_code_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE
		));

		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));

		// ASSERT
		let entry = Pallet::<Test>::linked_referral_account::<AccountId>(BOB);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn link_code_should_link_correctly_when_code_is_lowercase() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE
		));

		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"balls69".to_vec()));

		// ASSERT
		let entry = Pallet::<Test>::linked_referral_account::<AccountId>(BOB);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn link_code_should_fail_when_account_is_already_linked() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE
		));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));

		// ACT
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()),
			Error::<Test>::AlreadyLinked
		);
	});
}

#[test]
fn link_code_should_emit_event_when_successful() {
	ExtBuilder::default().build().execute_with(|| {
		//ARRANGE
		let code = b"BALLS69".to_vec();
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			ALICE
		));
		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code.clone()));
		// ASSERT
		expect_events(vec![Event::CodeLinked {
			account: BOB,
			code: code.try_into().unwrap(),
			referral_account: ALICE,
		}
		.into()]);
	});
}
