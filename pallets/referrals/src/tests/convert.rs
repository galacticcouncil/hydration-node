use crate::tests::*;
use frame_support::traits::Hooks;
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
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_assets(vec![DAI])
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
fn convert_should_remove_asset_from_the_asset_list() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_assets(vec![DAI])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI));
			// Assert
			let entry = PendingConversions::<Test>::get(DAI);
			assert_eq!(entry, None)
		});
}

#[test]
fn convert_should_emit_event_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_assets(vec![DAI])
		.build()
		.execute_with(|| {
			// Arrange
			assert_ok!(Referrals::convert(RuntimeOrigin::signed(ALICE), DAI));
			// Assert
			expect_events(vec![Event::Converted {
				from: AssetAmount::new(DAI, 1_000_000_000_000_000_000),
				to: AssetAmount::new(RewardAsset::get(), 1_000_000_000_000),
			}
			.into()]);
		});
}

#[test]
fn on_idle_should_convert_all_asset_amount_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_assets(vec![DAI])
		.build()
		.execute_with(|| {
			// Arrange
			Referrals::on_idle(10, 1_000_000_000_000.into());
			// Assert
			let balance = Tokens::free_balance(DAI, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 0);
			let balance = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 1_000_000_000_000);
			let entry = PendingConversions::<Test>::get(DAI);
			assert_eq!(entry, None)
		});
}

#[test]
fn on_idle_should_remove_asset_from_pending_conversions_when_not_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), DAI, 1_000_000_000_000_000_000)])
		.with_assets(vec![DAI])
		.build()
		.execute_with(|| {
			// Arrange
			// conversion fails, but the asset should be removed from PendingConversions
			Referrals::on_idle(10, 1_000_000_000_000.into());
			// Assert
			let balance = Tokens::free_balance(DAI, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 1_000_000_000_000_000_000);
			let balance = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(balance, 0);
			let entry = PendingConversions::<Test>::get(DAI);
			assert!(entry.is_none())
		});
}
