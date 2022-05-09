use super::*;
use crate::math::calculate_sell_state_changes;
use crate::{AssetState, FixedU128, SimpleImbalance};
use primitive_types::U256;
use proptest::prelude::*;

pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000; // * 1_000 * 1_000;

const BALANCE_RANGE: (Balance, Balance) = (10_000 * ONE, 10_000_000 * ONE);

fn asset_invariant(old_state: &AssetState<Balance>, new_state: &AssetState<Balance>, _desc: &str) -> FixedU128 {
	// new state invariant / old state invariant

	let new_s = U256::from(new_state.reserve) * U256::from(new_state.hub_reserve);
	let s1 = new_s.integer_sqrt();

	let old_s = U256::from(old_state.reserve) * U256::from(old_state.hub_reserve);
	let s2 = old_s.integer_sqrt();

	//if new_s < old_s {
	//	println!("{} - decreased new: {:?} vs old: {:?}", _desc, new_s,old_s);
	//}
	//assert!(new_s >= old_s, "Invariant decreased for {}", _desc);

	let s1_u128 = Balance::try_from(s1).unwrap();
	let s2_u128 = Balance::try_from(s2).unwrap();

	FixedU128::from((s1_u128, ONE)) / FixedU128::from((s2_u128, ONE))
}

fn asset_state() -> impl Strategy<Value = AssetState<Balance>> {
	(
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
	)
		.prop_map(|(reserve, hub_reserve, shares, protocol_shares, tvl)| AssetState {
			reserve,
			hub_reserve,
			shares,
			protocol_shares,
			tvl,
			..Default::default()
		})
}

fn asset_reserve() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	// Use one trade amount for now to follow python's testing
	Just(1000 * ONE)
	//1000..10_000 * ONE
}

