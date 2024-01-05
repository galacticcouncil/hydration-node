use super::*;
use crate::{AssetReserveState, FixedU128};
use frame_support::assert_noop;
use primitive_types::U256;
use proptest::prelude::*;

pub const ONE: Balance = 1_000_000_000_000;
pub const TOLERANCE: Balance = 1_000_000_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);

fn asset_reserve() -> impl Strategy<Value = Balance> {
	BALANCE_RANGE.0..BALANCE_RANGE.1
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..5000 * ONE
}

fn price() -> impl Strategy<Value = FixedU128> {
	(0.1f64..2f64).prop_map(FixedU128::from_float)
}

fn min_withdrawal_fee() -> impl Strategy<Value = Permill> {
	(0.001f64..2f64).prop_map(Permill::from_float)
}

fn adjustment() -> impl Strategy<Value = (u32, u32, bool)> {
	(
		0u32..50u32,
		prop_oneof![Just(100), Just(1000), Just(10000)],
		prop_oneof![Just(true), Just(false)],
	)
}

fn some_imbalance() -> impl Strategy<Value = SimpleImbalance<Balance>> {
	(0..10000 * ONE).prop_map(|value| SimpleImbalance { value, negative: true })
}

fn assert_asset_invariant(
	old_state: &AssetReserveState<Balance>,
	new_state: &AssetReserveState<Balance>,
	tolerance: FixedU128,
	desc: &str,
) {
	let new_s = U256::from(new_state.reserve) * U256::from(new_state.hub_reserve);
	let s1 = new_s.integer_sqrt();

	let old_s = U256::from(old_state.reserve) * U256::from(old_state.hub_reserve);
	let s2 = old_s.integer_sqrt();

	assert!(
		new_s >= old_s,
		"Invariant decreased for {desc} - {:?} >= {:?}",
		new_s,
		old_s
	);

	let s1_u128 = Balance::try_from(s1).unwrap();
	let s2_u128 = Balance::try_from(s2).unwrap();

	let invariant = FixedU128::from((s1_u128, ONE)) / FixedU128::from((s2_u128, ONE));
	assert_eq_approx!(invariant, FixedU128::from(1u128), tolerance, desc);
}
fn fee() -> impl Strategy<Value = Permill> {
	// Allow values between 0.001 and 3%
	(
		0u32..=30u32,
		prop_oneof![Just(1000u32), Just(10000u32), Just(100_000u32)],
	)
		.prop_map(|(n, d)| Permill::from_rational(n, d))
}

fn sum_asset_hub_liquidity() -> Balance {
	<Assets<Test>>::iter().fold(0, |acc, v| acc + v.1.hub_reserve)
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
	#![proptest_config(ProptestConfig::with_cases(100))]
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, 300, amount, Balance::zero()));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

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
	#![proptest_config(ProptestConfig::with_cases(100))]
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
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
			.with_protocol_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				HubAssetImbalance::<Test>::set(imbalance);

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, 300, amount, Balance::zero()));

				let updated_imbalance = HubAssetImbalance::<Test>::get();

				assert!(updated_imbalance.value <= imbalance.value);

				let imbalance_diff = imbalance.value - updated_imbalance.value;

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity, new_hub_liquidity + imbalance_diff, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity + imbalance_diff, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance = old_imbalance.value - new_imbalance.value;

				let remaining = delta_q_200 - delta_q_300 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_invariants_with_fees_and_on_trade_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
			.with_protocol_fee(protocol_fee)
			.with_on_trade_withdrawal(Permill::from_percent(100))
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				HubAssetImbalance::<Test>::set(imbalance);

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, 300, amount, Balance::zero()));

				let updated_imbalance = HubAssetImbalance::<Test>::get();

				assert!(updated_imbalance.value <= imbalance.value);

				let imbalance_diff = imbalance.value - updated_imbalance.value;

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity, new_hub_liquidity + imbalance_diff, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity + imbalance_diff, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance = old_imbalance.value - new_imbalance.value;

				let remaining = delta_q_200 - delta_q_300 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::max_value()));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

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
	#![proptest_config(ProptestConfig::with_cases(100))]
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
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
			.with_protocol_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				HubAssetImbalance::<Test>::set(imbalance);
				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::max_value()));

				let updated_imbalance = HubAssetImbalance::<Test>::get();
				assert!(updated_imbalance.value <= imbalance.value);

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let imbalance_diff = imbalance.value - updated_imbalance.value;

				assert_eq!(old_hub_liquidity, new_hub_liquidity + imbalance_diff, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity + imbalance_diff, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= old_imbalance.value - new_imbalance.value;

				let remaining = delta_q_200 - delta_q_300 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_invariants_with_fees_and_on_trade_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		imbalance in some_imbalance(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
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
			.with_protocol_fee(protocol_fee)
			.with_on_trade_withdrawal(Permill::from_percent(100))
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				HubAssetImbalance::<Test>::set(imbalance);
				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::max_value()));

				let updated_imbalance = HubAssetImbalance::<Test>::get();
				assert!(updated_imbalance.value <= imbalance.value);

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_200, &new_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");
				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let imbalance_diff = imbalance.value - updated_imbalance.value;

				assert_eq!(old_hub_liquidity, new_hub_liquidity + imbalance_diff, "Total Hub liquidity has changed!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_asset_hub_liquidity, new_asset_hub_liquidity + imbalance_diff, "Assets hub liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				// No LRNA lost
				let delta_q_200 = old_state_200.hub_reserve - new_state_200.hub_reserve;
				let delta_q_300 = new_state_300.hub_reserve - old_state_300.hub_reserve;
				let delta_q_hdx = new_state_hdx.hub_reserve - old_state_hdx.hub_reserve;
				let delta_imbalance= old_imbalance.value - new_imbalance.value;

				let remaining = delta_q_200 - delta_q_300 - delta_q_hdx - delta_imbalance;
				assert_eq!(remaining, 0u128, "Some LRNA was lost along the way");
			});
	}
}

