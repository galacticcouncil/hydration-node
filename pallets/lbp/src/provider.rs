use crate::{AssetId, AssetPair, Config, Pallet, PoolData};
use hydra_dx_math::types::Price;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use sp_runtime::traits::{BlockNumberProvider, One};
use sp_runtime::{FixedPointNumber, FixedU128};

impl<T: Config> SpotPriceProvider<AssetId> for Pallet<T> {
	type Price = FixedU128;

	fn pair_exists(asset_a: AssetId, asset_b: AssetId) -> bool {
		Self::exists(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		})
	}

	fn spot_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(FixedU128::one());
		}

		if Self::pair_exists(asset_a, asset_b) {
			let pair_account = <crate::Pallet<T>>::get_pair_id(AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			});

			let now = T::BlockNumberProvider::current_block_number();
			let pool_data = <PoolData<T>>::try_get(&pair_account).ok()?;
			let (weight_in, weight_out) = Self::get_sorted_weight(asset_a, now, &pool_data).ok()?;

			let asset_in_reserve = T::MultiCurrency::free_balance(asset_a, &pair_account);
			let asset_out_reserve = T::MultiCurrency::free_balance(asset_b, &pair_account);

			let n = asset_in_reserve.checked_mul(weight_out.into())?;
			let d = asset_out_reserve.checked_mul(weight_in.into())?;

			Price::checked_from_rational(n, d)
		} else {
			None
		}
	}
}
