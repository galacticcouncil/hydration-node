use super::*;

#[test]
fn join_farms_should_work_with_multiple_farm_pairs() {
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(CHARLIE, 2, One::one(), None, BSX_KSM_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let global_farm_2 = 2;
			let farm_entries = vec![(BSX_FARM, 3), (BSX_FARM, 4), (BSX_FARM, 5), (global_farm_2, 6)];

			// Act
			assert_ok!(LiquidityMining::join_farms(
				Origin::signed(ALICE),
				farm_entries.try_into().unwrap(),
				BSX_KSM_ASSET_PAIR,
				deposited_amount,
			));

			// Assert

			// Check if LP tokens are locked
			assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				deposited_amount
			);

			// Check if NFT is minted
			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			assert_eq!(nft_owner, ALICE);

			expect_events(vec![
				crate::Event::SharesDeposited {
					global_farm_id: BSX_FARM,
					yield_farm_id: 3,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: deposited_amount,
					deposit_id: 1,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: BSX_FARM,
					yield_farm_id: 4,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: deposited_amount,
					deposit_id: 1,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: BSX_FARM,
					yield_farm_id: 5,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: deposited_amount,
					deposit_id: 1,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: global_farm_2,
					yield_farm_id: 6,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: deposited_amount,
					deposit_id: 1,
				}
				.into(),
			]);
		});
}

#[test]
fn join_farms_should_fail_when_origin_is_not_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 100 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_ACA_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let farm_entries = vec![(BSX_FARM, 2), (BSX_FARM, 3), (BSX_FARM, 4)];

			// Act
			assert_noop!(
				LiquidityMining::join_farms(
					Origin::none(),
					farm_entries.try_into().unwrap(),
					BSX_KSM_ASSET_PAIR,
					deposited_amount,
				),
				BadOrigin
			);
		});
}

#[test]
fn join_farms_should_fail_when_no_yield_farm_specified() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 100 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_ACA_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = 50 * ONE;
			let farm_entries = vec![]; // No yield farm ids specified

			// Act and Assert
			assert_noop!(
				LiquidityMining::join_farms(
					Origin::signed(ALICE),
					farm_entries.try_into().unwrap(),
					BSX_KSM_ASSET_PAIR,
					deposited_amount,
				),
				Error::<Test>::NoFarmsSpecified
			);
		});
}

#[test]
fn join_farms_should_fail_when_not_enough_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, BSX, 1_000_000 * ONE), (ALICE, BSX_KSM_SHARE_ID, 100 * ONE)])
		.with_amm_pool(BSX_KSM_AMM, BSX_KSM_SHARE_ID, BSX_KSM_ASSET_PAIR)
		.with_amm_pool(BSX_ACA_AMM, BSX_ACA_SHARE_ID, BSX_ACA_ASSET_PAIR)
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
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_KSM_ASSET_PAIR)
		.with_yield_farm(BOB, 1, One::one(), None, BSX_ACA_ASSET_PAIR)
		.build()
		.execute_with(|| {
			set_block_number(1_800);
			let deposited_amount = Balance::MAX;
			let farm_entries = vec![(BSX_FARM, 2), (BSX_FARM, 3), (BSX_FARM, 4)];

			// Act and assert
			assert_noop!(
				LiquidityMining::join_farms(
					Origin::signed(ALICE),
					farm_entries.try_into().unwrap(),
					BSX_KSM_ASSET_PAIR,
					deposited_amount,
				),
				Error::<Test>::InsufficientXykSharesBalance
			);
		});
}
