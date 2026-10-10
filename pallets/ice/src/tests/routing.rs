use crate::tests::mock::*;
use crate::Event;
use crate::SolverRouting;
use frame_support::assert_noop;
use frame_support::assert_ok;
use ice_support::RoutingState;
use ice_support::RoutingTarget;
use pretty_assertions::assert_eq;
use sp_core::H160;
use sp_runtime::DispatchError::BadOrigin;

const POOL: H160 = H160::repeat_byte(0xab);

fn set(target: RoutingTarget, state: Option<RoutingState>) {
	assert_ok!(ICE::update_routing(RuntimeOrigin::root(), target, state));
}

fn batch(pairs: &[(u32, u32)]) -> RoutingTarget {
	RoutingTarget::XykPools(pairs.to_vec().try_into().unwrap())
}

#[test]
fn update_routing_should_fail_when_origin_is_not_authority() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			ICE::update_routing(
				RuntimeOrigin::signed(ALICE),
				RoutingTarget::OmnipoolAsset(100),
				Some(RoutingState::Excluded)
			),
			BadOrigin
		);

		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::OmnipoolAsset(100)), None);
	});
}

#[test]
fn update_routing_should_store_exclusion_when_asset_is_excluded_from_omnipool() {
	ExtBuilder::default().build().execute_with(|| {
		set(RoutingTarget::OmnipoolAsset(100), Some(RoutingState::Excluded));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::OmnipoolAsset(100)),
			Some(RoutingState::Excluded)
		);
		assert_eq!(
			last_ice_event(),
			Event::RoutingUpdated {
				target: RoutingTarget::OmnipoolAsset(100),
				state: Some(RoutingState::Excluded),
			}
		);
	});
}

#[test]
fn update_routing_should_store_registration_when_a_uniswap_pool_is_included() {
	ExtBuilder::default().build().execute_with(|| {
		set(RoutingTarget::UniswapV3Pool(POOL), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::UniswapV3Pool(POOL)),
			Some(RoutingState::Included)
		);
	});
}

/// `None` is the only way back to "no rule at all" — without it a veto could be
/// flipped but never lifted, and every one ever issued would be permanent.
#[test]
fn update_routing_should_remove_the_entry_when_state_is_none() {
	ExtBuilder::default().build().execute_with(|| {
		set(RoutingTarget::AaveWrap(5, 1001), Some(RoutingState::Excluded));
		assert!(SolverRouting::<Test>::contains_key(RoutingTarget::AaveWrap(5, 1001)));

		set(RoutingTarget::AaveWrap(5, 1001), None);

		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::AaveWrap(5, 1001)), None);
		assert_eq!(
			last_ice_event(),
			Event::RoutingUpdated {
				target: RoutingTarget::AaveWrap(5, 1001),
				state: None,
			}
		);
	});
}

#[test]
fn update_routing_should_remove_the_entry_when_a_batch_is_cleared() {
	ExtBuilder::default().build().execute_with(|| {
		set(batch(&[(100, 200)]), Some(RoutingState::Included));
		assert!(SolverRouting::<Test>::contains_key(batch(&[(100, 200)])));

		set(batch(&[(100, 200)]), None);

		assert_eq!(SolverRouting::<Test>::get(batch(&[(100, 200)])), None);
	});
}

#[test]
fn update_routing_should_store_the_batch_when_a_batch_is_registered() {
	ExtBuilder::default().build().execute_with(|| {
		set(batch(&[(100, 200), (300, 400)]), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(batch(&[(100, 200), (300, 400)])),
			Some(RoutingState::Included)
		);
	});
}

/// Two batches are two independent keys, so a venue is not capped by one batch —
/// overflowing into another key is how the per-key bound is meant to be handled.
#[test]
fn update_routing_should_keep_both_entries_when_two_batches_are_registered() {
	ExtBuilder::default().build().execute_with(|| {
		set(batch(&[(100, 200)]), Some(RoutingState::Included));
		set(batch(&[(300, 400)]), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(batch(&[(100, 200)])),
			Some(RoutingState::Included)
		);
		assert_eq!(
			SolverRouting::<Test>::get(batch(&[(300, 400)])),
			Some(RoutingState::Included)
		);
	});
}

#[test]
fn update_routing_should_normalize_pair_when_xyk_assets_are_unordered() {
	ExtBuilder::default().build().execute_with(|| {
		set(RoutingTarget::XykPool(200, 100), Some(RoutingState::Excluded));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::XykPool(100, 200)),
			Some(RoutingState::Excluded)
		);
		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::XykPool(200, 100)), None);
	});
}

/// The same set listed in a different order has to land on the same key, or one
/// batch could be registered twice and its pools loaded twice.
#[test]
fn update_routing_should_normalize_the_batch_when_members_are_unordered() {
	ExtBuilder::default().build().execute_with(|| {
		set(batch(&[(300, 400), (200, 100)]), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(batch(&[(100, 200), (300, 400)])),
			Some(RoutingState::Included)
		);
		assert_eq!(SolverRouting::<Test>::get(batch(&[(300, 400), (200, 100)])), None);
	});
}

#[test]
fn update_routing_should_deduplicate_the_batch_when_a_member_repeats() {
	ExtBuilder::default().build().execute_with(|| {
		set(batch(&[(100, 200), (100, 200)]), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(batch(&[(100, 200)])),
			Some(RoutingState::Included)
		);
	});
}

/// Aave wraps are directional — `(reserve, aToken)` is not the same wrap as
/// `(aToken, reserve)` — so normalisation must leave them alone.
#[test]
fn update_routing_should_not_normalize_the_pair_when_an_aave_wrap_is_registered() {
	ExtBuilder::default().build().execute_with(|| {
		set(RoutingTarget::AaveWrap(1001, 5), Some(RoutingState::Included));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::AaveWrap(1001, 5)),
			Some(RoutingState::Included)
		);
		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::AaveWrap(5, 1001)), None);
	});
}
