// SPDX-License-Identifier: Apache-2.0
//
// Property-based invariants for stake / unstake / yield-accrual /
// realize_yield interleavings. Rounding must always favour the protocol:
// users are never over-credited (INV6) and the gigapot is never over-drawn
// (INV7). Assertions use the pallet's own math so production code is
// validated against itself, not a re-derived model.

use super::mock::*;
use crate::{Error, Stakes, TotalLocked};
use frame_support::assert_ok;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use primitives::Balance;
use proptest::prelude::*;

const ACCS: [AccountId; 3] = [ALICE, BOB, TREASURY];

fn acc(i: usize) -> AccountId {
	ACCS[i % ACCS.len()]
}

fn locked_under_ghdx(a: AccountId) -> Balance {
	pallet_balances::Locks::<Test>::get(a)
		.iter()
		.find(|l| l.id == GIGAHDX_LOCK_ID)
		.map(|l| l.amount)
		.unwrap_or(0)
}

fn gigapot_balance() -> Balance {
	Balances::free_balance(GigaHdx::gigapot_account_id())
}

fn current_value(g: Balance) -> Balance {
	GigaHdx::calculate_hdx_amount_given_gigahdx(g).expect("rate math overflow")
}

#[derive(Debug, Clone)]
enum Op {
	Stake { who: usize, amount: Balance },
	Unstake { who: usize, frac: u8 },
	AccrueYield { amount: Balance },
	RealizeYield { who: usize },
	Unlock { who: usize },
}

fn op_strategy() -> impl Strategy<Value = Op> {
	prop_oneof![
		(0usize..3, ONE..=50 * ONE).prop_map(|(who, amount)| Op::Stake { who, amount }),
		(0usize..3, 1u8..=4).prop_map(|(who, frac)| Op::Unstake { who, frac }),
		(ONE..=100 * ONE).prop_map(|amount| Op::AccrueYield { amount }),
		(0usize..3).prop_map(|who| Op::RealizeYield { who }),
		(0usize..3).prop_map(|who| Op::Unlock { who }),
	]
}

/// INV1, INV2, INV7 — asserted after every op.
fn assert_global_invariants() {
	let mut sum_hdx: Balance = 0;
	let mut sum_value: Balance = 0;
	for &a in ACCS.iter() {
		let (hdx, gigahdx, unstaking) = Stakes::<Test>::get(a)
			.map(|s| (s.hdx, s.gigahdx, s.unstaking))
			.unwrap_or((0, 0, 0));
		sum_hdx = sum_hdx.saturating_add(hdx);
		sum_value = sum_value.saturating_add(current_value(gigahdx));
		// INV2: the ghdxlock equals active + pending principal.
		assert_eq!(
			locked_under_ghdx(a),
			hdx.saturating_add(unstaking),
			"INV2 lock mismatch for {a}"
		);
	}
	// INV1: TotalLocked is the sum of active principals.
	assert_eq!(TotalLocked::<Test>::get(), sum_hdx, "INV1 TotalLocked mismatch");
	// INV7: aggregate solvency — total redeemable value never exceeds total
	// backing (`TotalLocked + gigapot`). This is the real protocol promise and
	// holds exactly. (The *clamped* per-user sum can exceed the gigapot by a
	// few atomic units of cross-user rounding dust; that surfaces as a clean
	// `GigapotInsufficient` revert on `realize_yield`, never an over-draw.)
	assert!(
		sum_value <= GigaHdx::total_staked_hdx(),
		"INV7 solvency: redeemable {sum_value} > backing {}",
		GigaHdx::total_staked_hdx()
	);
}

