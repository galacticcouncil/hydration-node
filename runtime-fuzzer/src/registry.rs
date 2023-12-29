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
	let toml_str = fs::read_to_string(filename).expect("Failed to read registry.toml file");
	toml::from_str(&toml_str).expect("Failed to deserialize RegistrySetup")
}

impl RegistrySetup {
	fn new(filename: &str) -> Self {
		load_setup(filename)
	}

	pub fn asset_decimals(&self) -> Vec<(u32, u8)> {
		self.asset
			.iter()
			.map(|asset| (asset.asset_id, asset.decimals as u8))
			.collect()
	}

	pub fn assets(&self) -> Vec<(Vec<u8>, u32)> {
		self.asset
			.iter()
			.map(|asset| (asset.symbol.clone().into(), asset.asset_id))
			.collect()
	}

	pub fn calls(&self) -> Vec<RuntimeCall> {
		self.asset
			.iter()
			.map(|asset| {
				RuntimeCall::AssetRegistry(pallet_asset_registry::Call::set_metadata {
					asset_id: asset.asset_id,
					symbol: asset.symbol.as_bytes().to_vec(),
					decimals: asset.decimals as u8,
				})
			})
			.collect()
	}
}

pub fn registry_state() -> RegistrySetup {
	RegistrySetup::new("data/registry.toml")
}
