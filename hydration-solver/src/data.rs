use crate::problem::FloatType;
use pallet_ice::traits::OmnipoolAssetInfo;
use primitives::AssetId;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub(crate) struct AssetData {
	pub asset_id: AssetId,
	pub decimals: u8,
	pub reserve: FloatType,
	pub hub_reserve: FloatType,
	pub fee: FloatType,
	pub protocol_fee: FloatType,
}

pub(crate) fn process_omnipool_data(info: Vec<OmnipoolAssetInfo<AssetId>>) -> BTreeMap<AssetId, AssetData> {
	let mut r = BTreeMap::new();
	for asset in info {
		let asset_id = asset.asset_id;
		let decimals = asset.decimals;
		let reserve = asset.reserve_as_f64();
		let hub_reserve = asset.hub_reserve_as_f64();
		let fee = asset.fee_as_f64();
		let protocol_fee = asset.hub_fee_as_f64();
		let asset_data = AssetData {
			asset_id,
			decimals,
			reserve,
			hub_reserve,
			fee,
			protocol_fee,
		};
		r.insert(asset_id, asset_data);
	}
	r
}
