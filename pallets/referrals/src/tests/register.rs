use crate::tests::*;
use pretty_assertions::assert_eq;
use sp_runtime::traits::Zero;

#[test]
fn register_code_should_work_when_code_is_max_length() {
	ExtBuilder::default().build().execute_with(|| {
		let code: ReferralCode<<Test as Config>::CodeLength> = vec![b'x'; <Test as Config>::CodeLength::get() as usize]
			.try_into()
			.unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code,));
	});
}

#[test]
fn register_code_should_work_when_code_is_min_length() {
	ExtBuilder::default().build().execute_with(|| {
		let code: ReferralCode<<Test as Config>::CodeLength> =
			vec![b'x'; <Test as Config>::MinCodeLength::get() as usize]
				.try_into()
				.unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code,));
	});
}

#[test]
fn register_code_should_fail_when_code_is_too_short() {
	ExtBuilder::default().build().execute_with(|| {
		for len in 0..<Test as Config>::MinCodeLength::get() {
			let code: ReferralCode<<Test as Config>::CodeLength> = vec![b'x'; len as usize].try_into().unwrap();
			assert_noop!(
				Referrals::register_code(RuntimeOrigin::signed(ALICE), code),
				Error::<Test>::TooShort
			);
		}
	});
}

#[test]
fn register_code_should_fail_when_code_already_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = vec![b'x'; <Test as Config>::CodeLength::get() as usize]
			.try_into()
			.unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
		// Act
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(BOB), code),
			Error::<Test>::AlreadyExists
		);
	});
}

#[test]
fn register_code_should_fail_when_code_is_lowercase_and_already_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code,));
		// Act
		let code: ReferralCode<<Test as Config>::CodeLength> = b"balls69".to_vec().try_into().unwrap();
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(BOB), code),
			Error::<Test>::AlreadyExists
		);
	});
}

#[test]
fn register_code_should_fail_when_code_contains_invalid_char() {
	ExtBuilder::default().build().execute_with(|| {
		let code: ReferralCode<<Test as Config>::CodeLength> = b"ABCD?".to_vec().try_into().unwrap();
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), code),
			Error::<Test>::InvalidCharacter
		);
	});
}

#[test]
fn register_code_should_store_account_mapping_to_code_correctly() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		// Act
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
		// Assert
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(code);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn register_code_should_convert_to_upper_case_when_code_is_lower_case() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"balls69".to_vec().try_into().unwrap();
		// Act
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
		// Assert
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(code.clone());
		assert_eq!(entry, None);
		let normalized = Pallet::<Test>::normalize_code(code);
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(normalized);
		assert_eq!(entry, Some(ALICE));
	});
}

#[test]
fn register_code_should_emit_event_when_successful() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		// Act
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
		// Assert
		expect_events(vec![Event::CodeRegistered { code, account: ALICE }.into()]);
	});
}

#[test]
fn signer_should_pay_the_registration_fee() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		// Act
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code));
		// Assert
		let (fee_asset, amount, beneficiary) = RegistrationFee::get();
		assert_balance!(ALICE, fee_asset, INITIAL_ALICE_BALANCE - amount);
		assert_balance!(beneficiary, fee_asset, amount);
	});
}

#[test]
fn singer_should_set_default_level_for_referrer() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
		// Act
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code));
		// Assert
		let entry = Pallet::<Test>::referrer_level(ALICE);
		assert_eq!(entry, Some((Level::default(), Balance::zero())));
	});
}

#[test]
fn register_code_should_fail_when_account_has_already_code_registered() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code: ReferralCode<<Test as Config>::CodeLength> = b"FIRST".to_vec().try_into().unwrap();
		assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code));
		let code: ReferralCode<<Test as Config>::CodeLength> = b"SECOND".to_vec().try_into().unwrap();
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), code),
			Error::<Test>::AlreadyRegistered
		);
	});
}