fn fixed_fee() -> impl Strategy<Value = FixedU128> {
	(fee()).prop_map(FixedU128::from)
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

fn assert_asset_invariant(
	old_state: &AssetState<Balance>,
	new_state: &AssetState<Balance>,
	tolerance: FixedU128,
	desc: &str,
) {
	let invariant = asset_invariant(old_state, new_state, desc);
	assert_eq_approx!(invariant, FixedU128::from(1u128), tolerance, desc);
}
fn fee() -> impl Strategy<Value = (u32, u32)> {
	// Allow values between 0.001 and 0.1
	(0u32..1u32, prop_oneof![Just(1000u32), Just(10000u32), Just(100_000u32)]).prop_map(|(n, d)| (n, d))
}

#[derive(Debug)]
struct PoolToken {
	asset_id: AssetId,
	amount: Balance,
	price: FixedU128,
}

fn pool_token(asset_id: AssetId) -> impl Strategy<Value = PoolToken> {
	(asset_reserve(), price()).prop_map(move |(reserve, price)| PoolToken {
		asset_id,
		amount: reserve,
		price,
	})
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn swap_invariants_no_fees(asset_in in asset_state(), asset_out in asset_state(),
		amount in trade_amount()
	) {
		let result =  calculate_sell_state_changes(&asset_in, &asset_out, amount,
			FixedU128::from(0u128),
			FixedU128::from(0u128),
			&SimpleImbalance::default()
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let mut asset_in_state = asset_in.clone();
		assert!(asset_in_state.delta_update(&state_changes.asset_in).is_some());

		let in_invariant = asset_invariant(&asset_in, &asset_in_state,"" );

		assert_eq_approx!(in_invariant, FixedU128::from(1u128), FixedU128::from((TOLERANCE, ONE)), "Invariant");

		let mut asset_out_state = asset_out.clone();
		assert!(asset_out_state.delta_update(&state_changes.asset_out).is_some());

		let out_invariant = asset_invariant(&asset_out, &asset_out_state,"out" );

		assert_eq_approx!(out_invariant, FixedU128::from(1u128), FixedU128::from((TOLERANCE, ONE)), "Invariant");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn swap_invariants_with_fees(asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fixed_fee(),
		protocol_fee in fixed_fee()
	) {
		let result =  calculate_sell_state_changes(&asset_in, &asset_out, amount,
			asset_fee,
			protocol_fee,
			&SimpleImbalance::default()
		);

		assert!(result.is_some());

		let state_changes = result.unwrap();

		let mut asset_in_state = asset_in.clone();
		assert!(asset_in_state.delta_update(&state_changes.asset_in).is_some());

		let in_invariant = asset_invariant(&asset_in, &asset_in_state, "in" );

		assert_eq_approx!(in_invariant, FixedU128::from(1u128), FixedU128::from((TOLERANCE,ONE)), "Invariant");

		let mut asset_out_state = asset_out.clone();
		assert!(asset_out_state.delta_update(&state_changes.asset_out).is_some());

		let out_invariant = asset_invariant(&asset_out, &asset_out_state,"out" );

		assert_eq_approx!(out_invariant, FixedU128::from(1u128), FixedU128::from((TOLERANCE,ONE)), "Invariant");
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_invariants_feeless(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
				(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_initial_pool(
				stable_reserve,
				native_reserve,
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::signed(lp1), token_1.asset_id, token_1.amount, token_1.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp2), token_2.asset_id, token_2.amount, token_2.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp3), token_3.asset_id, token_3.amount, token_3.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp4), token_4.asset_id, token_4.amount, token_4.price));

				let old_state_200 = <Assets<Test>>::get(200).unwrap();
				let old_state_300 = <Assets<Test>>::get(300).unwrap();
				let old_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				fn sum_asset_hub_liquidity(assets: Vec<AssetId>) -> Balance {

					let mut total = 0;

					for asset_id in assets{
						let asset_state = <Assets<Test>>::get(asset_id).unwrap();
						 total += asset_state.hub_reserve;
					}

					total
				}

				let old_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(Origin::signed(seller), 200, 300, amount, Balance::zero()));

				let new_state_200 = <Assets<Test>>::get(200).unwrap();
				let new_state_300 = <Assets<Test>>::get(300).unwrap();
				let new_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= new_imbalance.value - old_imbalance.value; // note: in current implementation: imbalance cannot be positive, let's simply and ignore the sign for now

				let remaining = delta_q_300 - delta_q_200 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_invariants_with_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee()
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
				(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_reserve,
				native_reserve,
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::signed(lp1), token_1.asset_id, token_1.amount, token_1.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp2), token_2.asset_id, token_2.amount, token_2.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp3), token_3.asset_id, token_3.amount, token_3.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp4), token_4.asset_id, token_4.amount, token_4.price));

				let old_state_200 = <Assets<Test>>::get(200).unwrap();
				let old_state_300 = <Assets<Test>>::get(300).unwrap();
				let old_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				fn sum_asset_hub_liquidity(assets: Vec<AssetId>) -> Balance {

					let mut total = 0;

					for asset_id in assets{
						let asset_state = <Assets<Test>>::get(asset_id).unwrap();
						 total += asset_state.hub_reserve;
					}

					total
				}

				let old_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(Origin::signed(seller), 200, 300, amount, Balance::zero()));

				let new_state_200 = <Assets<Test>>::get(200).unwrap();
				let new_state_300 = <Assets<Test>>::get(300).unwrap();
				let new_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= new_imbalance.value - old_imbalance.value; // note: in current implementation: imbalance cannot be positive, let's simply and ignore the sign for now

				let remaining = delta_q_300 - delta_q_200 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_invariants_feeless(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
				(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(buyer, 200, amount * 1000 + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_initial_pool(
				stable_reserve,
				native_reserve,
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::signed(lp1), token_1.asset_id, token_1.amount, token_1.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp2), token_2.asset_id, token_2.amount, token_2.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp3), token_3.asset_id, token_3.amount, token_3.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp4), token_4.asset_id, token_4.amount, token_4.price));

				let old_state_200 = <Assets<Test>>::get(200).unwrap();
				let old_state_300 = <Assets<Test>>::get(300).unwrap();
				let old_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				fn sum_asset_hub_liquidity(assets: Vec<AssetId>) -> Balance {

					let mut total = 0;

					for asset_id in assets{
						let asset_state = <Assets<Test>>::get(asset_id).unwrap();
						 total += asset_state.hub_reserve;
					}

					total
				}

				let old_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(Origin::signed(buyer), 300, 200, amount, Balance::max_value()));

				let new_state_200 = <Assets<Test>>::get(200).unwrap();
				let new_state_300 = <Assets<Test>>::get(300).unwrap();
				let new_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= new_imbalance.value - old_imbalance.value; // note: in current implementation: imbalance cannot be positive, let's simply and ignore the sign for now

				let remaining = delta_q_300 - delta_q_200 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_invariants_with_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee()
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
				(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(buyer, 200, amount * 1000 + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_reserve,
				native_reserve,
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				assert_ok!(Omnipool::add_token(Origin::signed(lp1), token_1.asset_id, token_1.amount, token_1.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp2), token_2.asset_id, token_2.amount, token_2.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp3), token_3.asset_id, token_3.amount, token_3.price));
				assert_ok!(Omnipool::add_token(Origin::signed(lp4), token_4.asset_id, token_4.amount, token_4.price));

				let old_state_200 = <Assets<Test>>::get(200).unwrap();
				let old_state_300 = <Assets<Test>>::get(300).unwrap();
				let old_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				fn sum_asset_hub_liquidity(assets: Vec<AssetId>) -> Balance {

					let mut total = 0;

					for asset_id in assets{
						let asset_state = <Assets<Test>>::get(asset_id).unwrap();
						 total += asset_state.hub_reserve;
					}

					total
				}

				let old_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(Origin::signed(buyer), 300, 200, amount, Balance::max_value()));

				let new_state_200 = <Assets<Test>>::get(200).unwrap();
				let new_state_300 = <Assets<Test>>::get(300).unwrap();
				let new_state_hdx = <Assets<Test>>::get(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = <HubAssetLiquidity<Test>>::get();

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100,200,300,400]);

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= new_imbalance.value - old_imbalance.value; // note: in current implementation: imbalance cannot be positive, let's simply and ignore the sign for now

				let remaining = delta_q_300 - delta_q_200 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

