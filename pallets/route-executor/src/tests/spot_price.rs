use crate::tests::mock::*;
use hydradx_traits::router::RouteSpotPriceProvider;

#[test]
fn price_should_be_none_for_empty_route() {
	ExtBuilder::default().build().execute_with(|| {
		let price = Router::spot_price_with_fee(&[]);

		assert!(price.is_none());
	});
}
