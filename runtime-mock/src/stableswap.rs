use crate::{AccountId, MockedRuntime};
use hydradx_runtime::RuntimeCall;
use hydradx_traits::stableswap::AssetAmount;
use serde::Deserialize;
use serde::Deserializer;
use sp_runtime::{FixedPointNumber, FixedU128, Permill};
use std::fs;

#[derive(Debug, Deserialize)]
pub struct AssetReserve {
	asset_id: u32,
	#[serde(deserialize_with = "from_u128_str")]
	reserve: u128,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Pool {
	pool_id: u32,
	#[serde(deserialize_with = "from_u128_str")]
	initial_amplification: u128,
	#[serde(deserialize_with = "from_u128_str")]
	final_amplification: u128,
	#[serde(deserialize_with = "from_u128_str")]
	initial_block: u128,
	#[serde(deserialize_with = "from_u128_str")]
	final_block: u128,
	#[serde(deserialize_with = "from_f64_to_permill")]
	fee: Permill,
	reserves: Vec<AssetReserve>,
}

impl Pool {
	fn get_assets(&self) -> Vec<u32> {
		self.reserves.iter().map(|asset| asset.asset_id).collect()
	}

	fn get_asset_amounts(&self) -> Vec<AssetAmount<u32>> {
		vec![
			AssetAmount::new(10, 300_000_000_000),
			AssetAmount::new(18, 300_000_000_000_000_000_000_000),
			AssetAmount::new(23, 300_000_000_000),
			AssetAmount::new(21, 300_000_000_000),
		]
		/*
		self.reserves
			.iter()
			//.map(|asset| AssetAmount::new(asset.asset_id, asset.reserve))
			.map(|asset| AssetAmount::new(asset.asset_id, 1000))
			.collect()

		 */
	}
}

#[derive(Debug, Deserialize)]
pub struct Stablepools {
	pools: Vec<Pool>,
}

impl Stablepools {
	pub fn endowed_account(&self, account: AccountId) -> Vec<(AccountId, Vec<(u32, u128)>)> {
		let mut result = Vec::new();
		for p in self.pools.iter() {
			let mut reserves = Vec::new();
			for r in p.reserves.iter() {
				reserves.push((r.asset_id, r.reserve * 2)); // add more to avoid ED issues
			}
			result.push((account.clone(), reserves));
		}
		result
	}

	pub fn calls(&self) -> Vec<RuntimeCall> {
		self.pools
			.iter()
			.map(|pool| {
				RuntimeCall::Stableswap(pallet_stableswap::Call::create_pool {
					share_asset: pool.pool_id,
					assets: pool.get_assets(),
					amplification: pool.final_amplification as u16,
					fee: pool.fee,
				})
			})
			.collect()
	}

	pub fn add_liquid_calls(&self) -> Vec<RuntimeCall> {
		self.pools
			.iter()
			.map(|pool| {
				RuntimeCall::Stableswap(pallet_stableswap::Call::add_liquidity {
					pool_id: pool.pool_id,
					assets: pool.get_asset_amounts(),
				})
			})
			.collect()
	}

	pub fn add_token_calls(&self, owner: AccountId) -> Vec<RuntimeCall> {
		vec![
			RuntimeCall::Tokens(orml_tokens::Call::force_transfer {
				source: owner.clone(),
				dest: pallet_omnipool::Pallet::<MockedRuntime>::protocol_account(),
				currency_id: 100,
				amount: 1200000000000000000000000,
			}),
			RuntimeCall::Omnipool(pallet_omnipool::Call::add_token {
				asset: 100,
				initial_price: FixedU128::from_rational(42829670683, FixedU128::DIV),
				weight_cap: Permill::from_parts(200000),
				position_owner: owner,
			}),
		]
	}
}

pub fn from_u128_str<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: String = Deserialize::deserialize(deserializer)?;
	Ok(s.parse::<u128>().unwrap())
}

pub fn from_f64_to_permill<'de, D>(deserializer: D) -> Result<Permill, D::Error>
where
	D: Deserializer<'de>,
{
	let s: f64 = Deserialize::deserialize(deserializer)?;
	Ok(Permill::from_float(s / 100f64))
}

fn load_setup(filename: &str) -> Stablepools {
	let toml_str = fs::read_to_string(filename).expect("Failed to read stableswap.toml file");
	toml::from_str(&toml_str).expect("Failed to deserialize Stablepools")
}

pub fn stablepools() -> Stablepools {
	load_setup("data/stableswap.toml")
}
