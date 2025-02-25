use serde::{Deserialize, Deserializer};

pub type Balance = u128;
pub type AssetId = u32;
pub type IntentId = u128;
pub type FloatType = f64;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Intent {
	pub intent_id: IntentId,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partial: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedIntent {
	pub intent_id: IntentId,
	pub amount_in: Balance,
	pub amount_out: Balance,
}

#[derive(Debug, serde::Deserialize)]
pub enum Asset {
	Omnipool(OmnipoolAsset),
	StableSwap(StableSwapAsset),
}

#[derive(Debug, serde::Deserialize)]
pub struct OmnipoolAsset {
	pub asset_id: AssetId,
	pub reserve: Balance,
	pub hub_reserve: Balance,
	pub decimals: u8,
	#[serde(deserialize_with = "deserialize_fee")]
	pub fee: (u32, u32),
	#[serde(deserialize_with = "deserialize_fee")]
	pub hub_fee: (u32, u32),
}

#[derive(Debug, serde::Deserialize)]
pub struct StableSwapAsset {
	pub pool_id: AssetId,
	pub asset_id: AssetId,
	pub reserve: Balance,
	pub decimals: u8,
	#[serde(deserialize_with = "deserialize_fee")]
	pub fee: (u32, u32),
}

fn deserialize_fee<'de, D>(deserializer: D) -> Result<(u32, u32), D::Error>
where
	D: Deserializer<'de>,
{
	let number: u32 = u32::deserialize(deserializer)?;
	Ok((number, 1_000_000))
}

impl OmnipoolAsset {
	pub fn reserve_as_f64(&self) -> f64 {
		self.reserve as f64 / 10u128.pow(self.decimals as u32) as f64
	}

	pub fn hub_reserve_as_f64(&self) -> f64 {
		self.hub_reserve as f64 / 10u128.pow(12u32) as f64
	}

	pub fn fee_as_f64(&self) -> f64 {
		self.fee.0 as f64 / self.fee.1 as f64
	}

	pub fn hub_fee_as_f64(&self) -> f64 {
		self.hub_fee.0 as f64 / self.hub_fee.1 as f64
	}
	#[cfg(test)]
	pub fn reserve_no_decimals(&self) -> Balance {
		self.reserve / 10u128.pow(self.decimals as u32)
	}
	#[cfg(test)]
	pub fn hub_reserve_no_decimals(&self) -> Balance {
		self.hub_reserve / 10u128.pow(12u32)
	}
}