#[test]
fn buy_invariant_case_01() {
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
			(Omnipool::protocol_account(), DAI, stable_reserve),
			(Omnipool::protocol_account(), HDX, native_reserve),
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
		.with_initial_pool(stable_price, FixedU128::from(1))
		.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
		.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
		.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
		.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
		.build()
		.execute_with(|| {
			let old_state_200 = Omnipool::load_asset_state(200).unwrap();
			let old_state_300 = Omnipool::load_asset_state(300).unwrap();
			let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

			let old_imbalance = <HubAssetImbalance<Test>>::get();

			let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			let old_asset_hub_liquidity = sum_asset_hub_liquidity();

			assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(buyer),
				300,
				200,
				amount,
				Balance::max_value()
			));

			let new_state_200 = Omnipool::load_asset_state(200).unwrap();
			let new_state_300 = Omnipool::load_asset_state(300).unwrap();
			let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

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
			let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

			// total quantity of R_i remains unchanged
			let new_asset_hub_liquidity = sum_asset_hub_liquidity();

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

#[test]
fn buy_invariant_case_02() {
	let lp1: u64 = 100;
	let lp2: u64 = 200;
	let lp3: u64 = 300;
	let lp4: u64 = 400;
	let buyer: u64 = 500;

	let amount = 1_023_135_244_731_817;
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
		amount: 10_000_000_000_000_000,
		price: FixedU128::from_float(1.827_143_565_363_142_7),
	};
	let token_4 = PoolToken {
		asset_id: 400,
		amount: 10000000000000000,
		price: FixedU128::from_float(0.1),
	};

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_reserve),
			(Omnipool::protocol_account(), HDX, native_reserve),
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
		.with_initial_pool(stable_price, FixedU128::from(1))
		.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
		.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
		.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
		.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
		.build()
		.execute_with(|| {
			let old_state_200 = Omnipool::load_asset_state(200).unwrap();
			let old_state_300 = Omnipool::load_asset_state(300).unwrap();
			let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

			let old_imbalance = <HubAssetImbalance<Test>>::get();

			let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			let old_asset_hub_liquidity = sum_asset_hub_liquidity();

			assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

			// TODO: this fais with Overflow - but the real error should be Insufficient token amount after out calc
			assert_noop!(
				Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::max_value()),
				ArithmeticError::Overflow
			);

			let new_state_200 = Omnipool::load_asset_state(200).unwrap();
			let new_state_300 = Omnipool::load_asset_state(300).unwrap();
			let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

			// invariant does not decrease
			// assert_ne!(new_state_200.reserve, old_state_200.reserve);
			// assert_ne!(new_state_300.reserve, old_state_300.reserve);

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
			let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

			assert_eq!(old_hub_liquidity, new_hub_liquidity, "Total Hub liquidity has changed!");

			// total quantity of R_i remains unchanged
			let new_asset_hub_liquidity = sum_asset_hub_liquidity();

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

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_hub_invariants_with_fees(amount in trade_amount(),
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), LRNA, 300, amount, Balance::zero()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity + amount, new_hub_liquidity, "Total Hub liquidity increased incorrectly!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				assert_eq!(old_asset_hub_liquidity + amount, new_asset_hub_liquidity, "Assets hub liquidity");

				assert_imbalance_update(
					old_imbalance.value,
					new_imbalance.value,
					old_hub_liquidity,
					new_hub_liquidity,
					"Imbalance invariant in sell LRNA is incorrect"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_hub_invariants_with_fees_and_on_trade_fee_withdrawal(amount in trade_amount(),
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_on_trade_withdrawal(Permill::from_percent(100))
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), LRNA, 300, amount, Balance::zero()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq!(old_hub_liquidity + amount, new_hub_liquidity, "Total Hub liquidity increased incorrectly!");

				// total quantity of R_i remains unchanged
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				assert_eq!(old_asset_hub_liquidity + amount, new_asset_hub_liquidity, "Assets hub liquidity");

				assert_imbalance_update(
					old_imbalance.value,
					new_imbalance.value,
					old_hub_liquidity,
					new_hub_liquidity,
					"Imbalance invariant in sell LRNA is incorrect"
				);
			});
	}
}

