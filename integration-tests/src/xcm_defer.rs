#![cfg(test)]
use crate::polkadot_test_net::*;

use cumulus_pallet_xcmp_queue::WeightInfo;
use frame_support::{assert_noop, assert_ok};

use hydradx_runtime::weights::xcmp_queue::HydraWeight;
use hydradx_runtime::{MaxBucketsProcessed, MaxDeferredMessages, ReservedXcmpWeight};
use hydradx_runtime::BlockWeights;

#[test]
fn xcmp_operations_should_fit_in_weight_budget() {
	assert!(HydraWeight::<hydradx_runtime::Runtime>::try_place_in_deferred_queue(MaxDeferredMessages::get()).all_lte(ReservedXcmpWeight::get()), "placing in deferred queue should fit in weight budget");
    assert!(HydraWeight::<hydradx_runtime::Runtime>::service_deferred(MaxBucketsProcessed::get()).all_lte(ReservedXcmpWeight::get()), "processing deferred queue should fit in weight budget");
    // We take half the block weight as an arbitrary upper number for a reasonable weight here.
    let half_block = BlockWeights::get().max_block / 2;
    assert!(HydraWeight::<hydradx_runtime::Runtime>::discard_deferred_bucket(MaxDeferredMessages::get()).all_lte(half_block), "discarding deferred messages should fit in block weight budget");
    assert!(HydraWeight::<hydradx_runtime::Runtime>::discard_deferred_individual(MaxDeferredMessages::get()).all_lte(half_block), "discarding deferred messages should fit in block weight budget");
}


// #[test]
// fn hydra_should_receive_asset_when_transferred_from_polkadot_relay_chain() {
// 	//Arrange
// 	Hydra::execute_with(|| {
// 		assert_ok!(hydradx_runtime::AssetRegistry::set_location(
// 			hydradx_runtime::RuntimeOrigin::root(),
// 			1,
// 			hydradx_runtime::AssetLocation(MultiLocation::parent())
// 		));
// 	});

// 	PolkadotRelay::execute_with(|| {
// 		//Act
// 		assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
// 			polkadot_runtime::RuntimeOrigin::signed(ALICE.into()),
// 			Box::new(Parachain(HYDRA_PARA_ID).into_versioned()),
// 			Box::new(Junction::AccountId32 { id: BOB, network: None }.into()),
// 			Box::new((Here, 300 * UNITS).into()),
// 			0,
// 		));

// 		//Assert
// 		assert_eq!(
// 			polkadot_runtime::Balances::free_balance(&ParaId::from(HYDRA_PARA_ID).into_account_truncating()),
// 			310 * UNITS
// 		);
// 	});

// 	let fees = 401884032343;
// 	Hydra::execute_with(|| {
// 		assert_eq!(
// 			hydradx_runtime::Tokens::free_balance(1, &AccountId::from(BOB)),
// 			BOB_INITIAL_NATIVE_BALANCE + 300 * UNITS - fees
// 		);
// 		assert_eq!(
// 			hydradx_runtime::Tokens::free_balance(1, &hydradx_runtime::Treasury::account_id()),
// 			fees
// 		);
// 	});
// }
