use super::*;
use crate::mock::BlockNumber;
pub use crate::mock::{
	set_block_number, Currency, ExtBuilder, LBPPallet, RuntimeEvent as TestEvent, RuntimeOrigin as Origin, Test, ALICE,
	BOB, BSX, CHARLIE, ETH, HDX, KUSD,
};
use frame_support::assert_ok;
use hydra_dx_math::types::HYDRA_ONE;
use orml_traits::MultiCurrency;
use rug::ops::Pow;
use rug::Rational;
use sp_runtime::ModuleError;

use proptest::prelude::*;
use proptest::proptest;

fn calc_invariant(x: Balance, y: Balance, w1: u32, w2: u32) -> Rational {
	let x = Rational::from((x, HYDRA_ONE));
	let y = Rational::from((y, HYDRA_ONE));
	let w1 = w1 * 10 / MAX_WEIGHT;
	let w2 = w2 * 10 / MAX_WEIGHT;
	let r1 = x.pow(w1);
	let r2 = y.pow(w2);

	r1 * r2
}

fn invariant(pool_id: u64, asset_a: AssetId, asset_b: AssetId, at: BlockNumber) -> Rational {
	let pool_data = LBPPallet::pool_data(pool_id).unwrap();
	let a_balance = Currency::free_balance(asset_a, &pool_id);
	let b_balance = Currency::free_balance(asset_b, &pool_id);
	let (w1, w2) = LBPPallet::calculate_weights(&pool_data, at).unwrap();
	calc_invariant(a_balance, b_balance, w1, w2)
}

const RESERVE_RANGE: (Balance, Balance) = (10_000, 1_000_000_000);
const TRADE_RANGE: (Balance, Balance) = (1, 2_000);
fn asset_amount() -> impl Strategy<Value = Balance> {
	RESERVE_RANGE.0..RESERVE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	TRADE_RANGE.0..TRADE_RANGE.1
}

fn to_precision(value: Balance, precision: u8) -> Balance {
	value * 10u128.pow(precision as u32)
}

fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

fn weight_ratio() -> impl Strategy<Value = u32> {
	// we can only use simple ratios due to limitations in the invariant calculation
	1u32..10u32
}

fn weights() -> impl Strategy<Value = (LBPWeight, LBPWeight)> {
	weight_ratio().prop_map(|ratio| (ratio * MAX_WEIGHT / 10, (10 - ratio) * MAX_WEIGHT / 10))
}

#[derive(Debug, Copy, Clone)]
struct Assets {
	pub asset_a_amount: u128,
	pub asset_a_decimals: u8,
	pub asset_b_amount: u128,
	pub asset_b_decimals: u8,
}