fn assert_imbalance_update(
	old_imbalance: Balance,
	new_imbalance: Balance,
	old_hub_reserve: Balance,
	new_hub_reserve: Balance,
	desc: &str,
) {
	let q = U256::from(old_hub_reserve);
	let q_plus = U256::from(new_hub_reserve);
	let l = U256::from(old_imbalance);
	let l_plus = U256::from(new_imbalance);

	let left = q.checked_mul(q.checked_sub(l).unwrap()).unwrap();
	let right = q_plus.checked_mul(q_plus.checked_sub(l_plus).unwrap()).unwrap();

	assert!(left >= right, "{}", desc);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_hub_invariants_with_fees(amount in trade_amount(),
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, 100_000* ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(seller), 300, LRNA, amount, Balance::max_value()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				assert!(old_hub_liquidity < new_hub_liquidity, "Total Hub liquidity increased incorrectly!");

				assert_imbalance_update(
					old_imbalance.value,
					new_imbalance.value,
					old_hub_liquidity,
					new_hub_liquidity,
					"Imbalance invariant in buy for LRNA is incorrect"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_hub_invariants_with_fees_and_on_trade_fee_withdrawal(amount in trade_amount(),
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
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, 100_000* ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_on_trade_withdrawal(Permill::from_percent(100))
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(seller), 300, LRNA, amount, Balance::max_value()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				// invariant does not decrease
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant(&old_state_300, &new_state_300, FixedU128::from((TOLERANCE,ONE)), "Invariant 300");

				// Total hub asset liquidity has not changed
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let new_imbalance = <HubAssetImbalance<Test>>::get();

				assert!(old_hub_liquidity < new_hub_liquidity, "Total Hub liquidity increased incorrectly!");

				assert_imbalance_update(
					old_imbalance.value,
					new_imbalance.value,
					old_hub_liquidity,
					new_hub_liquidity,
					"Imbalance invariant in buy for LRNA is incorrect"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn add_liquidity_invariants_with_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		buy_amount in trade_amount(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;
		let buyer: u64 = 600;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(Omnipool::protocol_account(), token_1.asset_id, token_1.amount),
				(Omnipool::protocol_account(), token_2.asset_id, token_2.amount),
				(Omnipool::protocol_account(), token_3.asset_id, token_3.amount),
				(Omnipool::protocol_account(), token_4.asset_id, token_4.amount),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
				(buyer, LRNA, 200_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.build()
			.execute_with(|| {
				let old_imbalance = <HubAssetImbalance<Test>>::get();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_1.asset_id, token_1.price,Permill::from_percent(100),lp1));

				let new_imbalance = <HubAssetImbalance<Test>>::get();
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq_approx!( FixedU128::from((old_imbalance.value, old_hub_liquidity)),
								   FixedU128::from((new_imbalance.value, new_hub_liquidity)),
								   FixedU128::from_float(0.000000001),
								   "L/Q ratio changed"
				);

				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_2.asset_id, token_2.price,Permill::from_percent(100),lp2));
				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_3.asset_id, token_3.price,Permill::from_percent(100), lp3));

				let old_imbalance = <HubAssetImbalance<Test>>::get();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_4.asset_id, token_4.price,Permill::from_percent(100),lp4));

				let new_imbalance = <HubAssetImbalance<Test>>::get();
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq_approx!( FixedU128::from((old_imbalance.value, old_hub_liquidity)),
								   FixedU128::from((new_imbalance.value, new_hub_liquidity)),
								   FixedU128::from_float(0.000000001),
								   "L/Q ratio changed"
				);

				// Let's do a trade so imbalance changes, so it is not always 0
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, LRNA, buy_amount, Balance::max_value()));

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();

				let old_imbalance = <HubAssetImbalance<Test>>::get();

				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(seller), 200, amount));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();

				// Price should not change
				assert_eq_approx!(old_state_200.price().unwrap(),
						new_state_200.price().unwrap(),
						FixedU128::from_float(0.0000000001),
						"Price has changed after add liquidity");

				let new_imbalance = <HubAssetImbalance<Test>>::get();
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				assert_eq_approx!( FixedU128::from((old_imbalance.value, old_hub_liquidity)),
								   FixedU128::from((new_imbalance.value, new_hub_liquidity)),
								   FixedU128::from_float(0.000000001),
								   "L/Q ratio changed"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn remove_all_liquidity_invariants_with_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		buy_amount in trade_amount(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;
		let buyer: u64 = 600;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
				(buyer, DAI, 200_000_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.build()
			.execute_with(|| {
				let old_imbalance = <HubAssetImbalance<Test>>::get();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let position_id = <NextPositionId<Test>>::get();
				assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(seller), 200, amount));

				let position = <Positions<Test>>::get(position_id).unwrap();

				let before_buy_state_200 = Omnipool::load_asset_state(200).unwrap();

				// Let's do a trade so imbalance and price changes
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 200, DAI, buy_amount, Balance::max_value()));

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();

				assert_asset_invariant(&before_buy_state_200, &old_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");

				assert_ok!(Omnipool::remove_liquidity(RuntimeOrigin::signed(seller), position_id, position.shares));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_imbalance = <HubAssetImbalance<Test>>::get();

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				// Price should not change
				assert_eq_approx!(old_state_200.price().unwrap(),
						new_state_200.price().unwrap(),
						FixedU128::from_float(0.0000000001),
						"Price has changed after remove liquidity");

				assert_eq_approx!( FixedU128::from((old_imbalance.value, old_hub_liquidity)),
								   FixedU128::from((new_imbalance.value, new_hub_liquidity)),
								   FixedU128::from_float(0.000000001),
								   "L/Q ratio changed after remove liquidity"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn remove_liquidity_should_calculate_withdrawal_fee_correctly(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		buy_amount in trade_amount(),
		min_withdraw_fee in min_withdrawal_fee(),
		(price_adjustment, denom, direction) in adjustment(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;
		let buyer: u64 = 600;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve ),
				(Omnipool::protocol_account(), HDX, native_reserve ),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, 200, amount + 200 * ONE),
				(buyer, DAI, 200_000_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_asset_fee(protocol_fee)
			.with_initial_pool(
				stable_price,
				FixedU128::from(1),
			)
			.with_token(token_1.asset_id, token_1.price, lp1, token_1.amount)
			.with_token(token_2.asset_id, token_2.price, lp2, token_2.amount)
			.with_token(token_3.asset_id, token_3.price, lp3, token_3.amount)
			.with_token(token_4.asset_id, token_4.price, lp4, token_4.amount)
			.with_min_withdrawal_fee(min_withdraw_fee)
			.with_withdrawal_adjustment((price_adjustment, denom, direction))
			.build()
			.execute_with(|| {
				let old_imbalance = <HubAssetImbalance<Test>>::get();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				let position_id = <NextPositionId<Test>>::get();
				assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(seller), 200, amount));

				let position = <Positions<Test>>::get(position_id).unwrap();

				let before_buy_state_200 = Omnipool::load_asset_state(200).unwrap();

				// Let's do a trade so imbalance and price changes
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 200, DAI, buy_amount, Balance::max_value()));

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();

				assert_asset_invariant(&before_buy_state_200, &old_state_200, FixedU128::from((TOLERANCE,ONE)), "Invariant 200");

				assert_ok!(Omnipool::remove_liquidity(RuntimeOrigin::signed(seller), position_id, position.shares));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_imbalance = <HubAssetImbalance<Test>>::get();

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());

				// Price should not change
				assert_eq_approx!(old_state_200.price().unwrap(),
						new_state_200.price().unwrap(),
						FixedU128::from_float(0.0000000001),
						"Price has changed after remove liquidity");

				assert_eq_approx!( FixedU128::from((old_imbalance.value, old_hub_liquidity)),
								   FixedU128::from((new_imbalance.value, new_hub_liquidity)),
								   FixedU128::from_float(0.000000001),
								   "L/Q ratio changed after remove liquidity"
				);
			});
	}
}
