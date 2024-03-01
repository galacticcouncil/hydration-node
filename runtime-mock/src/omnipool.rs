use crate::traits::{Balance, TryExtrinsic};
use crate::AccountId;
use hydradx_runtime::RuntimeCall;
use serde::Deserialize;
use serde::Deserializer;
use sp_runtime::{FixedU128, Permill};
use std::fs;

#[derive(Debug, Deserialize)]
struct AssetConfig {
	asset_id: u32,
	#[serde(deserialize_with = "from_u128_str")]
	reserve: u128,
	#[serde(deserialize_with = "from_u128_str")]
	hub_reserve: u128,
}

#[derive(Debug, Deserialize)]
struct OmnipoolState {
	asset: Vec<AssetConfig>,
}

pub fn from_u128_str<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: String = Deserialize::deserialize(deserializer)?;
	Ok(s.parse::<u128>().unwrap())
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

struct OmnipoolPallet;

impl crate::traits::FuzzedPallet<RuntimeCall, u32, AccountId> for OmnipoolPallet {
	fn initial_calls(&self) -> Vec<RuntimeCall> {
		todo!()
	}

	fn native_endowed_accounts(&self) -> Vec<(AccountId, Balance)> {
		todo!()
	}

	fn foreign_endowed_accounts(&self) -> Vec<(AccountId, Vec<(u32, Balance)>)> {
		todo!()
	}
}

impl crate::traits::Loader for OmnipoolPallet {
	fn load_setup(_filename: &str) -> Self {
		todo!()
	}
}

pub struct OmnipoolHandler;

impl TryExtrinsic<RuntimeCall, u32> for OmnipoolHandler {
	fn try_extrinsic(&self, identifier: u8, data: &[u8], assets: &[u32]) -> Option<RuntimeCall> {
		match identifier {
			0 if data.len() > 18 => {
				let asset_in = assets[data[0] as usize % assets.len()];
				let asset_out = assets[data[1] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[2..18].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::sell {
					asset_in,
					asset_out,
					amount,
					min_buy_amount: 0,
				}))
			}
			1 if data.len() > 18 => {
				let asset_in = assets[data[0] as usize % assets.len()];
				let asset_out = assets[data[1] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[2..18].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::buy {
					asset_in,
					asset_out,
					amount,
					max_sell_amount: u128::MAX,
				}))
			}
			2 if data.len() > 17 => {
				let asset = assets[data[0] as usize % assets.len()];
				let amount = u128::from_ne_bytes(data[1..17].try_into().ok()?);
				Some(RuntimeCall::Omnipool(pallet_omnipool::Call::add_liquidity {
					asset,
					amount,
				}))
			}
			_ => None,
		}
	}
}
