use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo, PoolState};
use frame_support::assert_ok;
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn add_liquidity_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount)],
			));
			let (p, state) = last_liquidity_changed_hook_state().unwrap();
			assert_eq!(p, pool_id);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425995641788588073263117, 52033213790329, 119135337044269],
					after: vec![52425997641788588073263117, 52033213790329, 119135337044269],
					delta: vec![amount, 0, 0],
					issuance_before: 217677687130232134753136480,
					issuance_after: 217677689066649574177561306,
					share_price: (
						274526994944285284851115313033649172557,
						267281151408777762099703170812400231060
					),
				}
			)
		});
}

#[test]
fn add_liquidity_shares_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount * 2,
			));
			let (p, state) = last_liquidity_changed_hook_state().unwrap();
			assert_eq!(p, pool_id);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425995641788588073263117, 52033213790329, 119135337044269],
					after: vec![52425997653270608839100704, 52033213790329, 119135337044269],
					delta: vec![2011482020765837587, 0, 0],
					issuance_before: 217677687130232134753136480,
					issuance_after: 217677689077829756155082331,
					share_price: (
						274526995018510109258863123533517232439,
						267281151481037663623537458420636124649
					),
				}
			)
		});
}

#[test]
fn removing_liquidity_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount * 2,
			));
			// ACT
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				desired_shares,
				1_900_000_000_000_000_000,
			));

			let (p, state) = last_liquidity_changed_hook_state().unwrap();
			assert_eq!(p, pool_id);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425997653270608839100704, 52033213790329, 119135337044269],
					after: vec![52425995664752629398964188, 52033213790329, 119135337044269],
					delta: vec![1988517979440136516, 0, 0],
					issuance_before: 217677689077829756155082331,
					issuance_after: 217677687130232134753136480,
					share_price: (
						274526982163856750887334622910231248425,
						267281138952739257321645731804954633465
					),
				}
			)
		});
}

#[test]
fn withdraw_asset_amount_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount * 2,
			));
			// ACT
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				1_000_000_000_000_000_000,
				desired_shares,
			));

			let (p, state) = last_liquidity_changed_hook_state().unwrap();
			assert_eq!(p, pool_id);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425997653270608839100704, 52033213790329, 119135337044269],
					after: vec![52425996653270608839100704, 52033213790329, 119135337044269],
					delta: vec![1000000000000000000, 0, 0],
					issuance_before: 217677689077829756155082331,
					issuance_after: 217677688098441828103029128,
					share_price: (
						274526988554070995651320692108733882221,
						267281145180759683324218172695500992618
					),
				}
			)
		});
}

#[test]
fn sell_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			// ACT
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				1_000_000_000_000_000_000,
				0,
			));

			let (p, ai, ao, state) = last_trade_hook_state().unwrap();
			assert_eq!(p, pool_id);
			assert_eq!(ai, asset_a);
			assert_eq!(ao, asset_b);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425995641788588073263117, 52033213790329, 119135337044269],
					after: vec![52425996641788588073263117, 52033212800336, 119135337044269],
					delta: vec![1000000000000000000, 989993, 0],
					issuance_before: 217677687130232134753136480,
					issuance_after: 217677687130232134753136480,
					share_price: (
						274526987264157543280488507445843962471,
						267281143933421736083636966541416790180
					),
				}
			)
		});
}

#[test]
fn buy_should_provide_correct_values_in_the_hook() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(1),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			// ACT
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				1_000_000,
				1_100_000_000_000_000_000,
			));

			let (p, ai, ao, state) = last_trade_hook_state().unwrap();
			assert_eq!(p, pool_id);
			assert_eq!(ai, asset_a);
			assert_eq!(ao, asset_b);

			assert_eq!(
				state,
				PoolState {
					assets: vec![asset_a, asset_b, asset_c],
					before: vec![52425995641788588073263117, 52033213790329, 119135337044269],
					after: vec![52425996651795484690763781, 52033212790329, 119135337044269],
					delta: vec![1010006896617500664, 1_000_000, 0],
					issuance_before: 217677687130232134753136480,
					issuance_after: 217677687130232134753136480,
					share_price: (
						274526987316558150926868429559029605460,
						267281143984434362175021631530789792661
					),
				}
			)
		});
}
