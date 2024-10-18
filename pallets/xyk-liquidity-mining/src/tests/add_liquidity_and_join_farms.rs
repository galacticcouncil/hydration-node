use super::*;

#[test]
fn add_liquidity_and_join_farms_should_work() {
	let share_amount = 100 * ONE;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, BSX, 1_000_000 * ONE),
			(ALICE, BSX_KSM_SHARE_ID, share_amount),
		])
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
			let farm_entries = vec![(BSX_FARM, 2), (BSX_FARM, 3), (BSX_FARM, 4)];

			// Act
			assert_ok!(LiquidityMining::add_liquidity_and_join_farms(
				Origin::signed(ALICE),
				BSX,
				KSM,
				10 * ONE,
				Balance::MAX,
				farm_entries.try_into().unwrap(),
			));

			// Assert

			// Check if LP tokens are locked
			assert_eq!(
				Tokens::total_balance(BSX_KSM_SHARE_ID, &LiquidityMining::account_id()),
				share_amount + LOCKED_XYK_ADD_LIQUIDITY_XYK_SHARE_AMOUNT
			);

			// Check if NFT is minted
			let nft_owner: AccountId = DummyNFT::owner(&LM_NFT_COLLECTION, &1).unwrap();
			assert_eq!(nft_owner, ALICE);

			expect_events(vec![
				crate::Event::SharesDeposited {
					global_farm_id: 1,
					yield_farm_id: 2,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: share_amount,
					deposit_id: 1,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: 1,
					yield_farm_id: 3,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: share_amount,
					deposit_id: 1,
				}
				.into(),
				crate::Event::SharesRedeposited {
					global_farm_id: 1,
					yield_farm_id: 4,
					who: ALICE,
					lp_token: BSX_KSM_SHARE_ID,
					amount: share_amount,
					deposit_id: 1,
				}
				.into(),
			]);
		});
}

#[test]
fn add_liquidity_and_join_farms_should_fail_when_origin_is_not_signed() {
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
			let farm_entries = vec![(BSX_FARM, 2), (BSX_FARM, 3), (BSX_FARM, 4)];

			// Act
			assert_noop!(
				LiquidityMining::add_liquidity_and_join_farms(
					Origin::none(),
					BSX,
					KSM,
					10 * ONE,
					Balance::MAX,
					farm_entries.try_into().unwrap(),
				),
				BadOrigin
			);
		});
}

#[test]
fn add_liquidity_and_join_farms_should_fail_when_no_yield_farm_specified() {
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
			let farm_entries = vec![]; // No yield farm ids specified

			// Act and Assert
			assert_noop!(
				LiquidityMining::add_liquidity_and_join_farms(
					Origin::signed(ALICE),
					BSX,
					KSM,
					10 * ONE,
					Balance::MAX,
					farm_entries.try_into().unwrap(),
				),
				Error::<Test>::NoFarmsSpecified
			);
		});
}
