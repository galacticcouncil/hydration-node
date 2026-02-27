use super::*;
use crate::types::SlipFeeConfig;
use crate::FixedU128;
use proptest::prelude::*;

pub const ONE: Balance = 1_000_000_000_000;

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

fn fee() -> impl Strategy<Value = Permill> {
	(
		0u32..=30u32,
		prop_oneof![Just(1000u32), Just(10000u32), Just(100_000u32)],
	)
		.prop_map(|(n, d)| Permill::from_rational(n, d))
}

fn withdrawal_fee() -> impl Strategy<Value = Permill> {
	(0u32..100u32).prop_map(Permill::from_percent)
}

fn max_slip_fee() -> impl Strategy<Value = Permill> {
	(1u32..=50u32).prop_map(Permill::from_percent)
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

// ---------------------------------------------------------------------------
// Sell invariants with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, 300, amount, Balance::zero()));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				// Asset invariant (R * Q) does not decrease
				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				// Hub liquidity equals sum of asset hub_reserves
				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_invariants_with_slip_fees_and_on_trade_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, 300, amount, Balance::zero()));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

// ---------------------------------------------------------------------------
// Buy invariants with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::MAX));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_invariants_with_slip_fees_and_on_trade_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, 200, amount, Balance::MAX));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

// ---------------------------------------------------------------------------
// Sell hub asset (LRNA) invariants with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_hub_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
			.with_treasury_lrna(1000 * ONE)
			.build()
			.execute_with(|| {
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), LRNA, 300, amount, Balance::zero()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_300.reserve, old_state_300.reserve);
				assert_hub_swap_invariants!(&old_state_300, &new_state_300, &old_state_hdx, &new_state_hdx, "Hub swap 300");

				let initial_treasury = 1000 * ONE;
				assert!(Tokens::free_balance(LRNA, &TREASURY) > initial_treasury, "Treasury received H2O");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_hub_invariants_with_slip_fees_and_on_trade_fee_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
			.with_treasury_lrna(1000 * ONE)
			.build()
			.execute_with(|| {
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), LRNA, 300, amount, Balance::zero()));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_300.reserve, old_state_300.reserve);
				assert_hub_swap_invariants!(&old_state_300, &new_state_300, &old_state_hdx, &new_state_hdx, "Hub swap 300");

				let initial_treasury = 1000 * ONE;
				assert!(Tokens::free_balance(LRNA, &TREASURY) > initial_treasury, "Treasury received H2O");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

// ---------------------------------------------------------------------------
// Buy hub asset (LRNA) invariants with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_hub_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, 100_000 * ONE),
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
			.with_treasury_lrna(1000 * ONE)
			.build()
			.execute_with(|| {
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(seller), 300, LRNA, amount, Balance::MAX));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_300.reserve, old_state_300.reserve);
				assert_hub_swap_invariants!(&old_state_300, &new_state_300, &old_state_hdx, &new_state_hdx, "Hub swap 300");

				let initial_treasury = 1000 * ONE;
				assert!(Tokens::free_balance(LRNA, &TREASURY) > initial_treasury, "Treasury received H2O");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_hub_invariants_with_slip_fees_and_on_trade_fee_withdrawal(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, LRNA, 100_000 * ONE),
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
			.with_treasury_lrna(1000 * ONE)
			.build()
			.execute_with(|| {
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(seller), 300, LRNA, amount, Balance::MAX));

				let new_state_300 = Omnipool::load_asset_state(300).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_300.reserve, old_state_300.reserve);
				assert_hub_swap_invariants!(&old_state_300, &new_state_300, &old_state_hdx, &new_state_hdx, "Hub swap 300");

				let initial_treasury = 1000 * ONE;
				assert!(Tokens::free_balance(LRNA, &TREASURY) > initial_treasury, "Treasury received H2O");
			});
	}
}

// ---------------------------------------------------------------------------
// Native asset (HDX) trades with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_native_for_asset_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(seller, HDX, amount + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_burn_fee(Permill::from_percent(50))
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), HDX, 300, amount, Balance::zero()));

				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_hdx.reserve, old_state_hdx.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant_not_decreased!(&old_state_hdx, &new_state_hdx, "Invariant HDX");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn sell_asset_for_native_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
			.with_burn_fee(Permill::from_percent(50))
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(seller), 200, HDX, amount, Balance::zero()));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_hdx.reserve, old_state_hdx.reserve);

				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_hdx, &new_state_hdx, "Invariant HDX");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_native_with_asset_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

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
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_burn_fee(Permill::from_percent(50))
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), HDX, 200, amount, Balance::MAX));

				let new_state_200 = Omnipool::load_asset_state(200).unwrap();
				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();

				assert_ne!(new_state_200.reserve, old_state_200.reserve);
				assert_ne!(new_state_hdx.reserve, old_state_hdx.reserve);

				assert_asset_invariant_not_decreased!(&old_state_200, &new_state_200, "Invariant 200");
				assert_asset_invariant_not_decreased!(&old_state_hdx, &new_state_hdx, "Invariant HDX");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn buy_asset_with_native_invariants_with_slip_fees(amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let buyer: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(buyer, HDX, amount * 1000 + 200 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_burn_fee(Permill::from_percent(50))
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let old_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let old_state_300 = Omnipool::load_asset_state(300).unwrap();
				let old_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let old_asset_hub_liquidity = sum_asset_hub_liquidity();

				assert_eq!(old_hub_liquidity, old_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, HDX, amount, Balance::MAX));

				let new_state_hdx = Omnipool::load_asset_state(HDX).unwrap();
				let new_state_300 = Omnipool::load_asset_state(300).unwrap();

				assert_ne!(new_state_hdx.reserve, old_state_hdx.reserve);
				assert_ne!(new_state_300.reserve, old_state_300.reserve);

				assert_asset_invariant_not_decreased!(&old_state_hdx, &new_state_hdx, "Invariant HDX");
				assert_asset_invariant_not_decreased!(&old_state_300, &new_state_300, "Invariant 300");

				let new_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let new_asset_hub_liquidity = sum_asset_hub_liquidity();
				assert_eq!(new_hub_liquidity, new_asset_hub_liquidity, "Assets hub liquidity");
			});
	}
}

