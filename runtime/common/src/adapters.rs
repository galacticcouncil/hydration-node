use hydradx_traits::pools::{AMMTrader, PriceProvider, SpotPriceProvider};
use sp_std::marker::PhantomData;

pub struct AmmTraderAdapter<T, Origin, AssetId, Balance>(PhantomData<(T, Origin, AssetId, Balance)>);

impl<T: pallet_omnipool::Config<AssetId = AssetId, Origin = Origin>, Origin, AssetId, Balance>
	AMMTrader<Origin, AssetId, Balance> for AmmTraderAdapter<T, Origin, AssetId, Balance>
where
	u128: core::convert::From<Balance>,
{
	fn sell(
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		min_buy_amount: Balance,
	) -> sp_runtime::DispatchResult {
		pallet_omnipool::Pallet::<T>::sell(
			origin,
			asset_in.into(),
			asset_out.into(),
			amount.into(),
			min_buy_amount.into(),
		)
	}

	fn buy(
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		max_sell_amount: Balance,
	) -> sp_runtime::DispatchResult {
		pallet_omnipool::Pallet::<T>::buy(
			origin,
			asset_out.into(),
			asset_in.into(),
			amount.into(),
			max_sell_amount.into(),
		)
	}
}

pub struct PriceProviderAdapter<T, AssetId>(PhantomData<(T, AssetId)>);

impl<T: SpotPriceProvider<AssetId>, AssetId> PriceProvider<AssetId> for PriceProviderAdapter<T, AssetId> {
	type Price = T::Price;

	fn spot_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		T::spot_price(asset_a, asset_b)
	}
}