#[test]
fn case_01() {
	let lp1: u64 = 100;
	let lp2: u64 = 200;
	let lp3: u64 = 300;
	let lp4: u64 = 400;
	let buyer: u64 = 500;

	let amount = 1000000000000000;
	let stable_price = FixedU128::from_float(0.1);
	let stable_reserve = 10000000000000000;
	let native_reserve = 10000000000000000;
	let token_1 = PoolToken {
		asset_id: 100,
		amount: 10000000000000000,
		price: FixedU128::from_float(0.1),
	};
	let token_2 = PoolToken {
		asset_id: 200,
		amount: 10000000000000000,
		price: FixedU128::from_float(0.1),
	};
	let token_3 = PoolToken {
		asset_id: 300,
		amount: 4078272607222477550,
		price: FixedU128::from_float(0.1),
	};
	let token_4 = PoolToken {
		asset_id: 400,
		amount: 10000000000000000,
		price: FixedU128::from_float(0.1),
	};

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_reserve + 1000 * ONE),
			(Omnipool::protocol_account(), HDX, native_reserve + 1000 * ONE),
			(lp1, 100, token_1.amount + 2 * ONE),
			(lp2, 200, token_2.amount + 2 * ONE),
			(lp3, 300, token_3.amount + 2 * ONE),
			(lp4, 400, token_4.amount + 2 * ONE),
			(buyer, 200, amount * 1000 + 200 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_registered_asset(300)
		.with_registered_asset(400)
		.with_initial_pool(stable_reserve, native_reserve, stable_price, FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::signed(lp1),
				token_1.asset_id,
				token_1.amount,
				token_1.price
			));
			assert_ok!(Omnipool::add_token(
				Origin::signed(lp2),
				token_2.asset_id,
				token_2.amount,
				token_2.price
			));
			assert_ok!(Omnipool::add_token(
				Origin::signed(lp3),
				token_3.asset_id,
				token_3.amount,
				token_3.price
			));
			assert_ok!(Omnipool::add_token(
				Origin::signed(lp4),
				token_4.asset_id,
				token_4.amount,
				token_4.price
			));

			let old_state_200 = <Assets<Test>>::get(200).unwrap();
			let old_state_300 = <Assets<Test>>::get(300).unwrap();
			let old_state_hdx = <Assets<Test>>::get(HDX).unwrap();

			let old_imbalance = <HubAssetImbalance<Test>>::get();

			let old_hub_liquidity = <HubAssetLiquidity<Test>>::get();

			fn sum_asset_hub_liquidity(assets: Vec<AssetId>) -> Balance {
				let mut total = 0;

				for asset_id in assets {
					let asset_state = <Assets<Test>>::get(asset_id).unwrap();
					total += asset_state.hub_reserve;
				}

				total
			}

			let old_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100, 200, 300, 400]);

			assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

			assert_ok!(Omnipool::buy(
				Origin::signed(buyer),
				300,
				200,
				amount,
				Balance::max_value()
			));

			let new_state_200 = <Assets<Test>>::get(200).unwrap();
			let new_state_300 = <Assets<Test>>::get(300).unwrap();
			let new_state_hdx = <Assets<Test>>::get(HDX).unwrap();

			// invariant does not decrease
			assert_ne!(new_state_200.reserve, old_state_200.reserve);
			assert_ne!(new_state_300.reserve, old_state_300.reserve);

			assert_asset_invariant(
				&old_state_200,
				&new_state_200,
				FixedU128::from((TOLERANCE, ONE)),
				"Invariant 200",
			);
			assert_asset_invariant(
				&old_state_300,
				&new_state_300,
				FixedU128::from((TOLERANCE, ONE)),
				"Invariant 300",
			);

			// Total hub asset liquidity has not changed
			let new_hub_liquidity = <HubAssetLiquidity<Test>>::get();

			assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

			// total quantity of R_i remains unchanged
			let new_asset_hub_liquidity = sum_asset_hub_liquidity(vec![HDX, DAI, 100, 200, 300, 400]);

			assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");

			let new_imbalance = <HubAssetImbalance<Test>>::get();

			// No LRNA lost
			let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
			let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
			let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
			let delta_imbalance = new_imbalance.value - old_imbalance.value; // note: in current implementation: imbalance cannot be positive, let's simply and ignore the sign for now

			let remaining = delta_q_300 - delta_q_200 - delta_q_hdx - delta_imbalance;
			assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
		});
}