// ---------------------------------------------------------------------------
// Add / remove liquidity invariants with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn add_liquidity_invariants_with_slip_fees(amount in trade_amount(),
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
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;
		let buyer: u64 = 600;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_1.asset_id, token_1.price, Permill::from_percent(100), lp1));
				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_2.asset_id, token_2.price, Permill::from_percent(100), lp2));
				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_3.asset_id, token_3.price, Permill::from_percent(100), lp3));
				assert_ok!(Omnipool::add_token(RuntimeOrigin::root(), token_4.asset_id, token_4.price, Permill::from_percent(100), lp4));

				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 300, LRNA, buy_amount, Balance::MAX));

				let old_state_200 = Omnipool::load_asset_state(200).unwrap();
				assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(seller), 200, amount));
				let new_state_200 = Omnipool::load_asset_state(200).unwrap();

				// Price should not change
				assert_eq_approx!(old_state_200.price().unwrap(),
						new_state_200.price().unwrap(),
						FixedU128::from_float(0.0000000001),
						"Price has changed after add liquidity");
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn remove_all_liquidity_invariants_with_slip_fees(amount in trade_amount(),
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
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let seller: u64 = 500;
		let buyer: u64 = 600;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let position_id = <NextPositionId<Test>>::get();
				assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(seller), 200, amount));
				let position = <Positions<Test>>::get(position_id).unwrap();
				let before_buy_state_200 = Omnipool::load_asset_state(200).unwrap();

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(buyer), 200, DAI, buy_amount, Balance::MAX));
				let old_state_200 = Omnipool::load_asset_state(200).unwrap();

				assert_asset_invariant_not_decreased!(&before_buy_state_200, &old_state_200, "Invariant 200");

				assert_ok!(Omnipool::remove_liquidity(RuntimeOrigin::signed(seller), position_id, position.shares));
				let new_state_200 = Omnipool::load_asset_state(200).unwrap();

				// Price should not change
				assert_eq_approx!(old_state_200.price().unwrap(),
						new_state_200.price().unwrap(),
						FixedU128::from_float(0.0000000001),
						"Price has changed after remove liquidity");
			});
	}
}

// ---------------------------------------------------------------------------
// Hub reserve accounting with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn hub_reserve_sum_equals_protocol_balance_with_slip_fees_sell(
		sell_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let trader: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(trader, LRNA, 200_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let initial_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let initial_asset_hub_liquidity = sum_asset_hub_liquidity();
				let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

				assert_eq!(initial_hub_liquidity, initial_asset_hub_liquidity);

				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(trader), LRNA, 300, sell_amount, Balance::zero()));

				let post_sell_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let post_sell_asset_hub_liquidity = sum_asset_hub_liquidity();
				let post_sell_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

				assert_eq!(
					post_sell_hub_liquidity, post_sell_asset_hub_liquidity,
					"Post-sell invariant: hub_liquidity must equal sum_asset_hub_liquidity"
				);

				assert_eq!(
					post_sell_hdx_state.hub_reserve, initial_hdx_state.hub_reserve,
					"HDX hub_reserve must be unchanged after sell_hub"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn hub_reserve_sum_equals_protocol_balance_with_slip_fees_buy(
		buy_amount in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let trader: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(trader, LRNA, 200_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let initial_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let initial_asset_hub_liquidity = sum_asset_hub_liquidity();
				let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

				assert_eq!(initial_hub_liquidity, initial_asset_hub_liquidity);

				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(trader), 300, LRNA, buy_amount, Balance::MAX));

				let post_buy_hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
				let post_buy_asset_hub_liquidity = sum_asset_hub_liquidity();
				let post_buy_hdx_state = Omnipool::load_asset_state(HDX).unwrap();

				assert_eq!(
					post_buy_hub_liquidity, post_buy_asset_hub_liquidity,
					"Post-buy invariant: hub_liquidity must equal sum_asset_hub_liquidity"
				);

				assert_eq!(
					post_buy_hdx_state.hub_reserve, initial_hdx_state.hub_reserve,
					"HDX hub_reserve must be unchanged after buy_for_hub"
				);

				assert!(Tokens::free_balance(LRNA, &TREASURY) > 0, "Treasury must receive LRNA");
			});
	}
}

