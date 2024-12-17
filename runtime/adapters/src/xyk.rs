use hydradx_traits::CanCreatePool;
use hydradx_traits::Inspect;
use primitives::AssetId;

pub struct AllowPoolCreation<LBP, R>(sp_std::marker::PhantomData<(LBP, R)>);

impl<LBP, R> CanCreatePool<AssetId> for AllowPoolCreation<LBP, R>
where
	LBP: pallet_lbp::Config,
	R: Inspect<AssetId = AssetId>,
{
	fn can_create(asset_a: AssetId, asset_b: AssetId) -> bool {
		let Some(asset_a_type) = R::asset_type(asset_a) else {
			return false;
		};
		if asset_a_type == hydradx_traits::AssetKind::XYK {
			return false;
		}
		let Some(asset_b_type) = R::asset_type(asset_b) else {
			return false;
		};
		if asset_b_type == hydradx_traits::AssetKind::XYK {
			return false;
		}
		pallet_lbp::DisallowWhenLBPPoolRunning::<LBP>::can_create(asset_a, asset_b)
	}
}
