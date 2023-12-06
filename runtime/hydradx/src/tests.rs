use crate::*;
use primitives::constants::{
	currency::{CENTS, DOLLARS, MILLICENTS},
	time::{DAYS, HOURS},
};

use pallet_transaction_payment::Multiplier;

use codec::Encode;
use frame_support::{
	dispatch::{DispatchClass, GetDispatchInfo},
	sp_runtime::{traits::Convert, FixedPointNumber},
	weights::WeightToFee,
};
use sp_runtime::BuildStorage;

#[test]
#[ignore]
// TODO needs to be redesigned to not break after benchmarking
fn full_block_cost() {
	let max_bytes = *BlockLength::get().max.get(DispatchClass::Normal) as u128;
	let length_fee = max_bytes * TransactionByteFee::get();
	assert_eq!(length_fee, 39_321_600_000_000_000);

	let max_weight = BlockWeights::get()
		.get(DispatchClass::Normal)
		.max_total
		.unwrap_or(Weight::from_parts(1, 0));
	let weight_fee = crate::WeightToFee::weight_to_fee(&max_weight);
	assert_eq!(weight_fee, 375_600_961_538_250);

	let target_fee = 396 * DOLLARS + 97_201_061_378_250;

	assert_eq!(
		ExtrinsicBaseWeight::get().ref_time() as u128 + length_fee + weight_fee,
		target_fee
	);
}

#[test]
#[ignore]
// This function tests that the fee for `ExtrinsicBaseWeight` of weight is correct
fn extrinsic_base_fee_is_correct() {
	let base_fee = crate::WeightToFee::weight_to_fee(&ExtrinsicBaseWeight::get());
	let base_fee_expected = CENTS / 10;
	assert!(base_fee.max(base_fee_expected) - base_fee.min(base_fee_expected) < MILLICENTS);
}

#[test]
#[ignore]
// Useful to calculate how much single transfer costs in native currency with fee components breakdown
fn transfer_cost() {
	let call = pallet_balances::Call::<Runtime>::transfer {
		dest: AccountId::new([0; 32]),
		value: Default::default(),
	};
	let info = call.get_dispatch_info();
	// convert to outer call
	let call = RuntimeCall::Balances(call);
	let len = call.using_encoded(|e| e.len()) as u32;

	let mut ext = sp_io::TestExternalities::new_empty();
	ext.execute_with(|| {
		pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_integer(1));
		let fee_raw = TransactionPayment::compute_fee_details(len, &info, 0);
		let fee = fee_raw.final_fee();
		println!(
			"len = {:?} // weight = {:?} // base fee = {:?} // len fee = {:?} // adjusted weight_fee = {:?} // full transfer fee = {:?}\n",
			len,
			info.weight,
			fee_raw.inclusion_fee.clone().unwrap().base_fee,
			fee_raw.inclusion_fee.clone().unwrap().len_fee,
			fee_raw.inclusion_fee.unwrap().adjusted_weight_fee,
			fee,
		);
	});
}

fn run_with_system_weight<F>(w: Weight, mut assertions: F)
where
	F: FnMut(),
{
	let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap()
		.into();
	t.execute_with(|| {
		System::set_block_consumed_resources(w, 0);
		assertions()
	});
}

#[test]
#[ignore]
fn multiplier_can_grow_from_zero() {
	let minimum_multiplier = MinimumMultiplier::get();
	let target = TargetBlockFullness::get() * BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	// if the min is too small, then this will not change, and we are doomed forever.
	// the weight is 1/100th bigger than target.
	run_with_system_weight(target * 101 / 100, || {
		let next = SlowAdjustingFeeUpdate::<Runtime>::convert(minimum_multiplier);
		assert!(next > minimum_multiplier, "{next:?} !>= {minimum_multiplier:?}");
	})
}

#[test]
#[ignore]
fn multiplier_growth_simulator() {
	// calculate the value of the fee multiplier after one hour of operation with fully loaded blocks
	let mut multiplier = Multiplier::saturating_from_integer(1);
	let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	for _block_num in 1..=HOURS {
		run_with_system_weight(block_weight, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			// ensure that it is growing as well.
			assert!(next > multiplier, "{next:?} !>= {multiplier:?}");
			multiplier = next;
		});
	}
	println!("multiplier = {multiplier:?}");
}

#[test]
#[ignore]
fn max_multiplier() {
	// calculate the value of the fee multiplier after one hour of operation with fully loaded blocks
	let max_multiplier = MaximumMultiplier::get();
	let mut multiplier = Multiplier::saturating_from_integer(1);
	let block_weight = BlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
	for _block_num in 1..=10 * DAYS {
		run_with_system_weight(block_weight, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(multiplier);
			// ensure that the multiplier is not greater than max_multiplier
			assert!(next <= max_multiplier, "{next:?} !<= {max_multiplier:?}");
			multiplier = next;
		});
	}
}
