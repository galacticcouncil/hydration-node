use super::*;

#[test]
fn exit_farms_should_work_for_all_joined_farms() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX, 1_000_000 * ONE),
			(ALICE, BSX_KSM_SHARE_ID, 100 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_global_farm(
			100_000 * ONE,
			10_000,
			12,
			BSX,
			BSX,
			CHARLIE,
			Perquintill::from_percent(2),
			ONE,
			One::one(),
		)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(CHARLIE, 2, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let global_farm_2 = 2;
			let farm_entries = vec![(BSX_FARM, 3), (global_farm_2, 4)];
			let deposit_id = 1;

			assert_ok!(LiquidityMining::join_farms(
				Origin::signed(ALICE),
				farm_entries.clone().try_into().unwrap(),
				BSX_KSM_ASSET_PAIR,
				deposited_amount,
			));
			// Check if LP tokens are locked
			assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				deposited_amount
			);
			// Check if NFT is minted
			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			assert_eq!(nft_owner, ALICE);

			set_block_number(13_420_000);

			//Act
			let yield_farms = vec![3, 4];
			assert_ok!(LiquidityMining::exit_farms(
				Origin::signed(ALICE),
				deposit_id,
				BSX_KSM_ASSET_PAIR,
				yield_farms.try_into().unwrap(),
			));

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::RewardClaimed {
						global_farm_id: 1,
						yield_farm_id: 3,
						who: ALICE,
						claimed: 20_000_000 * ONE,
						reward_currency: BSX,
						deposit_id,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 1,
						yield_farm_id: 3,
						who: ALICE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: deposited_amount,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::RewardClaimed {
						global_farm_id: 2,
						yield_farm_id: 4,
						who: ALICE,
						claimed: 20_000_000 * ONE,
						reward_currency: BSX,
						deposit_id,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::SharesWithdrawn {
						global_farm_id: 2,
						yield_farm_id: 4,
						who: ALICE,
						lp_token: BSX_KSM_SHARE_ID,
						amount: deposited_amount,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);

			pretty_assertions::assert_eq!(
				has_event(
					crate::Event::DepositDestroyed {
						who: ALICE,
						deposit_id: 1,
					}
					.into(),
				),
				true
			);
		});
}

#[test]
fn exit_farms_should_fail_with_no_origin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX, 1_000_000 * ONE),
			(ALICE, BSX_KSM_SHARE_ID, 100 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_global_farm(
			100_000 * ONE,
			10_000,
			12,
			BSX,
			BSX,
			CHARLIE,
			Perquintill::from_percent(2),
			ONE,
			One::one(),
		)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(CHARLIE, 2, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let global_farm_2 = 2;
			let farm_entries = vec![(BSX_FARM, 3), (global_farm_2, 4)];
			let deposit_id = 1;

			assert_ok!(LiquidityMining::join_farms(
				Origin::signed(ALICE),
				farm_entries.clone().try_into().unwrap(),
				BSX_KSM_ASSET_PAIR,
				deposited_amount,
			));
			// Check if LP tokens are locked
			assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				deposited_amount
			);
			// Check if NFT is minted
			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			assert_eq!(nft_owner, ALICE);

			set_block_number(13_420_000);

			//Act and assert
			let yield_farms = vec![3, 4];
			assert_noop!(
				LiquidityMining::exit_farms(Origin::none(), deposit_id, BSX_KSM_ASSET_PAIR, yield_farms.try_into().unwrap(),),
				BadOrigin
			);
		});
}

#[test]
fn exit_farms_should_fail_with_non_nft_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(CHARLIE, BSX, 1_000_000 * ONE),
			(ALICE, BSX_KSM_SHARE_ID, 100 * ONE),
		])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_global_farm(
			500_000 * ONE,
			20_000,
			10,
			BSX,
			BSX,
			BOB,
			Perquintill::from_percent(1),
			ONE,
			One::one(),
		)
		.with_global_farm(
			100_000 * ONE,
			10_000,
			12,
			BSX,
			BSX,
			CHARLIE,
			Perquintill::from_percent(2),
			ONE,
			One::one(),
		)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(CHARLIE, 2, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let global_farm_2 = 2;
			let farm_entries = vec![(BSX_FARM, 3), (global_farm_2, 4)];
			let deposit_id = 1;

			assert_ok!(LiquidityMining::join_farms(
				Origin::signed(ALICE),
				farm_entries.clone().try_into().unwrap(),
				BSX_KSM_ASSET_PAIR,
				deposited_amount,
			));
			// Check if LP tokens are locked
			assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				deposited_amount
			);
			// Check if NFT is minted
			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			assert_eq!(nft_owner, ALICE);

			set_block_number(13_420_000);

			//Act and assert
			let yield_farms = vec![3, 4];
			assert_noop!(
				LiquidityMining::exit_farms(Origin::signed(BOB), deposit_id, BSX_KSM_ASSET_PAIR,  yield_farms.try_into().unwrap()),
				Error::<Test>::NotDepositOwner
			);
		});
}
