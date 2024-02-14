use crate::traits::{EnsurePriceWithin, ExternalPriceProvider, ShouldAllow};
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_ok, parameter_types};
use hydra_dx_math::ema::EmaPrice;
use sp_runtime::Permill;
use std::cell::RefCell;

thread_local! {
	pub static EXTERNAL_PRICE: RefCell<EmaPrice> = RefCell::new(EmaPrice::default());
}

struct SinglePriceProvider;

impl ExternalPriceProvider<u32, EmaPrice> for SinglePriceProvider {
	type Error = ();

	fn get_price(_asset_a: u32, _asset_b: u32) -> Result<EmaPrice, Self::Error> {
		Ok(EXTERNAL_PRICE.with(|v| *v.borrow()))
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
}

parameter_types! {
	pub const MaxAllowed: Permill = Permill::from_percent(1);
}

#[test]
fn ensure_price_should_be_ok_when_price_is_within_allowed_difference() {
	EXTERNAL_PRICE.with(|v| *v.borrow_mut() = EmaPrice::new(1, 10));
	let spot_price = EmaPrice::new(999, 10000);
	assert_ok!(EnsurePriceWithin::<u64, u32, SinglePriceProvider, MaxAllowed, ()>::ensure_price(&0, 1, 2, spot_price));

	let spot_price = EmaPrice::new(101, 1000);
	assert_ok!(EnsurePriceWithin::<u64, u32, SinglePriceProvider, MaxAllowed, ()>::ensure_price(&0, 1, 2, spot_price));
}

#[test]
fn ensure_price_should_fail_when_price_is_not_within_allowed_difference() {
	EXTERNAL_PRICE.with(|v| *v.borrow_mut() = EmaPrice::new(1, 10));
	let spot_price = EmaPrice::new(8, 1000);
	assert_err!(
		EnsurePriceWithin::<u64, u32, SinglePriceProvider, MaxAllowed, ()>::ensure_price(&0, 1, 2, spot_price),
		()
	);

	let spot_price = EmaPrice::new(2, 10);
	assert_err!(
		EnsurePriceWithin::<u64, u32, SinglePriceProvider, MaxAllowed, ()>::ensure_price(&0, 1, 2, spot_price),
		()
	);
}
