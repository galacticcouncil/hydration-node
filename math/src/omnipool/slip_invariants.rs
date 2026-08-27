// Adversarial value-conservation proptests for the SLIP-FEE trade paths.
//
// Rationale: every proptest in omnipool/invariants.rs passes `slip = None`, so the
// slip machinery (PR #1435) is exercised only by hand-picked numeric unit tests in
// omnipool/tests.rs. None of those run the randomized per-pool k-conservation check
// `assert_asset_invariant` (new_reserve*new_hub >= old_reserve*old_hub) that the
// no-slip paths are held to. This module closes that gap.
//
// The harness (asset_state, assert_asset_invariant) is copied verbatim from
// omnipool/invariants.rs so the tolerance / rounding semantics match exactly.

use crate::omnipool::types::{AssetReserveState, SignedBalance, TradeSlipFees};
use crate::omnipool::*;
use crate::types::Balance;
use primitive_types::U256;
use proptest::prelude::*;
use sp_arithmetic::Permill;

pub const ONE: Balance = 1_000_000_000_000;

const BALANCE_RANGE: (Balance, Balance) = (100_000 * ONE, 10_000_000 * ONE);

fn asset_state() -> impl Strategy<Value = AssetReserveState<Balance>> {
	(
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
		BALANCE_RANGE.0..BALANCE_RANGE.1,
	)
		.prop_map(|(reserve, hub_reserve, shares, protocol_shares)| AssetReserveState {
			reserve,
			hub_reserve,
			shares,
			protocol_shares,
		})
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	ONE / 10..10000 * ONE
}

fn fee() -> impl Strategy<Value = Permill> {
	(1u32..5u32, prop_oneof![Just(1000u32), Just(10000u32), Just(100_000u32)])
		.prop_map(|(n, d)| Permill::from_rational(n, d))
}

fn max_slip() -> impl Strategy<Value = Permill> {
	(1u32..20u32).prop_map(Permill::from_percent)
}

// The per-pool value-conservation check used for the no-slip paths in invariants.rs.
// new_reserve * new_hub_reserve must NOT be less than old_reserve * old_hub_reserve.
fn assert_asset_invariant(old_state: &AssetReserveState<Balance>, new_state: &AssetReserveState<Balance>, desc: &str) {
	let new_s = U256::from(new_state.reserve) * U256::from(new_state.hub_reserve);
	let old_s = U256::from(old_state.reserve) * U256::from(old_state.hub_reserve);
	assert!(
		new_s >= old_s,
		"Invariant decreased for {desc}: old={old_s}, new={new_s}, old_reserve={} old_hub={} new_reserve={} new_hub={}",
		old_state.reserve,
		old_state.hub_reserve,
		new_state.reserve,
		new_state.hub_reserve
	);
}

