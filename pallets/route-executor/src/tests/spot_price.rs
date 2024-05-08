use crate::tests::mock::*;
use crate::{Error, Trade};
use frame_support::pallet_prelude::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::router::RouteSpotPriceProvider;
use hydradx_traits::router::{AssetPair, PoolType};
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;

#[test]
fn price_should_be_none_for_empty_route() {
	ExtBuilder::default().build().execute_with(|| {
		let price = Router::spot_price_with_fee(&vec![]);

		assert!(price.is_none());
	});
}
