use hydradx_traits::ice::{AmmState, AssetInfo, OmnipoolAssetInfo};
use hydradx_traits::Inspect;
use primitives::AssetId;
use sp_runtime::Permill;
use sp_std::vec;
use sp_std::vec::Vec;

/// Provides state of all AMM pools in Hydration.
pub struct GlobalAmmState<T>(sp_std::marker::PhantomData<T>);

impl<T> AmmState<AssetId> for GlobalAmmState<T>
where
	T: pallet_omnipool::Config<AssetId = AssetId>
		+ pallet_asset_registry::Config<AssetId = AssetId>
		+ pallet_dynamic_fees::Config<Fee = Permill, AssetId = AssetId>,
	<T as pallet_omnipool::Config>::AssetId: From<AssetId>,
	<T as pallet_asset_registry::Config>::AssetId: From<<T as pallet_omnipool::Config>::AssetId> + From<AssetId>,
	AssetId: From<<T as pallet_omnipool::Config>::AssetId>,
{
	fn state<F: Fn(&AssetId) -> bool>(retain: F) -> Vec<AssetInfo<AssetId>> {
		// Get state of omnipool
		let mut assets = vec![];
		for (asset_id, state) in pallet_omnipool::Pallet::<T>::omnipool_state() {
			if !retain(&asset_id) {
				continue;
			}
			let decimals = pallet_asset_registry::Pallet::<T>::decimals(asset_id.into()).unwrap();
			let (asset_fee, hub_fee) = pallet_dynamic_fees::Pallet::<T>::get_fee(asset_id);
			assets.push(AssetInfo::Omnipool(OmnipoolAssetInfo {
				asset_id: asset_id.into(),
				reserve: state.reserve,
				hub_reserve: state.hub_reserve,
				decimals,
				fee: asset_fee,
				hub_fee,
			}));
		}
		assets
		// TODO: add state of all stableswap pools
	}
}