fn pool_assets() -> impl Strategy<Value = Assets> {
	(decimals(), decimals(), asset_amount(), asset_amount()).prop_map(|(dec_a, dec_b, a_amount, b_amount)| Assets {
		asset_a_amount: to_precision(a_amount, dec_a),
		asset_a_decimals: dec_a,
		asset_b_amount: to_precision(b_amount, dec_b),
		asset_b_decimals: dec_b,
	})
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_accumulated_asset_invariant(
		assets in pool_assets(),
		sell_amount in trade_amount(),
		(weight_a, weight_b) in weights(),
	) {
		let asset_a = 1;
		let asset_b = 2;
		let pool_id: PoolId<Test> = 1002;

		let sell_amount = to_precision(sell_amount, assets.asset_a_decimals);

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(ALICE, asset_a, assets.asset_a_amount + sell_amount),
				(ALICE, asset_b, assets.asset_b_amount + sell_amount),
			])
			.build()
			.execute_with(|| {
				assert_ok!(LBPPallet::create_pool(
					Origin::root(),
					ALICE,
					asset_a,
					assets.asset_a_amount,
					asset_b,
					assets.asset_b_amount,
					weight_a,
					weight_b,
					WeightCurveType::Linear,
					(0, 1),
					CHARLIE,
					0,
				));
				assert_ok!(LBPPallet::update_pool_data(
					Origin::signed(ALICE),
					pool_id,
					None,
					Some(10),
					Some(40),
					None,
					None,
					None,
					None,
					None,
				));

				let block_num = 10;
				set_block_number(block_num);

				let before = invariant(pool_id, asset_a, asset_b, block_num);
				assert_ok!(filter_errors(LBPPallet::sell(Origin::signed(ALICE), asset_a, asset_b, sell_amount, 0,)));

				let after = invariant(pool_id, asset_a, asset_b, block_num);
				assert!(after >= before);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_distributed_asset_invariant(
		assets in pool_assets(),
		sell_amount in trade_amount(),
		(weight_a, weight_b) in weights(),
	) {
		let asset_a = 1;
		let asset_b = 2;
		let pool_id: PoolId<Test> = 1002;

		let sell_amount = to_precision(sell_amount, assets.asset_a_decimals);

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(ALICE, asset_a, assets.asset_a_amount + sell_amount),
				(ALICE, asset_b, assets.asset_b_amount + sell_amount),
			])
			.build()
			.execute_with(|| {
				assert_ok!(LBPPallet::create_pool(
					Origin::root(),
					ALICE,
					asset_a,
					assets.asset_a_amount,
					asset_b,
					assets.asset_b_amount,
					weight_a,
					weight_b,
					WeightCurveType::Linear,
					(0, 1),
					CHARLIE,
					0,
				));
				assert_ok!(LBPPallet::update_pool_data(
					Origin::signed(ALICE),
					pool_id,
					None,
					Some(10),
					Some(40),
					None,
					None,
					None,
					None,
					None,
				));

				let block_num = 10;
				set_block_number(block_num);

				let before = invariant(pool_id, asset_a, asset_b, block_num);
				assert_ok!(filter_errors(LBPPallet::sell(Origin::signed(ALICE), asset_b, asset_a, sell_amount, 0,)));
				let after = invariant(pool_id, asset_a, asset_b, block_num);
				assert!(after >= before);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_distributed_invariant(
		assets in pool_assets(),
		buy_amount in trade_amount(),
		(weight_a, weight_b) in weights(),
	) {
		let asset_a = 1;
		let asset_b = 2;
		let pool_id: PoolId<Test> = 1002;

		let buy_amount = to_precision(buy_amount, assets.asset_b_decimals);

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(ALICE, asset_a, assets.asset_a_amount * 1000),
				(ALICE, asset_b, assets.asset_b_amount * 1000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(LBPPallet::create_pool(
					Origin::root(),
					ALICE,
					asset_a,
					assets.asset_a_amount,
					asset_b,
					assets.asset_b_amount,
					weight_a,
					weight_b,
					WeightCurveType::Linear,
					(0, 1),
					CHARLIE,
					0,
				));
				assert_ok!(LBPPallet::update_pool_data(
					Origin::signed(ALICE),
					pool_id,
					None,
					Some(10),
					Some(40),
					None,
					None,
					None,
					None,
					None,
				));

				let block_num = 10;
				set_block_number(block_num);

				let before = invariant(pool_id, asset_a, asset_b, block_num);
				assert_ok!(filter_errors(LBPPallet::buy(Origin::signed(ALICE), asset_b, asset_a, buy_amount, u128::MAX,)));
				let after = invariant(pool_id, asset_a, asset_b, block_num);
				assert!(after >= before);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_accumulated_invariant(
		assets in pool_assets(),
		buy_amount in trade_amount(),
		(weight_a, weight_b) in weights(),
	) {
		let asset_a = 1;
		let asset_b = 2;
		let pool_id: PoolId<Test> = 1002;

		let buy_amount = to_precision(buy_amount, assets.asset_b_decimals);

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(ALICE, asset_a, assets.asset_a_amount * 1000),
				(ALICE, asset_b, assets.asset_b_amount * 1000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(LBPPallet::create_pool(
					Origin::root(),
					ALICE,
					asset_a,
					assets.asset_a_amount,
					asset_b,
					assets.asset_b_amount,
					weight_a,
					weight_b,
					WeightCurveType::Linear,
					(0, 1),
					CHARLIE,
					0,
				));
				assert_ok!(LBPPallet::update_pool_data(
					Origin::signed(ALICE),
					pool_id,
					None,
					Some(10),
					Some(40),
					None,
					None,
					None,
					None,
					None,
				));

				let block_num = 10;
				set_block_number(block_num);

				let before = invariant(pool_id, asset_a, asset_b, block_num);
				assert_ok!(filter_errors(LBPPallet::buy(Origin::signed(ALICE), asset_a, asset_b, buy_amount, u128::MAX,)));

				let after = invariant(pool_id, asset_a, asset_b, block_num);
				assert!(after >= before);
			});
	}
}

fn filter_errors(dispatch_result: DispatchResult) -> DispatchResult {
	if dispatch_result.is_err() {
		let is_filtered = matches!(
			dispatch_result,
			Err(DispatchError::Module(ModuleError {
				index: 1,
				error: [14, 0, 0, 0],
				message: Some("MaxInRatioExceeded"),
			})) | Err(DispatchError::Module(ModuleError {
				index: 1,
				error: [15, 0, 0, 0],
				message: Some("MaxOutRatioExceeded"),
			}))
		);

		if is_filtered {
			println!("Error skipped");
			return Ok(());
		};
	}

	dispatch_result
}
