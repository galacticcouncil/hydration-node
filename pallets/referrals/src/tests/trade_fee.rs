use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn process_trade_should_increased_accrued() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_rewards(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(ALICE),
				HDX,
				DAI,
				1_000_000_000_000,
			));
			// Assert
			let rewards = Accrued::<Test>::get(&HDX, &ALICE);
			assert_eq!(rewards, 5_000_000_000_000_000);
		});
}

#[test]
fn process_trade_should_not_increase_accrued_when_trader_does_not_have_linked_account() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_rewards(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
			RuntimeOrigin::signed(ALICE),
			b"BALLS69".to_vec(),
			ALICE
			));
			// Assert
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(ALICE),
				HDX,
				DAI,
				1_000_000_000_000,
			));
			let rewards = Accrued::<Test>::get(&HDX, &ALICE);
			assert_eq!(rewards, 0);
		});
}
