use crate::types::AssetId;
use crate::types::FloatType;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) struct OmnipoolAsset {
	pub asset_id: AssetId,
	pub decimals: u8,
	pub reserve: FloatType,
	pub hub_reserve: FloatType,
	pub fee: FloatType,
	pub protocol_fee: FloatType,
	pub hub_price: FloatType,
}

#[derive(Debug, Clone)]
pub(crate) struct Stablepool {
	pub pool_id: AssetId,
	pub assets: Vec<AssetId>,
	pub reserves: Vec<FloatType>,
	pub fee: FloatType,
}

pub(crate) struct AssetInfo {
	pub decimals: u8,
}

pub(crate) struct AmmStore {
	pub(crate) omnipool: BTreeMap<AssetId, OmnipoolAsset>,
	stablepools: BTreeMap<AssetId, Stablepool>,
	assets: BTreeMap<AssetId, AssetInfo>,
}

pub(crate) fn process_data(info: Vec<crate::types::Asset>) -> AmmStore {
	let mut omnipool = BTreeMap::new();
	let mut stablepools: BTreeMap<AssetId, Stablepool> = BTreeMap::new();
	let mut assets = BTreeMap::new();
	for asset in info {
		match asset {
			crate::types::Asset::StableSwap(asset) => {
				let pool_id = asset.pool_id;
				let asset_id = asset.asset_id;
				let decimals = asset.decimals;
				let reserve = asset.reserve as f64 / 10u128.pow(decimals as u32) as f64;
				let fee = asset.fee.0 as f64 / asset.fee.1 as f64;

				stablepools
					.entry(pool_id)
					.and_modify(|pool| {
						pool.assets.push(asset_id);
						pool.reserves.push(reserve);
					})
					.or_insert(Stablepool {
						pool_id,
						assets: vec![asset_id],
						reserves: vec![reserve],
						fee,
					});

				assert!(assets.get(&asset_id).is_none(), "Asset already in list of assets");
				assets.insert(asset_id, AssetInfo { decimals });
				assets.insert(pool_id, AssetInfo { decimals: 18 });
			}
			crate::types::Asset::Omnipool(asset) => {
				let asset_id = asset.asset_id;
				let decimals = asset.decimals;
				let reserve = asset.reserve_as_f64();
				let hub_reserve = asset.hub_reserve_as_f64();
				let fee = asset.fee_as_f64();
				let protocol_fee = asset.hub_fee_as_f64();
				let hub_price = if reserve > 0. { hub_reserve / reserve } else { 0. };
				let asset_data = OmnipoolAsset {
					asset_id,
					decimals,
					reserve,
					hub_reserve,
					fee,
					protocol_fee,
					hub_price,
				};
				omnipool.insert(asset_id, asset_data);

				assert!(assets.get(&asset_id).is_none(), "Asset already in list of assets");
				assets.insert(asset_id, AssetInfo { decimals });
			}
		}
	}
	AmmStore {
		omnipool,
		stablepools,
		assets,
	}
}
