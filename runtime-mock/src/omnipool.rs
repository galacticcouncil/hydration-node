use crate::AccountId;
use hydradx_runtime::RuntimeCall;
use serde::Deserialize;
use serde::Deserializer;
use sp_runtime::{FixedU128, Permill};
use std::fs;
use toml;

#[derive(Debug, Deserialize)]
struct AssetConfig {
	asset_id: u32,
	#[serde(deserialize_with = "from_u128_str")]
	reserve: u128,
	#[serde(deserialize_with = "from_u128_str")]
	hub_reserve: u128,
}

#[derive(Debug, Deserialize)]
struct Position {
	asset_id: String,
	#[serde(deserialize_with = "from_u128_str")]
	amount: u128,
}

#[derive(Debug, Deserialize)]
struct OmnipoolState {
	asset: Vec<AssetConfig>,
	position: Option<Vec<Position>>,
}

pub fn from_u128_str<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: String = Deserialize::deserialize(deserializer)?;
	Ok(u128::from_str_radix(&s, 10).unwrap())
}

fn load_setup(filename: &str) -> OmnipoolState {
	let toml_str = fs::read_to_string(filename).expect("Failed to read omnipool.toml file");
	toml::from_str(&toml_str).expect("Failed to deserialize OmnipoolSetup")
}

pub struct OmnipoolSetup {
	state: OmnipoolState,
}

impl OmnipoolSetup {
	fn new(filename: &str) -> Self {
		let state = load_setup(filename);
		Self { state }
	}

	pub fn get_omnipool_reserves(&self) -> (u128, Vec<(u32, u128)>) {
		let mut results = Vec::new();
		let mut native_reserve = 0u128;
		for asset in self.state.asset.iter() {
			if asset.asset_id == 0 {
				native_reserve = asset.reserve;
			} else {
				results.push((asset.asset_id, asset.reserve));
			}
		}
		(native_reserve, results)
	}

	pub fn calls(&self, owner: &AccountId) -> Vec<RuntimeCall> {
		self.state
			.asset
			.iter()
			.map(|asset| {
				let price = FixedU128::from_rational(asset.hub_reserve, asset.reserve);
				RuntimeCall::Omnipool(pallet_omnipool::Call::add_token {
					asset: asset.asset_id,
					initial_price: price,
					weight_cap: Permill::from_percent(100),
					position_owner: owner.clone(),
				})
			})
			.collect()
	}
}

pub fn omnipool_initial_state() -> OmnipoolSetup {
	OmnipoolSetup::new("data/omnipool.toml")
}
