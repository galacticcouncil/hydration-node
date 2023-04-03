use crate::pallet::Assets;
use crate::{Config, Pallet};
use hydradx_traits::pools::SpotPriceProvider;
use sp_runtime::traits::{CheckedMul, Get};
use sp_runtime::{FixedPointNumber, FixedU128};

impl<T: Config> SpotPriceProvider<T::AssetId> for Pallet<T> {
	type Price = FixedU128;

	fn pair_exists(asset_a: T::AssetId, asset_b: T::AssetId) -> bool {
		<Assets<T>>::get(asset_a).is_some() && <Assets<T>>::get(asset_b).is_some()
	}

	fn spot_price(asset_a: T::AssetId, asset_b: T::AssetId) -> Option<Self::Price> {
		if asset_a == T::HubAssetId::get() {
			let asset_b = Self::load_asset_state(asset_b).ok()?;
			asset_b.price()
		} else if asset_b == T::HubAssetId::get() {
			let asset_a = Self::load_asset_state(asset_a).ok()?;
			asset_a.price()
		} else {
			let asset_a = Self::load_asset_state(asset_a).ok()?;
			let asset_b = Self::load_asset_state(asset_b).ok()?;
			let price_a = FixedU128::checked_from_rational(asset_a.hub_reserve, asset_a.reserve)?;
			let price_b = FixedU128::checked_from_rational(asset_b.reserve, asset_b.hub_reserve)?;
			price_a.checked_mul(&price_b)
		}
	}
}
