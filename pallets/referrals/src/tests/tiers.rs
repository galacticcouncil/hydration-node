use crate::tests::*;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn setting_asset_tier_should_fail_when_not_correct_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::set_reward_percentage(
				RuntimeOrigin::signed(BOB),
				DAI,
				Level::Novice,
				Permill::from_percent(1),
				Permill::from_percent(2),
			),
			BadOrigin
		);
	});
}

#[test]
fn setting_asset_tier_should_correctly_update_storage() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::set_reward_percentage(
			RuntimeOrigin::root(),
			DAI,
			Level::Novice,
			Permill::from_percent(1),
			Permill::from_percent(2),
		));
		let d = AssetTier::<Test>::get(DAI, Level::Novice);
		assert_eq!(
			d,
			Some(Tier {
				referrer: Permill::from_percent(1),
				trader: Permill::from_percent(2)
			})
		)
	});
}

#[test]
fn setting_asset_tier_should_fail_when_total_percentage_exceeds_hundred_percent() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Referrals::set_reward_percentage(
				RuntimeOrigin::root(),
				DAI,
				Level::Novice,
				Permill::from_percent(70),
				Permill::from_percent(40),
			),
			Error::<Test>::IncorrectRewardPercentage
		);
	});
}
