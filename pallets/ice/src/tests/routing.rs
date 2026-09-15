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

#[test]
fn update_routing_should_fail_when_origin_is_not_authority() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			ICE::update_routing(RuntimeOrigin::signed(ALICE), RoutingTarget::OmnipoolAsset(100), true),
			BadOrigin
		);

		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::OmnipoolAsset(100)), None);
	});
}

#[test]
fn update_routing_should_store_exclusion_when_asset_is_excluded_from_omnipool() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::OmnipoolAsset(100),
			true
		));

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

/// Included is the default for a venue the solver enumerates itself, so undoing
/// an exclusion drops the entry rather than storing a redundant one.
#[test]
fn update_routing_should_remove_entry_when_a_self_discovered_target_is_included() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::StableswapPool(100),
			true
		));
		assert!(SolverRouting::<Test>::contains_key(RoutingTarget::StableswapPool(100)));

		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::StableswapPool(100),
			false
		));

		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::StableswapPool(100)), None);
		assert_eq!(
			last_ice_event(),
			Event::RoutingUpdated {
				target: RoutingTarget::StableswapPool(100),
				state: None,
			}
		);
	});
}

/// Uniswap and XYK are the exception: they are opt-in, so the included entry is
/// the registration and has to be kept.
#[test]
fn update_routing_should_keep_the_entry_when_a_uniswap_pool_is_included() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::UniswapV3Pool(POOL),
			false
		));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::UniswapV3Pool(POOL)),
			Some(RoutingState::Included)
		);
	});
}

#[test]
fn update_routing_should_store_exclusion_when_a_uniswap_pool_is_excluded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::UniswapV3Pool(POOL),
			true
		));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::UniswapV3Pool(POOL)),
			Some(RoutingState::Excluded)
		);
	});
}

#[test]
fn update_routing_should_normalize_pair_when_xyk_assets_are_unordered() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(200, 100),
			true
		));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::XykPool(100, 200)),
			Some(RoutingState::Excluded)
		);
		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::XykPool(200, 100)), None);
	});
}

#[test]
fn update_routing_should_keep_the_entry_when_an_xyk_pool_is_included() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(100, 200),
			false
		));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::XykPool(100, 200)),
			Some(RoutingState::Included)
		);
		assert_eq!(
			last_ice_event(),
			Event::RoutingUpdated {
				target: RoutingTarget::XykPool(100, 200),
				state: Some(RoutingState::Included),
			}
		);
	});
}

/// Registering with the pair the other way round must land on the same entry, or
/// a pool could be registered twice and loaded twice.
#[test]
fn update_routing_should_normalize_pair_when_an_unordered_xyk_pool_is_included() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(200, 100),
			false
		));

		assert_eq!(
			SolverRouting::<Test>::get(RoutingTarget::XykPool(100, 200)),
			Some(RoutingState::Included)
		);
		assert_eq!(SolverRouting::<Test>::get(RoutingTarget::XykPool(200, 100)), None);
	});
}
