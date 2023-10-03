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
					share_prices: vec![
						(
							244172029011710087403192798282012196353,
							237774431442618702971797451001065834949
						),
						(
							242342653619958367071470709703767207495,
							235994599304938300150180376956359031350
						),
						(
							277434081196253218157257401460680927048,
							270028136753013259345282294182542391590
						),
					],
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
					share_prices: vec![
						(
							244172029077715777840038075781124430196,
							237774431506856955341017889748675045088
						),
						(
							242342653632393129398314631966654546729,
							235994599317056345594289199485815138505
						),
						(
							277434081210488544912619848520794671151,
							270028136766880764563944366606722528411
						),
					],
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
					share_prices: vec![
						(
							244172017646496141931881546776558729267,
							237774420369329812405830865665816325322
						),
						(
							242342651478874253226076804333304539483,
							235994597206079116406485002658840615492
						),
						(
							277434078745138257519264990750332186257,
							270028134351147570076019747632855761869
						),
					],
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
					share_prices: vec![
						(
							244172023329103094899977306627498151895,
							237774425905975301730362762954157554417
						),
						(
							242342652549416310238848287819413499559,
							235994598255509763815801102215445783932
						),
						(
							277434079970695737941378597514810262391,
							270028135552081622489138148719401241513
						),
					],
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
					share_prices: vec![
						(
							244172022182886814719310776904442891040,
							237774424796633280187638760349219887633
						),
						(
							242342646854010494978191820980530694027,
							235994592720130900742488461665268284013
						),
						(
							277434078729099359529053778471903663189,
							270028134351164531322291071024039187311
						),
					],
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
					share_prices: vec![
						(
							244172022229493653972247906659887664828,
							237774424841978111980195760417085965484
						),
						(
							242342646807403395241781075612437030142,
							235994592674786219840491504252753386956
						),
						(
							277434078729099485563276308752488408495,
							270028134351164686159912051067527354862
						),
					],
				}
			)
		});
}
