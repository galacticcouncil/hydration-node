use std::fs;
use serde::Deserialize;
use serde::Deserializer;
use toml;

#[derive(Debug, Deserialize)]
struct AssetConfig {
    symbol: String,
    asset_id: u32,
	#[serde(deserialize_with = "from_u128_str")]
    reserve: u128,
    #[serde(deserialize_with = "from_u128_str")]
    hub_reserve: u128,
}

#[derive(Debug, Deserialize)]
struct Position{
    asset_id: String,
	#[serde(deserialize_with = "from_u128_str")]
    amount: u128,
}

#[derive(Debug, Deserialize)]
struct OmnipoolSetup{
    asset: Vec<AssetConfig>,
    position: Option<Vec<Position>>
}

pub fn from_u128_str<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    Ok(u128::from_str_radix(&s, 10).unwrap())
}

fn load_setup() {
    let toml_str = fs::read_to_string("data/omnipool.toml").expect("Failed to read omnipool.toml file");
    let cargo_toml: OmnipoolSetup = toml::from_str(&toml_str).expect("Failed to deserialize OmnipoolSetup");
    println!("{:#?}", cargo_toml);
}

fn main() {
	load_setup();
}
