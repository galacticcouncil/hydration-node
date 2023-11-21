use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn claim_rewards_should_work_when_amount_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
	});
}

#[test]
fn claim_rewards_should_transfer_rewards() {
	ExtBuilder::default()
		.with_rewards(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let reserve = Tokens::free_balance(HDX, &BOB);
			assert_eq!(reserve, 1_000_000_000_000);

			let reserve = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(reserve, 0);
		});
}

#[test]
fn claim_rewards_should_reset_rewards_amount() {
	ExtBuilder::default()
		.with_rewards(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let rewards = Rewards::<Test>::get(&BOB);
			assert_eq!(rewards, 0);
		});
}

#[test]
fn claim_rewards_should_emit_event_when_successful() {
	ExtBuilder::default()
		.with_rewards(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			expect_events(vec![Event::Claimed {
				who: BOB,
				amount: 1_000_000_000_000,
			}
			.into()]);
		});
}
