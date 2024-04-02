use crate::{AssetId, AssetPair, BalanceOf, Config, Pallet, PoolData};
use hydra_dx_math::types::Price;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use sp_runtime::traits::{BlockNumberProvider, CheckedMul, Get, One};
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
				asset_in: asset_a.clone(),
				asset_out: asset_b.clone(),
			});
			let asset_in_reserve = T::MultiCurrency::free_balance(asset_a.clone(), &pair_account);
			let asset_out_reserve = T::MultiCurrency::free_balance(asset_b.clone(), &pair_account);

			let pool_data = match <PoolData<T>>::try_get(&pair_account) {
				Ok(pool) => pool,
				Err(_) => return None,
			};

			let now = T::BlockNumberProvider::current_block_number();

			//TODO: check ordering, that might be not good for all case
			let (weight_in, weight_out) = match Self::get_sorted_weight(asset_a.clone(), now, &pool_data) {
				Ok(weights) => weights,
				Err(_) => return None,
			};

			let Some(d) = asset_out_reserve.checked_mul(weight_in.into()) else {
				return None;
			};

			let Some(n) = asset_in_reserve.checked_mul(weight_out.into()) else {
				return None;
			};

			Price::checked_from_rational(n, d)
		} else {
			None
		}
	}
}