fn fresh_slip(
	asset_in: &AssetReserveState<Balance>,
	asset_out: &AssetReserveState<Balance>,
	max_slip_fee: Permill,
) -> TradeSlipFees {
	TradeSlipFees {
		asset_in_hub_reserve: asset_in.hub_reserve,
		asset_in_delta: SignedBalance::zero(),
		asset_out_hub_reserve: asset_out.hub_reserve,
		asset_out_delta: SignedBalance::zero(),
		max_slip_fee,
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(2000))]
	#[test]
	fn sell_with_slip_preserves_pool_invariant(
		asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		protocol_fee in fee(),
		max_slip_fee in max_slip(),
	) {
		let slip = fresh_slip(&asset_in, &asset_out, max_slip_fee);
		let result = calculate_sell_state_changes(
			&asset_in, &asset_out, amount,
			asset_fee, protocol_fee, Permill::zero(),
			Some(&slip),
		);
		if let Some(state_changes) = result {
			let asset_in_new = asset_in.clone().delta_update(&state_changes.asset_in).unwrap();
			assert_asset_invariant(&asset_in, &asset_in_new, "Sell w/ slip - token in");
			let asset_out_new = asset_out.clone().delta_update(&state_changes.asset_out).unwrap();
			assert_asset_invariant(&asset_out, &asset_out_new, "Sell w/ slip - token out");
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(2000))]
	#[test]
	fn sell_then_sell_back_roundtrip_no_profit(
		asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		max_slip_fee in max_slip(),
	) {
		// Zero fees: a user selling X->Y then immediately selling the received Y->X
		// must NOT end up with more X than they started (LP value leak / free money).
		// Model a single block: the second trade sees the hub-delta accumulated by the first.
		let slip1 = fresh_slip(&asset_in, &asset_out, max_slip_fee);
		let r1 = calculate_sell_state_changes(
			&asset_in, &asset_out, amount,
			Permill::zero(), Permill::zero(), Permill::zero(),
			Some(&slip1),
		);
		if let Some(sc1) = r1 {
			let tokens_out = *sc1.asset_out.delta_reserve;
			// Updated pool states after first trade.
			let asset_in_1 = asset_in.clone().delta_update(&sc1.asset_in).unwrap();
			let asset_out_1 = asset_out.clone().delta_update(&sc1.asset_out).unwrap();
			// Second trade Y->X within same block: in = old asset_out, out = old asset_in.
			// Accumulated deltas: asset_out lost hub d_net (negative), asset_in lost hub delta_hub_in (negative).
			let slip2 = TradeSlipFees {
				asset_in_hub_reserve: asset_out.hub_reserve,
				asset_in_delta: SignedBalance::Positive(*sc1.asset_out.delta_hub_reserve),
				asset_out_hub_reserve: asset_in.hub_reserve,
				asset_out_delta: SignedBalance::Negative(*sc1.asset_in.delta_hub_reserve),
				max_slip_fee,
			};
			let r2 = calculate_sell_state_changes(
				&asset_out_1, &asset_in_1, tokens_out,
				Permill::zero(), Permill::zero(), Permill::zero(),
				Some(&slip2),
			);
			if let Some(sc2) = r2 {
				let x_back = *sc2.asset_out.delta_reserve;
				assert!(
					x_back <= amount,
					"Round-trip PROFIT: sold {amount} X, got back {x_back} X (tokens_out={tokens_out})"
				);
			}
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(3000))]
	#[test]
	fn slip_trades_conserve_hub(
		asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		protocol_fee in fee(),
		max_slip_fee in max_slip(),
	) {
		// Hub (LRNA) conservation: the hub asset debited from the IN pool must equal the
		// hub credited to the OUT pool plus the total protocol fee. Any mismatch mints or
		// burns LRNA. This is NOT checked by the per-pool k-invariant.
		let slip = fresh_slip(&asset_in, &asset_out, max_slip_fee);

		if let Some(sc) = calculate_sell_state_changes(
			&asset_in, &asset_out, amount, asset_fee, protocol_fee, Permill::zero(), Some(&slip),
		) {
			let hub_in = *sc.asset_in.delta_hub_reserve;   // Decrease from in pool
			let hub_out = *sc.asset_out.delta_hub_reserve; // Increase to out pool
			let pf = sc.fee.protocol_fee;
			prop_assert_eq!(hub_in, hub_out + pf, "SELL hub not conserved: in={} out={} pf={}", hub_in, hub_out, pf);
		}

		if let Some(sc) = calculate_buy_state_changes(
			&asset_in, &asset_out, amount, asset_fee, protocol_fee, Permill::zero(), Some(&slip),
		) {
			let hub_in = *sc.asset_in.delta_hub_reserve;
			let hub_out = *sc.asset_out.delta_hub_reserve;
			let pf = sc.fee.protocol_fee;
			prop_assert_eq!(hub_in, hub_out + pf, "BUY hub not conserved: in={} out={} pf={}", hub_in, hub_out, pf);
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(3000))]
	#[test]
	fn buy_then_sellback_roundtrip_no_profit_with_slip(
		asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		max_slip_fee in max_slip(),
	) {
		// Zero fees. Executable round-trip: buy `amount` of OUT paying input I (asset_in),
		// mutate both pools, then sell `amount` of OUT back within the same block. The
		// recovered asset_in must NOT exceed I — otherwise the user extracts free value.
		let slip = fresh_slip(&asset_in, &asset_out, max_slip_fee);
		let buy = calculate_buy_state_changes(
			&asset_in, &asset_out, amount,
			Permill::zero(), Permill::zero(), Permill::zero(), Some(&slip),
		);
		if let Some(bc) = buy {
			let input_paid = *bc.asset_in.delta_reserve;
			let asset_in_1 = asset_in.clone().delta_update(&bc.asset_in).unwrap();
			let asset_out_1 = asset_out.clone().delta_update(&bc.asset_out).unwrap();
			// Sell `amount` of OUT back to IN, carrying accumulated hub deltas from the buy.
			let slip2 = TradeSlipFees {
				asset_in_hub_reserve: asset_out.hub_reserve,
				asset_in_delta: SignedBalance::Positive(*bc.asset_out.delta_hub_reserve),
				asset_out_hub_reserve: asset_in.hub_reserve,
				asset_out_delta: SignedBalance::Negative(*bc.asset_in.delta_hub_reserve),
				max_slip_fee,
			};
			let sell = calculate_sell_state_changes(
				&asset_out_1, &asset_in_1, amount,
				Permill::zero(), Permill::zero(), Permill::zero(), Some(&slip2),
			);
			if let Some(sic) = sell {
				let recovered = *sic.asset_out.delta_reserve;
				prop_assert!(
					recovered <= input_paid,
					"ROUND-TRIP PROFIT: paid {input_paid} to buy {amount}; selling {amount} back recovers {recovered} (> {input_paid})"
				);
			}
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(2000))]
	#[test]
	fn buy_with_slip_preserves_pool_invariant(
		asset_in in asset_state(),
		asset_out in asset_state(),
		amount in trade_amount(),
		asset_fee in fee(),
		protocol_fee in fee(),
		max_slip_fee in max_slip(),
	) {
		let slip = fresh_slip(&asset_in, &asset_out, max_slip_fee);
		let result = calculate_buy_state_changes(
			&asset_in, &asset_out, amount,
			asset_fee, protocol_fee, Permill::zero(),
			Some(&slip),
		);
		if let Some(state_changes) = result {
			let asset_in_new = asset_in.clone().delta_update(&state_changes.asset_in).unwrap();
			assert_asset_invariant(&asset_in, &asset_in_new, "Buy w/ slip - token in");
			let asset_out_new = asset_out.clone().delta_update(&state_changes.asset_out).unwrap();
			assert_asset_invariant(&asset_out, &asset_out_new, "Buy w/ slip - token out");
		}
	}
}
