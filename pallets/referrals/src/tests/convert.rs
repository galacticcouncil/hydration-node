use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn convert_should_fail_when_amount_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		assert_noop!(
			Referrals::convert(RuntimeOrigin::signed(ALICE), DAI),
			Error::<Test>::ZeroAmount
		);
	});
}

#[test]
fn convert_should_convert_all_asset_amount_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI));
			// Assert
			let balance = Tokens::free_balance(DAI, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 0);
			let balance = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 1_000_000_000_000);
		});
}

#[test]
fn convert_should_emit_event_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI));
			// Assert
			expect_events(vec![Event::Converted {
				from: DAI,
				to: RewardAsset::get(),
				amount: 1_000_000_000_000_000_000,
				received: 1_000_000_000_000,
			}
			.into()]);
		});
}
