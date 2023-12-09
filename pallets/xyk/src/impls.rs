use crate::types::{AssetId, AssetPair, Price};
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use sp_runtime::FixedPointNumber;
use sp_std::marker::PhantomData;

pub struct XYKSpotPrice<T>(PhantomData<T>);

impl<T: crate::Config> SpotPriceProvider<AssetId> for XYKSpotPrice<T> {
	type Price = Price;

	fn pair_exists(asset_a: AssetId, asset_b: AssetId) -> bool {
		<crate::Pallet<T>>::exists(AssetPair::new(asset_b, asset_a))
	}

	fn spot_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if Self::pair_exists(asset_a, asset_b) {
			let pair_account = <crate::Pallet<T>>::get_pair_id(AssetPair {
				asset_out: asset_a,
				asset_in: asset_b,
			});
			let asset_a_reserve = T::Currency::free_balance(asset_a, &pair_account);
			let asset_b_reserve = T::Currency::free_balance(asset_b, &pair_account);

			Price::checked_from_rational(asset_a_reserve, asset_b_reserve)
		} else {
			None
		}
	}
}
