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
				Level::Tier0,
				FeeDistribution {
					referrer: Permill::from_percent(1),
					trader: Permill::from_percent(2),
					external: Permill::from_percent(2),
				}
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
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(1),
				trader: Permill::from_percent(2),
				external: Permill::from_percent(3),
			}
		));
		let d = AssetRewards::<Test>::get(DAI, Level::Tier0);
		assert_eq!(
			d,
			Some(FeeDistribution {
				referrer: Permill::from_percent(1),
				trader: Permill::from_percent(2),
				external: Permill::from_percent(3),
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
				Level::Tier0,
				FeeDistribution {
					referrer: Permill::from_percent(60),
					trader: Permill::from_percent(40),
					external: Permill::from_percent(10),
				}
			),
			Error::<Test>::IncorrectRewardPercentage
		);
	});
}

#[test]
fn setting_asset_tier_should_emit_event() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::set_reward_percentage(
			RuntimeOrigin::root(),
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(1),
				trader: Permill::from_percent(2),
				external: Permill::from_percent(3),
			}
		));
		expect_events(vec![Event::AssetRewardsUpdated {
			asset_id: DAI,
			level: Level::Tier0,
			rewards: FeeDistribution {
				referrer: Permill::from_percent(1),
				trader: Permill::from_percent(2),
				external: Permill::from_percent(3),
			},
		}
		.into()]);
	});
}
