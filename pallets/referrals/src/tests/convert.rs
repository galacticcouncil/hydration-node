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
fn convert_should_convert_all_asset_entries() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_trade_activity(vec![(BOB, DAI, 1_000_000_000_000_000_000), (BOB, DOT, 10_000_000_000)])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
			// Assert
			let length = Accrued::<Test>::iter_prefix(DAI).count();
			assert_eq!(length, 0);
			let length = Accrued::<Test>::iter_prefix(DOT).count();
			assert_eq!(length, 1);
		});
}

#[test]
fn convert_should_convert_all_asset_amount_when_successful() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_trade_activity(vec![(BOB, DAI, 1_000_000_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
			// Assert
			let balance = Tokens::free_balance(DAI, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 0);
			let balance = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 1_000_000_000_000);
		});
}

#[test]
fn convert_should_update_account_rewards() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_trade_activity(vec![(BOB, DAI, 1_000_000_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
			// Assert
			let rewards = Rewards::<Test>::get(&BOB);
			assert_eq!(rewards, 1_000_000_000_000);
		});
}
#[test]
fn convert_should_emit_event_when_successful() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_trade_activity(vec![(BOB, DAI, 1_000_000_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
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

#[test]
fn convert_should_distribute_native_amount_correct_when_there_is_multiple_entries() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_trade_activity(vec![
			(BOB, DAI, 1_000_000_000_000_000_000),
			(CHARLIE, DAI, 2_000_000_000_000_000_000),
		])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
			// Assert
			let rewards = Rewards::<Test>::get(&BOB);
			assert_eq!(rewards, 1_000_000_000_000);

			let rewards = Rewards::<Test>::get(&CHARLIE);
			assert_eq!(rewards, 2_000_000_000_000);
		});
}

#[test]
fn convert_should_transfer_leftovers_to_registration_fee_beneficiary() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_333_333_333_333, 1_333_333_333_333_333_333),
		)
		.with_trade_activity(vec![
			(BOB, DAI, 1_333_333_333_333_333_333),
			(CHARLIE, DAI, 2_333_333_333_333_333_333),
		])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI,));
			// Assert
			let rewards = Rewards::<Test>::get(&BOB);
			assert_eq!(rewards, 1_333_333_333_333);

			let rewards = Rewards::<Test>::get(&CHARLIE);
			assert_eq!(rewards, 2_333_333_333_332);

			let treasury = Tokens::free_balance(HDX, &TREASURY);
			assert_eq!(treasury, 1);
		});
}

#[test]
fn convert_should_have_correct_reward_balance() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_333_333_333_333, 1_333_333_333_333_333_333),
		)
		.with_trade_activity(vec![
			(BOB, DAI, 1_333_333_333_333_333_333),
			(CHARLIE, DAI, 2_333_333_333_333_333_333),
		])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI));
			// Assert
			let distributed = Rewards::<Test>::iter_values().sum::<Balance>();
			let reserve = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(reserve, distributed);
		});
}
