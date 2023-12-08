use sp_std::marker::PhantomData;
use crate::{Config};
use hydradx_traits::price::PriceProvider;
use sp_runtime::traits::{CheckedMul, Get, One};
use sp_runtime::{FixedPointNumber, FixedU128};

pub struct OmnipoolSpotPriceProvider<T>(PhantomData<T>);

impl<T: Config> PriceProvider<T::AssetId> for OmnipoolSpotPriceProvider<T> {
	type Price = FixedU128;

	fn get_price(asset_a: T::AssetId, asset_b: T::AssetId) -> Option<Self::Price> {
        if asset_a == asset_b {
            return Some(FixedU128::one());
        }
		if asset_a == T::HubAssetId::get() {
			let asset_b = crate::Pallet::<T>::load_asset_state(asset_b).ok()?;
			FixedU128::checked_from_rational(asset_b.hub_reserve, asset_b.reserve)
		} else if asset_b == T::HubAssetId::get() {
			let asset_a = crate::Pallet::<T>::load_asset_state(asset_a).ok()?;
			FixedU128::checked_from_rational(asset_a.reserve, asset_a.hub_reserve)
		} else {
			let asset_a = crate::Pallet::<T>::load_asset_state(asset_a).ok()?;
			let asset_b = crate::Pallet::<T>::load_asset_state(asset_b).ok()?;
			// (A / LRNA) * (LRNA / B) = A / B
			let price_a = FixedU128::checked_from_rational(asset_a.reserve, asset_a.hub_reserve)?;
			let price_b = FixedU128::checked_from_rational(asset_b.hub_reserve, asset_b.reserve)?;
			price_a.checked_mul(&price_b)
		}
	}
}