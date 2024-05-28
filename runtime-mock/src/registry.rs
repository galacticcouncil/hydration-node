use hydradx_runtime::{AssetId, AssetRegistryConfig, Balance, RegistryStrLimit};
use pallet_asset_registry::{Name, Symbol};
use primitives::constants::currency::NATIVE_EXISTENTIAL_DEPOSIT;
use serde::Deserialize;
use sp_runtime::BoundedVec;
use std::fs;

const TOKEN_SYMBOL: &str = "HDX";
const NATIVE_DECIMALS: u8 = 12;

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
	pub asset_id: Option<AssetId>,
	pub symbol: String,
	pub decimals: Option<u8>,
	pub is_sufficient: bool,
}

pub type RegisteredAsset = (
	Option<AssetId>,
	Option<Name<RegistryStrLimit>>,
	Balance,
	Option<Symbol<RegistryStrLimit>>,
	Option<u8>,
	Option<Balance>,
	bool,
);

pub type BoundedName = BoundedVec<u8, RegistryStrLimit>;

#[derive(Debug, Deserialize)]
pub struct AssetRegistrySetup {
	pub assets: Vec<Asset>,
}

impl AssetRegistrySetup {
	pub fn new() -> Self {
		let filename = "data/registry.toml";
		let toml_str = fs::read_to_string(filename).expect("Failed to read registry.toml file");
		toml::from_str(&toml_str).expect("Failed to deserialize RegistrySetup")
	}

	pub fn registered_assets(self) -> Vec<RegisteredAsset> {
		self.assets
			.iter()
			.filter(|asset| asset.asset_id != Some(0))
			.map(|asset| asset.clone().into())
			.collect()
	}

	pub fn config(self) -> AssetRegistryConfig {
		AssetRegistryConfig {
			registered_assets: self.registered_assets(),
			native_asset_name: TOKEN_SYMBOL
				.as_bytes()
				.to_vec()
				.try_into()
				.expect("Native asset name is too long."),
			native_existential_deposit: NATIVE_EXISTENTIAL_DEPOSIT,
			native_symbol: TOKEN_SYMBOL
				.as_bytes()
				.to_vec()
				.try_into()
				.expect("Native symbol is too long."),
			native_decimals: NATIVE_DECIMALS,
		}
	}
}

impl From<Asset> for RegisteredAsset {
	fn from(a: Asset) -> RegisteredAsset {
		(
			a.asset_id,
			Some(BoundedName::try_from(format!("Name {}", a.asset_id.unwrap()).as_bytes().to_vec()).unwrap()),
			1_000u128,
			Some(BoundedName::try_from(a.symbol.as_bytes().to_vec()).unwrap()),
			a.decimals,
			None,
			a.is_sufficient,
		)
	}
}