// ---------------------------------------------------------------------------
// Multiple operations with slip fees
// ---------------------------------------------------------------------------

proptest! {
	#![proptest_config(ProptestConfig::with_cases(100))]
	#[test]
	fn hub_reserve_sum_equals_protocol_balance_after_multiple_operations_with_slip_fees(
		sell_amount_1 in trade_amount(),
		sell_amount_2 in trade_amount(),
		buy_amount_1 in trade_amount(),
		buy_amount_2 in trade_amount(),
		stable_price in price(),
		stable_reserve in asset_reserve(),
		native_reserve in asset_reserve(),
		token_1 in pool_token(100),
		token_2 in pool_token(200),
		token_3 in pool_token(300),
		token_4 in pool_token(400),
		asset_fee in fee(),
		protocol_fee in fee(),
		withdraw_fee in withdrawal_fee(),
		slip_max in max_slip_fee(),
	) {
		let lp1: u64 = 100;
		let lp2: u64 = 200;
		let lp3: u64 = 300;
		let lp4: u64 = 400;
		let trader: u64 = 500;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(Omnipool::protocol_account(), DAI, stable_reserve),
				(Omnipool::protocol_account(), HDX, native_reserve),
				(lp1, 100, token_1.amount + 2 * ONE),
				(lp2, 200, token_2.amount + 2 * ONE),
				(lp3, 300, token_3.amount + 2 * ONE),
				(lp4, 400, token_4.amount + 2 * ONE),
				(trader, LRNA, 500_000 * ONE),
			])
			.with_registered_asset(100)
			.with_registered_asset(200)
			.with_registered_asset(300)
			.with_registered_asset(400)
			.with_asset_fee(asset_fee)
			.with_protocol_fee(protocol_fee)
			.with_on_trade_withdrawal(withdraw_fee)
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
				SlipFee::<Test>::put(SlipFeeConfig { max_slip_fee: slip_max });

				let check_invariant = |msg: &str| {
					let hub_liquidity = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
					let asset_hub_liquidity = sum_asset_hub_liquidity();
					assert_eq!(hub_liquidity, asset_hub_liquidity, "{}", msg);
				};

				let initial_hdx_state = Omnipool::load_asset_state(HDX).unwrap();
				check_invariant("Initial invariant");

				// Operation 1: Sell LRNA -> asset 300
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(trader), LRNA, 300, sell_amount_1, Balance::zero()));
				check_invariant("After sell #1 (LRNA -> 300)");

				let hdx_after_sell_1 = Omnipool::load_asset_state(HDX).unwrap();
				assert_eq!(
					hdx_after_sell_1.hub_reserve, initial_hdx_state.hub_reserve,
					"HDX hub_reserve must be unchanged after sell #1"
				);

				// Operation 2: Buy asset 100 with LRNA
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(trader), 100, LRNA, buy_amount_1, Balance::MAX));
				check_invariant("After buy #1 (100 <- LRNA)");

				let hdx_after_buy_1 = Omnipool::load_asset_state(HDX).unwrap();
				assert_eq!(
					hdx_after_buy_1.hub_reserve, hdx_after_sell_1.hub_reserve,
					"HDX hub_reserve must be unchanged after buy #1"
				);

				// Operation 3: Sell LRNA -> asset 200
				assert_ok!(Omnipool::sell(RuntimeOrigin::signed(trader), LRNA, 200, sell_amount_2, Balance::zero()));
				check_invariant("After sell #2 (LRNA -> 200)");

				let hdx_after_sell_2 = Omnipool::load_asset_state(HDX).unwrap();
				assert_eq!(
					hdx_after_sell_2.hub_reserve, hdx_after_buy_1.hub_reserve,
					"HDX hub_reserve must be unchanged after sell #2"
				);

				// Operation 4: Buy asset 400 with LRNA
				assert_ok!(Omnipool::buy(RuntimeOrigin::signed(trader), 400, LRNA, buy_amount_2, Balance::MAX));
				check_invariant("After buy #2 (400 <- LRNA)");

				let hdx_after_buy_2 = Omnipool::load_asset_state(HDX).unwrap();
				assert_eq!(
					hdx_after_buy_2.hub_reserve, hdx_after_sell_2.hub_reserve,
					"HDX hub_reserve must be unchanged after buy #2"
				);

				assert_eq!(
					hdx_after_buy_2.hub_reserve, initial_hdx_state.hub_reserve,
					"HDX hub_reserve must be unchanged after all operations"
				);
				assert!(
					Tokens::free_balance(LRNA, &TREASURY) > 0,
					"Treasury must have accumulated LRNA after all operations"
				);
			});
	}
}
