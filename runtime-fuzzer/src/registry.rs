use hydradx_runtime::RuntimeCall;
use serde::Deserialize;
use std::fs;
use toml;

#[derive(Debug, Deserialize)]
struct Asset {
	symbol: String,
	decimals: u32,
	asset_id: u32,
}

#[derive(Debug, Deserialize)]
pub struct RegistrySetup {
	asset: Vec<Asset>,
}

fn load_setup(filename: &str) -> RegistrySetup {
	let toml_str = fs::read_to_string(filename).expect("Failed to read omnipool.toml file");
	toml::from_str(&toml_str).expect("Failed to deserialize OmnipoolSetup")
}

impl RegistrySetup {
	fn new(filename: &str) -> Self {
		load_setup(filename)
	}
	pub fn assets(&self) -> Vec<(Vec<u8>, u32)> {
		let mut results = Vec::new();
		for asset in self.asset.iter() {
			results.push((asset.symbol.clone().into(), asset.asset_id));
		}
		results
	}

	pub fn calls(&self) -> Vec<RuntimeCall> {
		let mut calls = Vec::new();
		for asset in self.asset.iter() {
			let call = RuntimeCall::AssetRegistry(pallet_asset_registry::Call::set_metadata {
				asset_id: asset.asset_id,
				symbol: asset.symbol.as_bytes().to_vec(),
				decimals: asset.decimals as u8,
			});
			calls.push(call);
		}

		calls
	}
}

pub fn registry_state() -> RegistrySetup {
	RegistrySetup::new("data/registry.toml")
}
