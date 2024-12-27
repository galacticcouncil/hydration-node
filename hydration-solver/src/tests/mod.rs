extern crate rand;

mod v3;

use crate::data::AssetData;
use crate::traits::{OmnipoolAssetInfo, TempOmnipoolAssetInfo};
use crate::types::FloatType;
use crate::types::Intent;
use rand::Rng;

const OMNIPOOL_DATA: &str = r##"[{"asset_id": 100, "reserve": 1392263929561840317724897, "hub_reserve": 50483454258911331, "decimals": 18, "fee": 2504, "hub_fee": 500, "symbol": "4-Pool"},{"asset_id": 0, "reserve": 140474254463930214441, "hub_reserve": 24725802166085100, "decimals": 12, "fee": 2500, "hub_fee": 500, "symbol": "HDX"},{"asset_id": 28, "reserve": 1941765870068803245372, "hub_reserve": 10802301353604526, "decimals": 15, "fee": 2500, "hub_fee": 500, "symbol": "KILT"},{"asset_id": 20, "reserve": 897820372708098091909, "hub_reserve": 82979992792480889, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "WETH"},{"asset_id": 101, "reserve": 80376407421087835272, "hub_reserve": 197326543312095758, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "2-Pool"},{"asset_id": 16, "reserve": 7389788325282889772690033, "hub_reserve": 44400113772627681, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "GLMR"},{"asset_id": 14, "reserve": 5294190655262755253, "hub_reserve": 35968107631988627, "decimals": 12, "fee": 2500, "hub_fee": 500, "symbol": "BNC"},{"asset_id": 31, "reserve": 30608622540452908043463002, "hub_reserve": 1996484382337770, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "RING"},{"asset_id": 33, "reserve": 1709768909360181457244842, "hub_reserve": 4292819030020081, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "vASTR"},{"asset_id": 15, "reserve": 8517557840315843, "hub_reserve": 182410990007273071, "decimals": 10, "fee": 2500, "hub_fee": 500, "symbol": "vDOT"},{"asset_id": 13, "reserve": 3497639039771749578811390, "hub_reserve": 41595576892166959, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "CFG"},{"asset_id": 27, "reserve": 337868268274751003, "hub_reserve": 4744442135139952, "decimals": 12, "fee": 2500, "hub_fee": 500, "symbol": "CRU"},{"asset_id": 102, "reserve": 14626788977583803950815838, "hub_reserve": 523282707224236528, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "2-Pool"},{"asset_id": 5, "reserve": 23699654990946855, "hub_reserve": 363516483882480814, "decimals": 10, "fee": 2500, "hub_fee": 500, "symbol": "DOT"},{"asset_id": 8, "reserve": 6002455470581388547, "hub_reserve": 24099247547699764, "decimals": 12, "fee": 2500, "hub_fee": 500, "symbol": "PHA"},{"asset_id": 12, "reserve": 97076438291619355, "hub_reserve": 4208903658046130, "decimals": 10, "fee": 2500, "hub_fee": 500, "symbol": "ZTG"},{"asset_id": 17, "reserve": 527569284895074643, "hub_reserve": 19516483401186167, "decimals": 10, "fee": 2500, "hub_fee": 500, "symbol": "INTR"},{"asset_id": 9, "reserve": 31837859712733867027462915, "hub_reserve": 68571523757927389, "decimals": 18, "fee": 2500, "hub_fee": 500, "symbol": "ASTR"}]"##;

pub const ALICE: [u8; 32] = [4u8; 32];

type AssetId = u32;

pub(crate) fn load_omnipool_data() -> Vec<OmnipoolAssetInfo<AssetId>> {
	let d: Vec<TempOmnipoolAssetInfo<AssetId>> = serde_json::from_str(OMNIPOOL_DATA).unwrap();

	d.into_iter()
		.map(|a| OmnipoolAssetInfo {
			asset_id: a.asset_id,
			reserve: a.reserve,
			hub_reserve: a.hub_reserve,
			decimals: a.decimals,
			fee: (a.fee, 1000_000),
			hub_fee: (a.hub_fee, 1000_000),
		})
		.collect()
}

fn price(da: &OmnipoolAssetInfo<AssetId>, db: &OmnipoolAssetInfo<AssetId>, asset_a: AssetId, asset_b: AssetId) -> f64 {
	if asset_a == asset_b {
		1.
	} else if asset_b == 1u32 {
		da.hub_reserve as f64 / da.reserve as f64
	} else if asset_a == 1u32 {
		let p = db.hub_reserve as f64 / db.reserve as f64;
		1. / p
	} else {
		let p1 = db.reserve as f64 / db.hub_reserve as f64;
		let p2 = da.hub_reserve as f64 / da.reserve as f64;
		let r = p1 * p2;
		r
	}
}

pub(crate) fn generate_random_intents(c: u32, data: Vec<OmnipoolAssetInfo<AssetId>>) -> Vec<Intent> {
	let random_pair = || {
		let mut rng = rand::thread_rng();
		loop {
			let idx_in = rng.gen_range(0..data.len());
			let idx_out = rng.gen_range(0..data.len());
			if (idx_in == idx_out) {
				continue;
			}
			let reserve_in = data[idx_in].reserve;
			let reserve_out = data[idx_out].reserve;
			let amount_in = rng.gen_range(1..reserve_in / 4);
			let price = price(
				&data[idx_in],
				&data[idx_out],
				data[idx_in].asset_id,
				data[idx_out].asset_id,
			);
			let p = 0.9f64;
			let amount_out = (price * amount_in as f64) as u128;
			let amount_out = (p * amount_out as f64) as u128;
			return (data[idx_in].asset_id, data[idx_out].asset_id, amount_in, amount_out);
		}
	};

	let mut intents = Vec::new();
	let mut rng = rand::thread_rng();
	for i in 0..c {
		let (asset_in, asset_out, amount_in, amount_out) = random_pair();
		let partial = if i < c / 2 { true } else { false };
		intents.push(Intent {
			intent_id: i as u128,
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial,
		});
	}
	intents
}

pub(crate) struct DataProvider;

impl DataProvider {
	fn assets(filter: Option<Vec<AssetId>>) -> Vec<OmnipoolAssetInfo<AssetId>> {
		let d = load_omnipool_data();
		if let Some(filtered_assets) = filter {
			d.into_iter()
				.filter(|a| filtered_assets.contains(&a.asset_id))
				.collect()
		} else {
			d
		}
	}
}

#[test]
fn test_data_provider() {
	let d = DataProvider::assets(None);
	assert_eq!(d.len(), 18);
	let d = DataProvider::assets(Some(vec![0, 27]));
	assert_eq!(d.len(), 2);
	assert_eq!(d[0].asset_id, 0);
	assert_eq!(d[1].asset_id, 27);
}

#[test]
fn test_generate_intents() {
	let d = DataProvider::assets(None);
	let intents = generate_random_intents(1, d);
	assert_eq!(intents.len(), 1);
}