/// Run realize_yield with the local conservation / inertness checks
/// (INV3, INV4, INV5, INV6, INV9).
fn realize_checked(a: AccountId) {
	let before = Stakes::<Test>::get(a);
	let record_present = before.is_some();
	let hdx_before = before.as_ref().map(|s| s.hdx).unwrap_or(0);
	let gigahdx_before = before.as_ref().map(|s| s.gigahdx).unwrap_or(0);
	let frozen_before = before.as_ref().map(|s| s.frozen).unwrap_or(0);
	let unstaking_before = before.as_ref().map(|s| s.unstaking).unwrap_or(0);

	let supply_before = GigaHdx::total_gigahdx_supply();
	let rate_before = GigaHdx::exchange_rate();
	let gigapot_before = gigapot_balance();
	let acct_total_before = Balances::total_balance(&a);
	let issuance_before = Balances::total_issuance();

	// Cross-user rounding can leave the gigapot a few atomic units short of a
	// staker's clamped claimable. realize_yield then reverts cleanly with
	// `GigapotInsufficient` (INV8: full rollback), never over-draws.
	match GigaHdx::realize_yield(RawOrigin::Signed(a).into()) {
		Ok(()) => {}
		Err(e) => {
			assert_eq!(
				e,
				Error::<Test>::GigapotInsufficient.into(),
				"unexpected realize_yield error: {e:?}"
			);
			assert_eq!(Stakes::<Test>::get(a), before, "INV8 rollback: Stakes changed");
			assert_eq!(gigapot_balance(), gigapot_before, "INV8 rollback: gigapot changed");
			assert_eq!(
				GigaHdx::total_gigahdx_supply(),
				supply_before,
				"INV8 rollback: supply changed"
			);
			return;
		}
	}

	let after = Stakes::<Test>::get(a);
	// INV9: realize never creates or destroys a record.
	assert_eq!(after.is_some(), record_present, "INV9 record lifecycle");

	let hdx_after = after.as_ref().map(|s| s.hdx).unwrap_or(0);
	let gigahdx_after = after.as_ref().map(|s| s.gigahdx).unwrap_or(0);
	let frozen_after = after.as_ref().map(|s| s.frozen).unwrap_or(0);
	let unstaking_after = after.as_ref().map(|s| s.unstaking).unwrap_or(0);
	let accrued = hdx_after - hdx_before;
	let cv_before = current_value(gigahdx_before);

	// INV3: economically inert.
	assert_eq!(gigahdx_after, gigahdx_before, "INV3 gigahdx changed");
	assert_eq!(GigaHdx::total_gigahdx_supply(), supply_before, "INV3 supply changed");
	assert_eq!(GigaHdx::exchange_rate(), rate_before, "INV3 rate changed");
	assert_eq!(frozen_after, frozen_before, "INV3 frozen changed");
	assert_eq!(unstaking_after, unstaking_before, "INV3 unstaking changed");

	// INV4: pure transfer gigapot → who, no mint.
	assert_eq!(gigapot_balance(), gigapot_before - accrued, "INV4 gigapot delta");
	assert_eq!(
		Balances::total_balance(&a),
		acct_total_before + accrued,
		"INV4 account delta"
	);
	assert_eq!(
		Balances::total_issuance(),
		issuance_before,
		"INV4 native issuance moved"
	);

	// INV6: credited exactly the clamped floor accrued — never more. In the
	// no-op case (`cv_before <= hdx_before`, a stake-time rounding residual in
	// the protocol's favour) nothing is credited and the residual is left
	// untouched.
	assert_eq!(
		accrued,
		cv_before.saturating_sub(hdx_before),
		"INV6 over-credit: accrued exceeds floor yield"
	);
	if accrued > 0 {
		assert_eq!(hdx_after, cv_before, "INV6 principal != current value after realize");
	}

	// INV5: immediately idempotent.
	let h = hdx_after;
	let p = gigapot_balance();
	assert_ok!(GigaHdx::realize_yield(RawOrigin::Signed(a).into()));
	assert_eq!(
		Stakes::<Test>::get(a).map(|s| s.hdx).unwrap_or(0),
		h,
		"INV5 not idempotent"
	);
	assert_eq!(gigapot_balance(), p, "INV5 idempotent gigapot");
}

fn apply(op: &Op) {
	match *op {
		Op::Stake { who, amount } => {
			let _ = GigaHdx::giga_stake(RawOrigin::Signed(acc(who)).into(), amount);
		}
		Op::Unstake { who, frac } => {
			let a = acc(who);
			if let Some(s) = Stakes::<Test>::get(a) {
				let amt = s.gigahdx / 4 * frac as Balance;
				if amt > 0 {
					let _ = GigaHdx::giga_unstake(RawOrigin::Signed(a).into(), amt);
				}
			}
		}
		Op::AccrueYield { amount } => {
			let _ = Balances::deposit_creating(&GigaHdx::gigapot_account_id(), amount);
		}
		Op::RealizeYield { who } => realize_checked(acc(who)),
		Op::Unlock { who } => {
			let a = acc(who);
			if let Some((id, _)) = crate::PendingUnstakes::<Test>::iter_prefix(a).next() {
				let target = (id + GigaHdxCooldownPeriod::get() + 1).max(frame_system::Pallet::<Test>::block_number());
				frame_system::Pallet::<Test>::set_block_number(target);
				let _ = GigaHdx::unlock(RawOrigin::Signed(a).into(), id);
			}
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(64))]

	/// Random op sequences keep every global invariant after every step.
	#[test]
	fn invariants_hold_under_random_op_sequences(ops in prop::collection::vec(op_strategy(), 1..24)) {
		ExtBuilder::default().build().execute_with(|| {
			assert_global_invariants();
			for op in &ops {
				apply(op);
				assert_global_invariants();
			}
		});
	}

	/// INV10 — value-neutrality: realize_yield then full unstake yields the
	/// same total HDX (±1) as a direct full unstake from an identical state.
	#[test]
	fn realize_then_unstake_is_value_neutral(
		prefix in prop::collection::vec(op_strategy(), 0..12),
		seed in ONE..=200 * ONE,
	) {
		let run = |realize_first: bool| -> Balance {
			let mut total = 0;
			ExtBuilder::default().build().execute_with(|| {
				// Guarantee ALICE has a position to unstake.
				let _ = GigaHdx::giga_stake(RawOrigin::Signed(ALICE).into(), seed);
				for op in &prefix {
					apply(op);
				}
				if realize_first {
					let _ = GigaHdx::realize_yield(RawOrigin::Signed(ALICE).into());
				}
				if let Some(s) = Stakes::<Test>::get(ALICE) {
					if s.gigahdx > 0 {
						let _ = GigaHdx::giga_unstake(RawOrigin::Signed(ALICE).into(), s.gigahdx);
					}
				}
				total = Balances::total_balance(&ALICE);
			});
			total
		};

		let without = run(false);
		let with = run(true);
		let diff = without.abs_diff(with);
		prop_assert!(diff <= 1, "INV10 value drift: without={without} with={with} diff={diff}");
	}
}
