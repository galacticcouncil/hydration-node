use crate::tests::*;
use pretty_assertions::assert_eq;
use sp_runtime::traits::Zero;

#[test]
fn register_code_should_work_when_code_is_max_length() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			BOB
		));
	});
}

#[test]
fn register_code_should_work_when_code_is_min_length() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"ABC".to_vec(),
			BOB
		));
	});
}

#[test]
fn register_code_should_fail_when_code_is_too_long() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"TOOMANYBALLS69".to_vec(), BOB),
			Error::<Test>::TooLong
		);
	});
}

#[test]
fn register_code_should_fail_when_code_is_too_short() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"".to_vec(), BOB),
			Error::<Test>::TooShort
		);
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"A".to_vec(), BOB),
			Error::<Test>::TooShort
		);
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"AB".to_vec(), BOB),
			Error::<Test>::TooShort
		);
	});
}

#[test]
fn register_code_should_fail_when_code_already_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			BOB
		));
		// Act
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"BALLS69".to_vec(), BOB),
			Error::<Test>::AlreadyExists
		);
	});
}

#[test]
fn register_code_should_fail_when_code_is_lowercase_and_already_exists() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			BOB
		));
		// Act
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"balls69".to_vec(), BOB),
			Error::<Test>::AlreadyExists
		);
	});
}

#[test]
fn register_code_should_fail_when_code_contains_invalid_char() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::register_code(RuntimeOrigin::signed(ALICE), b"ABC?".to_vec(), BOB),
			Error::<Test>::InvalidCharacter
		);
	});
}

#[test]
fn register_code_should_store_account_mapping_to_code_correctly() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code = b"BALLS69".to_vec();
		// Act
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			BOB
		));
		// Assert
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(code.try_into().unwrap());
		assert_eq!(entry, Some(BOB));
	});
}

#[test]
fn register_code_should_convert_to_upper_case_when_code_is_lower_case() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code = b"balls69".to_vec();
		// Act
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			BOB
		));
		// Assert
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(code.clone().try_into().unwrap());
		assert_eq!(entry, None);
		let normalized = Pallet::<Test>::normalize_code(code.try_into().unwrap());
		let entry = Pallet::<Test>::referral_account::<ReferralCode<CodeLength>>(normalized);
		assert_eq!(entry, Some(BOB));
	});
}

#[test]
fn register_code_should_emit_event_when_successful() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code = b"BALLS69".to_vec();
		// Act
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			BOB
		));
		// Assert
		expect_events(vec![Event::CodeRegistered {
			code: code.try_into().unwrap(),
			account: BOB,
		}
		.into()]);
	});
}

#[test]
fn signer_should_pay_the_registration_fee() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let code = b"BALLS69".to_vec();
		// Act
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			BOB
		));
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
		let code = b"BALLS69".to_vec();
		// Act
		assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			code.clone(),
			BOB
		));
		// Assert
		let entry = Pallet::<Test>::referrer_level(BOB);
		assert_eq!(entry, Some((Level::default(), Balance::zero())));
	});
}
