#![cfg(test)]
use cumulus_pallet_xcmp_queue::WeightInfo;

use hydradx_runtime::weights::xcmp_queue::HydraWeight;
use hydradx_runtime::BlockWeights;
use hydradx_runtime::{MaxBucketsProcessed, MaxDeferredMessages, ReservedXcmpWeight};

#[test]
fn xcmp_operations_should_fit_in_weight_budget() {
	assert!(
		HydraWeight::<hydradx_runtime::Runtime>::try_place_in_deferred_queue(MaxDeferredMessages::get())
			.all_lte(ReservedXcmpWeight::get()),
		"placing in deferred queue should fit in weight budget"
	);
	assert!(
		HydraWeight::<hydradx_runtime::Runtime>::service_deferred(MaxBucketsProcessed::get())
			.all_lte(ReservedXcmpWeight::get()),
		"processing deferred queue should fit in weight budget"
	);
	// We take half the block weight as an arbitrary upper number for a reasonable weight here.
	let half_block = BlockWeights::get().max_block / 2;
	assert!(
		HydraWeight::<hydradx_runtime::Runtime>::discard_deferred_bucket(MaxDeferredMessages::get())
			.all_lte(half_block),
		"discarding deferred messages should fit in block weight budget"
	);
	assert!(
		HydraWeight::<hydradx_runtime::Runtime>::discard_deferred_individual(MaxDeferredMessages::get())
			.all_lte(half_block),
		"discarding deferred messages should fit in block weight budget"
	);
}
