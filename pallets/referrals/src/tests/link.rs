use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn link_code_should_work_when_code_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
	});
}

#[test]
fn link_code_should_fail_when_code_does_not_exist() {
	ExtBuilder::default().build().execute_with(|| {
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(ALICE), code),
			Error::<Test>::InvalidCode
		);
	});
}

#[test]
fn link_code_should_link_correctly_when_code_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));

		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

		// ASSERT
		let entry = Pallet::<Test>::linked_referral_account::<AccountId>(BOB);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn link_code_should_fail_when_linking_to_same_acccount() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));

		// ACT
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(ALICE), code),
			Error::<Test>::LinkNotAllowed
		);
	});
}

#[test]
fn link_code_should_link_correctly_when_code_is_lowercase() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code,));

		// ACT
		let code: ReferralCode<<Test as Config>::CodeLength> = b"balls69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

		// ASSERT
		let entry = Pallet::<Test>::linked_referral_account::<AccountId>(BOB);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn link_code_should_fail_when_account_is_already_linked() {
	ExtBuilder::default().build().execute_with(|| {
		// ARRANGE
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code.clone()));

		// ACT
		assert_noop!(
			Referrals::link_code(RuntimeOrigin::signed(BOB), code),
			Error::<Test>::AlreadyLinked
		);
	});
}

#[test]
fn link_code_should_emit_event_when_successful() {
	ExtBuilder::default().build().execute_with(|| {
		//ARRANGE
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
		// ACT
		assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code.clone()));
		// ASSERT
		expect_events(vec![Event::CodeLinked {
			account: BOB,
			code,
			referral_account: ALICE,
		}
		.into()]);
	});
}
