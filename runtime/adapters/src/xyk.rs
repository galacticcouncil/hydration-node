use hydradx_traits::CanCreatePool;
use hydradx_traits::Registry;
use primitives::{AssetId, Balance};
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

pub struct AllowPoolCreation<LBP, R>(sp_std::marker::PhantomData<(LBP, R)>);

impl<LBP, R> CanCreatePool<AssetId> for AllowPoolCreation<LBP, R>
where
	LBP: pallet_lbp::Config,
	R: Registry<AssetId, Vec<u8>, Balance, DispatchError>,
{
	fn can_create(asset_a: AssetId, asset_b: AssetId) -> bool {
		let Some(asset_a_type) = R::retrieve_asset_type(asset_a).ok() else {
            return false;
        };
		if asset_a_type == hydradx_traits::AssetKind::XYK {
			return false;
		}
		let Some(asset_b_type) = R::retrieve_asset_type(asset_b).ok() else {
            return false;
        };
		if asset_b_type == hydradx_traits::AssetKind::XYK {
			return false;
		}
		pallet_lbp::DisallowWhenLBPPoolRunning::<LBP>::can_create(asset_a, asset_b)
	}
}
