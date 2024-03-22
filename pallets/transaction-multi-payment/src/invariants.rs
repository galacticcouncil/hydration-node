use super::*;
use crate::mock::Balance;
use crate::tests::ExtBuilder;
use crate::tests::*;
use frame_support::assert_ok;
use frame_support::dispatch::PostDispatchInfo;
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;
use proptest::prelude::Strategy;
use proptest::prelude::*;
use sp_runtime::traits::SignedExtension;

pub const ONE: Balance = 1_000_000_000_000;

fn length() -> impl Strategy<Value = usize> {
	1..2000usize
}

fn tips() -> impl Strategy<Value = Balance> {
	10000..100 * ONE
}

fn weight() -> impl Strategy<Value = u64> {
	10000..100_000_000_000u64
}

const CALL: &<Test as frame_system::Config>::RuntimeCall =
	&RuntimeCall::Balances(BalancesCall::transfer { dest: BOB, value: 69 });

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn issuance_should_hold(len in length(), tip in tips(), pre_weight in weight(), post_weight in weight()) {
		ExtBuilder::default()
			.build()
			.execute_with(|| {
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				CHARLIE,
				1000 * ONE,
			));
			let dispatch_info = info_from_weight(Weight::from_parts(pre_weight, 0));
			let post_dispatch_info = post_info_from_weight(Weight::from_parts(post_weight, 0));
			let previous_total_issuance = Balances::total_issuance();

			// Act
			let pre = ChargeTransactionPayment::<Test>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &dispatch_info, len)
				.unwrap();

			assert_ok!(ChargeTransactionPayment::<Test>::post_dispatch(
				Some(pre),
				&dispatch_info,
				&post_dispatch_info,
				len,
				&Ok(())
			));

			// Assert
			assert_eq!(Balances::total_issuance(), previous_total_issuance);
			});
	}
}
fn post_info_from_weight(w: Weight) -> PostDispatchInfo {
	PostDispatchInfo {
		actual_weight: Some(w),
		pays_fee: Default::default(),
	}
}
